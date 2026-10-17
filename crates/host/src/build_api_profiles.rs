//! Capture only new requests. Replays compare the durable selection without
//! requiring removed or changed configuration files to become available again.
use crate::{
    build_record::{
        self as build,
        profiles::{Bindings, Kind, LoadedProfile},
    },
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_project::Voice;
use sungeki_protocol::{ApiError, StartBuildVideoArgs};

pub(super) fn capture(
    shared: &Arc<Mutex<Sessions>>,
    args: &StartBuildVideoArgs,
) -> Result<Bindings, ApiError> {
    let database = {
        let sessions = shared.lock().map_err(invalid)?;
        if let Some(row) = sessions
            .store
            .load_job(&args.job_id)
            .map_err(storage_error)?
        {
            let (input, _) = build::decode(&row)?;
            let ids = input
                .managed_profiles
                .iter()
                .map(|(character, fixed)| (character.clone(), fixed.profile_id.clone()))
                .collect();
            if args.managed_profiles != ids {
                return Err(invalid("保存済みの話者設定を変更できません"));
            }
            // create_configured still compares recipe, output location and VV connection.
            return Ok(input.managed_profiles);
        }
        sessions.review_database_path.clone()
    };
    let mut bindings = Bindings::new();
    let flag = AtomicBool::new(false);
    for role in &args.recipe.roles {
        let kind = match role.character.voice {
            Voice::Voicevox { .. } => continue,
            Voice::Sbv2 { .. } => Kind::Sbv2,
            Voice::Qwen { .. } => Kind::Qwen,
            Voice::Irodori { .. } => Kind::Irodori,
        };
        let id = args
            .managed_profiles
            .get(&role.character.id)
            .ok_or_else(|| invalid("制作話者の起動設定を指定してください"))?;
        let loaded = LoadedProfile::load(&database, kind, id, &flag)?;
        bindings.insert(role.character.id.clone(), loaded.freeze()?);
    }
    if bindings.len() != args.managed_profiles.len() {
        return Err(invalid("余分な話者設定が指定されています"));
    }
    build::profiles::validate(&bindings, &args.recipe)?;
    Ok(bindings)
}

pub(super) fn connection(
    args: &StartBuildVideoArgs,
) -> Result<Option<build::SpeechConnection>, ApiError> {
    let voice = args
        .recipe
        .roles
        .iter()
        .find_map(|r| match &r.character.voice {
            Voice::Voicevox {
                engine_id,
                core_version,
                ..
            } => Some((engine_id, core_version)),
            _ => None,
        });
    let Some((engine_id, core_version)) = voice else {
        if !args.base_url.is_empty() || !args.engine_version.is_empty() {
            return Err(invalid(
                "VOICEVOX話者のない制作にVOICEVOX接続は指定できません",
            ));
        }
        return Ok(None);
    };
    Ok(Some(
        build::SpeechConnection {
            endpoint: args.base_url.clone(),
            identity: sungeki_speech::VoicevoxIdentity {
                engine_id: engine_id.clone(),
                core_version: core_version.clone(),
                engine_version: args.engine_version.clone(),
            },
        }
        .canonical(&args.recipe)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_replay_uses_fixed_selection_without_loading_configuration() {
        let root = std::env::temp_dir().join(format!("build-api-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let database = root.join("db");
        let mut recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recipes/build-dialogue.json"
        ))
        .unwrap();
        for role in &mut recipe.roles {
            role.character.voice = serde_json::from_value(serde_json::json!({
                "type":"irodori", "settings":{"voice_id":"none","caption":null,"cfg_scale":3.0,"num_steps":40,"seed":42},
                "identity":{"checkpoint":"model","model_manifest_sha256":"a".repeat(64),"codec_manifest_sha256":"b".repeat(64),"engine_manifest_sha256":"c".repeat(64),"voice_manifest_sha256":"d".repeat(64),"sample_rate":24000,"channels":1}
            })).unwrap();
        }
        let fixed: Bindings = recipe
            .roles
            .iter()
            .map(|r| {
                (
                    r.character.id.clone(),
                    build::profiles::FrozenProfile {
                        kind: Kind::Irodori,
                        profile_id: "local".into(),
                        configuration_sha256: "e".repeat(64),
                    },
                )
            })
            .collect();
        let mut args = StartBuildVideoArgs {
            wait_ms: None,
            reuse_from: None,
            managed_profiles: fixed
                .iter()
                .map(|(k, v)| (k.clone(), v.profile_id.clone()))
                .collect(),
            job_id: uuid::Uuid::new_v4().to_string(),
            recipe,
            project_path: root.join("replay.vvm.json").to_string_lossy().into(),
            base_url: String::new(),
            engine_version: String::new(),
        };
        let mut sessions = Sessions::open(&database).unwrap();
        build::create_configured(
            &mut sessions,
            &args.job_id,
            args.recipe.clone(),
            std::path::Path::new(&args.project_path),
            None,
            fixed.clone(),
        )
        .unwrap();
        drop(sessions);
        let shared = Arc::new(Mutex::new(Sessions::open(&database).unwrap()));
        assert_eq!(capture(&shared, &args).unwrap(), fixed);
        let response = build::execute(
            &shared,
            sungeki_protocol::BuildVideoRequest::Start { args: args.clone() },
        )
        .unwrap();
        let sungeki_protocol::SessionResponse::BuildVideoJob { job } = response else {
            panic!()
        };
        assert_eq!(job.phase, sungeki_project::JobPhase::Interrupted);
        args.managed_profiles
            .values_mut()
            .next()
            .unwrap()
            .push_str("-changed");
        assert!(capture(&shared, &args).is_err());
        args.base_url = "http://127.0.0.1:55021".into();
        assert!(connection(&args).is_err());
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
