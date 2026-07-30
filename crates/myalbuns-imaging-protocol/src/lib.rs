use std::path::PathBuf;

use myalbuns_core::RenderSnapshot;
use myalbuns_paths::{CachePathPlan, RootBindingPlan};
use serde::{Deserialize, Serialize};

pub const IMAGING_PROTOCOL_VERSION: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImagingFailureStage {
    CacheProcessing,
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
            Self::CacheProcessing => "cache_processing",
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
            Self::CacheProcessing => 27,
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
            27 => Some(Self::CacheProcessing),
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
    ResetCache(CacheResetRequest),
}

impl ImagingCommand {
    pub fn render(request: ImagingRequest) -> Self {
        Self::Render(request)
    }

    pub fn build_cache(request: CacheRequest) -> Self {
        Self::BuildCache(request)
    }

    pub fn reset_cache(request: CacheResetRequest) -> Self {
        Self::ResetCache(request)
    }
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

pub fn encode_response(response: &ImagingResponse) -> Result<Vec<u8>, String> {
    serde_json::to_vec(response)
        .map_err(|error| format!("não foi possível serializar a resposta: {error}"))
}

pub fn decode_response(payload: &[u8]) -> Result<ImagingResponse, String> {
    serde_json::from_slice(payload)
        .map_err(|error| format!("resposta do Processador de Imagens inválida: {error}"))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub prepared_output_path: PathBuf,
    pub snapshot: RenderSnapshot,
    pub sheet_id: String,
    pub dpi: u32,
    pub sources: Vec<MediaSource>,
    pub source_policy: RenderSourcePolicy,
    pub root_bindings: RootBindingPlan,
}

impl ImagingRequest {
    pub fn new(
        request_id: impl Into<String>,
        prepared_output_path: PathBuf,
        snapshot: RenderSnapshot,
        sheet_id: impl Into<String>,
        dpi: u32,
        sources: Vec<MediaSource>,
        root_bindings: RootBindingPlan,
    ) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            prepared_output_path,
            snapshot,
            sheet_id: sheet_id.into(),
            dpi,
            sources,
            source_policy: RenderSourcePolicy::LinkedOriginals,
            root_bindings,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn procedural_fixture(
        request_id: impl Into<String>,
        prepared_output_path: PathBuf,
        snapshot: RenderSnapshot,
        sheet_id: impl Into<String>,
        dpi: u32,
        root_bindings: RootBindingPlan,
    ) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            prepared_output_path,
            snapshot,
            sheet_id: sheet_id.into(),
            dpi,
            sources: vec![],
            source_policy: RenderSourcePolicy::ProceduralFixture,
            root_bindings,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != IMAGING_PROTOCOL_VERSION {
            return Err(format!(
                "versão de protocolo não suportada: {}",
                self.protocol_version
            ));
        }
        if !is_safe_identifier(&self.request_id) {
            return Err("a Identidade da solicitação é inválida".into());
        }
        if !self.prepared_output_path.is_absolute() {
            return Err("o caminho da preparação não é absoluto".into());
        }
        self.root_bindings
            .validate()
            .map_err(|error| format!("o plano de raízes é inválido: {error}"))?;
        if !self.root_bindings.covers(&self.prepared_output_path) {
            return Err("o caminho da preparação não pertence ao plano de raízes".into());
        }
        if !(1..=1200).contains(&self.dpi) {
            return Err("a resolução da Exportação é inválida".into());
        }
        self.snapshot
            .validate()
            .map_err(|error| format!("snapshot inválido: {error}"))?;
        let sheet = self
            .snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == self.sheet_id)
            .ok_or_else(|| "a Lâmina solicitada não existe no snapshot".to_string())?;
        let required_media = sheet
            .frames
            .iter()
            .filter_map(|frame| frame.photo.as_ref())
            .map(|photo| photo.media_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        if required_media.is_empty() {
            return Err("a Lâmina solicitada não contém Fotos".into());
        }
        match self.source_policy {
            RenderSourcePolicy::LinkedOriginals => {
                let mut supplied_media = std::collections::HashSet::new();
                for source in &self.sources {
                    source.validate()?;
                    if !self.root_bindings.covers(source.source_path()) {
                        return Err(format!(
                            "a raiz da mídia {} não pertence ao plano da operação",
                            source.media_id()
                        ));
                    }
                    if !supplied_media.insert(source.media_id()) {
                        return Err("a Exportação contém mídia duplicada".into());
                    }
                }
                if supplied_media != required_media {
                    return Err(
                        "as fontes da Exportação não correspondem às Fotos da Lâmina".into(),
                    );
                }
            }
            RenderSourcePolicy::ProceduralFixture if !self.sources.is_empty() => {
                return Err("a prova procedural não aceita fontes nativas".into());
            }
            RenderSourcePolicy::ProceduralFixture => {}
        }
        Ok(())
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderSourcePolicy {
    LinkedOriginals,
    ProceduralFixture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub project_id: String,
    pub cache_paths: CachePathPlan,
    pub jobs: Vec<CacheJob>,
    pub max_edge_px: u32,
    pub root_bindings: RootBindingPlan,
}

impl CacheRequest {
    pub fn new(
        request_id: impl Into<String>,
        project_id: impl Into<String>,
        cache_paths: CachePathPlan,
        jobs: Vec<CacheJob>,
        max_edge_px: u32,
        root_bindings: RootBindingPlan,
    ) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            project_id: project_id.into(),
            cache_paths,
            jobs,
            max_edge_px,
            root_bindings,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != IMAGING_PROTOCOL_VERSION {
            return Err(format!(
                "versão de protocolo não suportada: {}",
                self.protocol_version
            ));
        }
        if self.request_id.trim().is_empty() {
            return Err("a Identidade da solicitação está vazia".into());
        }
        if self.project_id.trim().is_empty() {
            return Err("a Identidade do Projeto está vazia".into());
        }
        if self.jobs.is_empty() {
            return Err("a solicitação de Cache não contém Fotos".into());
        }
        if !(1..=4096).contains(&self.max_edge_px) {
            return Err("a dimensão da representação reduzida é inválida".into());
        }
        self.cache_paths
            .validate()
            .map_err(|error| error.to_string())?;
        self.root_bindings
            .validate()
            .map_err(|error| format!("o plano de raízes é inválido: {error}"))?;
        if !self.root_bindings.covers(self.cache_paths.root()) {
            return Err("a raiz do Cache não pertence ao plano da operação".into());
        }

        let mut media_ids = std::collections::HashSet::new();
        for job in &self.jobs {
            job.validate()?;
            let source = &job.source;
            if !self.root_bindings.covers(source.source_path()) {
                return Err(format!(
                    "a raiz da mídia {} não pertence ao plano da operação",
                    source.media_id()
                ));
            }
            self.cache_paths
                .preview_file(source.media_id(), &job.generation_id)
                .map_err(|error| error.to_string())?;
            if !media_ids.insert(source.media_id()) {
                return Err("a solicitação de Cache contém mídia duplicada".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheJob {
    pub source: MediaSource,
    pub generation_id: String,
}

impl CacheJob {
    pub fn new(source: MediaSource, generation_id: impl Into<String>) -> Result<Self, String> {
        let job = Self {
            source,
            generation_id: generation_id.into(),
        };
        job.validate()?;
        Ok(job)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        if !is_safe_identifier(&self.generation_id) {
            return Err(format!(
                "a geração da mídia {} é inválida",
                self.source.media_id()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSource {
    media_id: String,
    source_path: PathBuf,
    source_bytes: u64,
    source_sha256: String,
}

impl MediaSource {
    pub fn new(
        media_id: impl Into<String>,
        source_path: PathBuf,
        source_bytes: u64,
        source_sha256: impl Into<String>,
    ) -> Result<Self, String> {
        let source = Self {
            media_id: media_id.into(),
            source_path,
            source_bytes,
            source_sha256: source_sha256.into(),
        };
        source.validate()?;
        Ok(source)
    }

    pub fn media_id(&self) -> &str {
        &self.media_id
    }

    pub fn source_path(&self) -> &std::path::Path {
        &self.source_path
    }

    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    fn validate(&self) -> Result<(), String> {
        if self.media_id.is_empty()
            || self.media_id.len() > 128
            || !self
                .media_id
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
        {
            return Err("a identidade de mídia é inválida".into());
        }
        if !self.source_path.is_absolute() {
            return Err(format!(
                "o caminho da mídia {} não é absoluto",
                self.media_id
            ));
        }
        if self.source_bytes == 0 {
            return Err(format!("a mídia {} está vazia", self.media_id));
        }
        if self.source_sha256.len() != 64
            || !self
                .source_sha256
                .bytes()
                .all(|value| value.is_ascii_hexdigit())
        {
            return Err(format!(
                "o fingerprint da mídia {} é inválido",
                self.media_id
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheCompletion {
    pub artifacts: Vec<CacheArtifact>,
    pub generated_count: usize,
    pub reused_count: usize,
    pub source_bytes: u64,
    pub preview_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCompletion {
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: u32,
    pub source_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub output_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheArtifact {
    pub media_id: String,
    pub generation_id: String,
    pub width_px: u32,
    pub height_px: u32,
    pub preview_bytes: u64,
    pub format: CacheArtifactFormat,
    pub exif_orientation: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheArtifactFormat {
    Jpeg,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheResetRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub project_ids: Vec<String>,
}

impl CacheResetRequest {
    pub fn new(request_id: impl Into<String>, project_ids: Vec<String>) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            project_ids,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != IMAGING_PROTOCOL_VERSION {
            return Err(format!(
                "versão de protocolo não suportada: {}",
                self.protocol_version
            ));
        }
        if self.request_id.trim().is_empty() {
            return Err("a Identidade da solicitação está vazia".into());
        }
        if self.project_ids.is_empty() {
            return Err("a limpeza de Cache não contém Projetos".into());
        }
        let unique = self
            .project_ids
            .iter()
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != self.project_ids.len() {
            return Err("a limpeza de Cache contém Projeto duplicado".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImagingResponse {
    Completed {
        request_id: String,
        completion: RenderCompletion,
    },
    CacheCompleted {
        request_id: String,
        completion: CacheCompletion,
    },
    CacheReset {
        request_id: String,
        removed_count: usize,
    },
}

impl ImagingResponse {
    pub fn completed(request_id: impl Into<String>, completion: RenderCompletion) -> Self {
        Self::Completed {
            request_id: request_id.into(),
            completion,
        }
    }

    pub fn cache_completed(request_id: impl Into<String>, completion: CacheCompletion) -> Self {
        Self::CacheCompleted {
            request_id: request_id.into(),
            completion,
        }
    }

    pub fn cache_reset(request_id: impl Into<String>, removed_count: usize) -> Self {
        Self::CacheReset {
            request_id: request_id.into(),
            removed_count,
        }
    }

    pub fn completed_for(&self, expected_request_id: &str) -> Option<&RenderCompletion> {
        match self {
            Self::Completed {
                request_id,
                completion,
            } if request_id == expected_request_id => Some(completion),
            _ => None,
        }
    }

    pub fn cache_completed_for(&self, expected_request_id: &str) -> Option<&CacheCompletion> {
        match self {
            Self::CacheCompleted {
                request_id,
                completion,
            } if request_id == expected_request_id => Some(completion),
            _ => None,
        }
    }

    pub fn cache_reset_for(&self, expected_request_id: &str) -> Option<usize> {
        match self {
            Self::CacheReset {
                request_id,
                removed_count,
            } if request_id == expected_request_id => Some(*removed_count),
            _ => None,
        }
    }
}
