use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    project_document::{MAX_SAFE_INTEGER, ProjectRevision},
    project_store,
};

const RECOVERY_SCHEMA_VERSION: u32 = 1;

/// Consolidated creative state captured after one completed action.
///
/// The type intentionally carries no History or runtime media state. Only an
/// authorized editable Project can create one; decoding alone never grants a
/// local namespace authority.
#[derive(Clone, Debug)]
pub struct RecoveryCheckpoint {
    pub(crate) project_id: Uuid,
    pub(crate) base_saved_revision: u64,
    pub(crate) creative_revision: ProjectRevision,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RecoveryCheckpointError {
    #[error("o checkpoint de Recuperação é inválido")]
    InvalidCheckpoint,
    #[error("o checkpoint pertence a outra Identidade de Projeto")]
    IdentityMismatch,
    #[error("o checkpoint deriva de outra Revisão salva")]
    BaselineMismatch,
    #[error("a Sessão editável não está disponível")]
    SessionUnavailable,
}

impl RecoveryCheckpoint {
    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    pub fn base_saved_revision(&self) -> u64 {
        self.base_saved_revision
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, RecoveryCheckpointError> {
        let creative_state = serde_json::from_slice(
            &project_store::encode(&self.creative_revision)
                .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?,
        )
        .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
        let project_id = self.project_id.hyphenated().to_string();
        let envelope = RecoveryEnvelopeV1 {
            schema_version: RECOVERY_SCHEMA_VERSION,
            project_id: project_id.clone(),
            base_revision: RecoveryBaseRevisionV1 {
                project_id,
                revision: self.base_saved_revision,
            },
            creative_state,
        };
        let mut bytes = serde_json::to_vec_pretty(&envelope)
            .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RecoveryCheckpointError> {
        let envelope: RecoveryEnvelopeV1 = serde_json::from_slice(bytes)
            .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
        if envelope.schema_version != RECOVERY_SCHEMA_VERSION
            || envelope.base_revision.revision > MAX_SAFE_INTEGER
        {
            return Err(RecoveryCheckpointError::InvalidCheckpoint);
        }
        let project_id = parse_canonical_uuid(&envelope.project_id)?;
        if parse_canonical_uuid(&envelope.base_revision.project_id)? != project_id {
            return Err(RecoveryCheckpointError::InvalidCheckpoint);
        }
        let creative_bytes = serde_json::to_vec(&envelope.creative_state)
            .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
        let creative_revision = project_store::decode(&creative_bytes)
            .map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
        if creative_revision.project_id != project_id {
            return Err(RecoveryCheckpointError::InvalidCheckpoint);
        }
        Ok(Self {
            project_id,
            base_saved_revision: envelope.base_revision.revision,
            creative_revision,
        })
    }
}

fn parse_canonical_uuid(value: &str) -> Result<Uuid, RecoveryCheckpointError> {
    let project_id =
        Uuid::parse_str(value).map_err(|_| RecoveryCheckpointError::InvalidCheckpoint)?;
    (project_id.hyphenated().to_string() == value)
        .then_some(project_id)
        .ok_or(RecoveryCheckpointError::InvalidCheckpoint)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryEnvelopeV1 {
    schema_version: u32,
    project_id: String,
    base_revision: RecoveryBaseRevisionV1,
    creative_state: serde_json::Value,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecoveryBaseRevisionV1 {
    project_id: String,
    revision: u64,
}
