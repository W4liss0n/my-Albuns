use myalbuns_paths::RootBindingPlan;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cache::CacheRequest;
use crate::render::ImagingRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImagingFailureStage {
    InvalidRenderRequest,
    CacheProcessing,
    ResourceLimitExceeded,
    SourceVerification,
    SourceDecode,
    Composition,
    OutputPrepare,
    OutputEncode,
    OutputVerify,
}

impl ImagingFailureStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRenderRequest => "invalid_render_request",
            Self::CacheProcessing => "cache_processing",
            Self::ResourceLimitExceeded => "resource_limit_exceeded",
            Self::SourceVerification => "source_verification",
            Self::SourceDecode => "source_decode",
            Self::Composition => "composition",
            Self::OutputPrepare => "output_prepare",
            Self::OutputEncode => "output_encode",
            Self::OutputVerify => "output_verify",
        }
    }

    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidRenderRequest => 29,
            Self::CacheProcessing => 27,
            Self::ResourceLimitExceeded => 28,
            Self::SourceVerification => 20,
            Self::SourceDecode => 21,
            Self::Composition => 22,
            Self::OutputPrepare => 23,
            Self::OutputEncode => 24,
            Self::OutputVerify => 26,
        }
    }

    pub const fn from_exit_code(exit_code: i32) -> Option<Self> {
        match exit_code {
            29 => Some(Self::InvalidRenderRequest),
            27 => Some(Self::CacheProcessing),
            28 => Some(Self::ResourceLimitExceeded),
            20 => Some(Self::SourceVerification),
            21 => Some(Self::SourceDecode),
            22 => Some(Self::Composition),
            23 => Some(Self::OutputPrepare),
            24 => Some(Self::OutputEncode),
            26 => Some(Self::OutputVerify),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "camelCase")]
pub enum ImagingCommand {
    Render(ImagingRequest),
    BuildCache(CacheRequest),
}

impl ImagingCommand {
    pub fn render(request: ImagingRequest) -> Self {
        Self::Render(request)
    }

    pub fn build_cache(request: CacheRequest) -> Self {
        Self::BuildCache(request)
    }

    /// Returns the immutable path plan carried by commands that perform
    /// external I/O.
    pub fn root_bindings(&self) -> &RootBindingPlan {
        match self {
            Self::Render(request) => &request.root_bindings,
            Self::BuildCache(request) => &request.root_bindings,
        }
    }
}

/// Produces an opaque correlation value for one frozen root-binding plan.
///
/// The digest is diagnostic evidence only: it is never used to resolve paths,
/// compare filesystem objects or select a cached plan.
pub fn root_binding_plan_sha256(plan: &RootBindingPlan) -> Result<String, String> {
    let payload = serde_json::to_vec(plan)
        .map_err(|error| format!("não foi possível serializar o plano de raízes: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}

pub fn encode_command(command: &ImagingCommand) -> Result<Vec<u8>, String> {
    let mut payload = serde_json::to_vec(command)
        .map_err(|error| format!("não foi possível serializar o comando: {error}"))?;
    payload.push(b'\n');
    Ok(payload)
}

pub fn decode_command(payload: &[u8]) -> Result<ImagingCommand, String> {
    serde_json::from_slice(payload)
        .map_err(|error| format!("comando do Processador de Imagens inválido: {error}"))
}
