//! Immutable provenance for a NEW parent. References are not permission to reuse:
//! dispatch must revalidate originals and match each fresh target input again.
use super::speech_reuse::VerifiedReuse;
use crate::{
    build_state::{hash_ok, invalid},
    sessions::{Sessions, storage_error},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sungeki_protocol::ApiError;
use sungeki_storage::StoredJob;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordStamp {
    id: String,
    sha256: String,
}
impl RecordStamp {
    fn new(row: &StoredJob) -> Result<Self, ApiError> {
        let bytes = serde_json::to_vec(&(
            row.id.as_str(),
            row.session_id.as_str(),
            row.version,
            &row.input,
            &row.state,
        ))
        .map_err(invalid)?;
        Ok(Self {
            id: row.id.clone(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    fn valid(&self) -> bool {
        uuid::Uuid::parse_str(&self.id).is_ok_and(|id| id.to_string() == self.id)
            && hash_ok(&self.sha256)
    }
    fn load(&self, sessions: &Sessions) -> Result<StoredJob, ApiError> {
        let row = sessions
            .store
            .load_job(&self.id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("続行元の記録がありません"))?;
        if Self::new(&row)? != *self {
            return Err(invalid("続行元の記録が固定時点から変更されています"));
        }
        Ok(row)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    target_index: u16,
    source_index: u16,
    child: RecordStamp,
    original_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Continuation {
    parent: RecordStamp,
    speech: Vec<Binding>,
}
impl Continuation {
    pub fn from_verified(proofs: &[(u16, VerifiedReuse)], count: u16) -> Result<Self, ApiError> {
        let first = proofs
            .first()
            .ok_or_else(|| invalid("再利用する音声がありません"))?;
        let parent = RecordStamp::new(&first.1.parent)?;
        let mut speech = Vec::with_capacity(proofs.len());
        for (target_index, proof) in proofs {
            if RecordStamp::new(&proof.parent)? != parent {
                return Err(invalid("異なる制作の再利用証拠は混在できません"));
            }
            speech.push(Binding {
                target_index: *target_index,
                source_index: super::decode(&proof.parent)?
                    .0
                    .step_ids
                    .iter()
                    .position(|id| id == &proof.child.id)
                    .and_then(|i| i.checked_sub(2))
                    .ok_or_else(|| invalid("再利用元の発話番号がありません"))?
                    as u16,
                child: RecordStamp::new(&proof.child)?,
                original_sha256: proof.artifact.sha256.clone(),
            });
        }
        speech.sort_by_key(|binding| binding.target_index);
        let result = Self { parent, speech };
        result.validate(count)?;
        Ok(result)
    }
    pub fn merge(mut self, other: Self, count: u16) -> Result<Self, ApiError> {
        if self.parent != other.parent {
            return Err(invalid("再利用元の制作記録が変わりました"));
        }
        self.speech.extend(other.speech);
        self.speech.sort_by_key(|b| b.target_index);
        self.validate(count)?;
        Ok(self)
    }
    pub fn validate(&self, count: u16) -> Result<(), ApiError> {
        if !self.parent.valid()
            || self.speech.is_empty()
            || self.speech.len() > usize::from(count)
            || self.speech.iter().any(|b| {
                b.target_index >= count
                    || !b.child.valid()
                    || b.child.id == self.parent.id
                    || !hash_ok(&b.original_sha256)
            })
            || self
                .speech
                .windows(2)
                .any(|pair| pair[0].target_index >= pair[1].target_index)
        {
            return Err(invalid("続行の音声対応または固定記録が不正です"));
        }
        Ok(())
    }
    /// The binding is a candidate only. Call speech_reuse::verify with the new
    /// target, then verify_proof before publishing or committing any result.
    pub fn source_for(&self, target_index: u16) -> Option<(&str, &str)> {
        self.speech
            .iter()
            .find(|b| b.target_index == target_index)
            .map(|b| (self.parent.id.as_str(), b.child.id.as_str()))
    }
    pub fn verify_proof(&self, target_index: u16, proof: &VerifiedReuse) -> Result<(), ApiError> {
        let binding = self
            .speech
            .iter()
            .find(|b| b.target_index == target_index)
            .ok_or_else(|| invalid("この発話には再利用元がありません"))?;
        if RecordStamp::new(&proof.parent)? != self.parent
            || RecordStamp::new(&proof.child)? != binding.child
            || proof.artifact.sha256 != binding.original_sha256
            || proof.audio.info().content_hash != binding.original_sha256
        {
            return Err(invalid("再利用の検証結果が保存済みの対応と一致しません"));
        }
        Ok(())
    }
    pub fn matches_request(&self, request: &sungeki_protocol::BuildReuseRequest) -> bool {
        self.parent.id == request.parent_job_id
            && self
                .speech
                .iter()
                .map(|b| (b.target_index.to_string(), b.source_index))
                .collect::<std::collections::BTreeMap<_, _>>()
                == request.speech
    }
    pub fn verify_current(&self, sessions: &Sessions, new_parent_id: &str) -> Result<(), ApiError> {
        if self.parent.id == new_parent_id {
            return Err(invalid("続行には新しい制作IDが必要です"));
        }
        let parent = self.parent.load(sessions)?;
        let (_, state) = super::decode(&parent)?;
        if !state.phase.is_terminal() || !super::settlement::ready(sessions, &parent)? {
            return Err(invalid("続行元の全処理の終了を確認できません"));
        }
        for binding in &self.speech {
            let child = binding.child.load(sessions)?;
            let (input, state) = crate::speech_journal::decode_input(&child)?;
            if child.session_id != parent.session_id
                || input.parent_job_id() != Some(&parent.id)
                || state
                    .artifact
                    .as_ref()
                    .is_none_or(|a| a.sha256 != binding.original_sha256)
            {
                return Err(invalid("続行元の音声記録が一致しません"));
            }
        }
        Ok(())
    }
}
