use crate::{RenderPlan, master_policy as policy};
use sungeki_media::{LoudnessAnalysis, LoudnessValue};
use sungeki_project::{Normalization, Project, Scene, TimeUs};

fn report(lufs: f64, peak: f64) -> LoudnessAnalysis {
    serde_json::from_value(
        serde_json::json!({"version":2,"method":"ffmpeg_ebur128_summary_192k",
        "source_sha256":"a".repeat(64),"source_frames":192000,"sample_rate":48000,"channels":2,
        "source_pre_gain":false,"dual_mono":false,"measurement":{"status":"measured",
        "integrated_lufs":lufs,"true_peak_db":peak,"range_lu":0,"threshold_lufs":-30}}),
    )
    .unwrap()
}
#[test]
fn master_preserves_mix_balance_and_rejects_unreachable_targets() {
    let target = Normalization {
        target_lufs: -20.,
        true_peak_db: -1.,
    };
    assert!((policy::gain(&report(-22.1, -4.2), &target).unwrap() - 2.1).abs() < 1e-9);
    assert_eq!(policy::gain(&report(-16., -2.), &target).unwrap(), -4.);
    assert!(policy::gain(&report(-30., -2.), &target).is_err());
    assert!(policy::gain(&report(-60., -50.), &target).is_err());
    assert!(policy::gain(&report(-20., -0.9), &target).is_err());
    let mut source = report(-22.1, -4.2);
    source.source_pre_gain = true;
    assert!(policy::gain(&source, &target).is_err());
    let mut source = report(-22.1, -4.2);
    source.measurement = LoudnessValue::NotMeasurable {
        reason: sungeki_media::LoudnessUnavailable::TooShort,
        true_peak_db: None,
    };
    assert!(policy::gain(&source, &target).is_err());
    assert!(policy::verify(&report(-20.2, -1.1), &target).is_ok());
    assert!(policy::verify(&report(-20.3, -1.1), &target).is_err());
    assert!(policy::verify(&report(-20., -1.), &target).is_err());
    assert!(
        policy::gain(
            &report(-20., -20.),
            &Normalization {
                target_lufs: -70.,
                true_peak_db: -1.
            }
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires SSD VVM_MEDIA_TEST_ROOT, VVM_TEST_FFMPEG and VVM_TEST_FFPROBE"]
fn real_master_pcm_and_encoded_aac_meet_target_or_fail_explicitly() {
    use crate::{
        RenderRuntime, audio_mix,
        execute::{Runner, strings},
        master_audio,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("master-test-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let runtime = RenderRuntime {
        vrm: None,
        ffmpeg: std::env::var_os("VVM_TEST_FFMPEG").unwrap().into(),
        ffprobe: std::env::var_os("VVM_TEST_FFPROBE").unwrap().into(),
        python: "/unused".into(),
        image_worker: "/unused".into(),
        font: "/unused".into(),
        font_sha256: String::new(),
        deadline: Duration::from_secs(60),
    };
    let flag = AtomicBool::new(false);
    for (name, signal, target) in [
        (
            "lower",
            "sine=frequency=1000:sample_rate=48000:duration=4",
            -26.,
        ),
        (
            "raise",
            "sine=frequency=1000:sample_rate=48000:duration=4",
            -16.,
        ),
        ("silent", "anullsrc=r=48000:cl=stereo:d=4", -20.),
    ] {
        let dir = root.0.join(name);
        fs::create_dir(&dir).unwrap();
        let runner = Runner {
            runtime: (&runtime).into(),
            directory: &dir,
            flag: &flag,
            end: Instant::now() + runtime.deadline,
        };
        runner
            .ffmpeg(strings(&[
                "-f",
                "lavfi",
                "-i",
                signal,
                "-ac",
                "2",
                "-c:a",
                "pcm_f32le",
                "mix.wav",
            ]))
            .unwrap();
        let mut project =
            Project::empty("00000000-0000-4000-8000-000000000001".into(), name.into());
        project.scenes.push(Scene {
            fade: None,
            id: "00000000-0000-4000-8000-000000000002".into(),
            name: "test".into(),
            start_us: TimeUs(0),
            duration_us: TimeUs(4_000_000),
        });
        let disabled = RenderPlan::new(&project, |_| Ok(())).unwrap();
        assert!(
            master_audio::prepare(&disabled, &runner, "unused")
                .unwrap()
                .is_none()
        );
        project.audio_mix.normalization = Some(Normalization {
            target_lufs: target,
            true_peak_db: -1.,
        });
        let before = project.clone();
        let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
        let (hash, _, _) = audio_mix::inspect(&runner, "mix.wav", plan.audio_samples()).unwrap();
        let prepared = master_audio::prepare(&plan, &runner, &hash);
        if name == "silent" {
            assert!(prepared.unwrap_err().to_string().contains("測定できません"));
            assert!(!dir.join("master.wav").exists());
            continue;
        }
        let prepared = prepared.unwrap().unwrap();
        assert!(
            prepared["gain_db"].as_f64().unwrap() * if name == "lower" { -1. } else { 1. } > 0.
        );
        runner
            .ffmpeg(strings(&[
                "-f",
                "lavfi",
                "-i",
                "color=s=16x16:r=30:d=4",
                "-i",
                "master.wav",
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "output.mp4",
            ]))
            .unwrap();
        let result = master_audio::verify_encoded(&plan, &runner, prepared).unwrap();
        assert_eq!(result["encoded_verified"], true);
        assert!(!dir.join("decoded-audio").exists());
        assert_eq!(
            audio_mix::inspect(&runner, "mix.wav", plan.audio_samples())
                .unwrap()
                .0,
            hash
        );
        assert_eq!(project, before);
        assert!(
            master_audio::prepare(&plan, &runner, &hash).is_err(),
            "cannot overwrite existing master"
        );
        println!("{name}: {}", result["encoded"]["measurement"]);
    }
}
