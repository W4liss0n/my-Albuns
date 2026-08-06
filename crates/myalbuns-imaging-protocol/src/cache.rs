use myalbuns_paths::{CacheArtifactFormat, CachePathPlan, RootBindingPlan};
use serde::{Deserialize, Serialize};

use crate::render::MediaSource;
use crate::{IMAGING_PROTOCOL_VERSION, is_safe_identifier};

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
        if !is_safe_identifier(&self.request_id) {
            return Err("a Identidade da solicitação é inválida".into());
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
                .preview_file(
                    source.media_id(),
                    &job.generation_id,
                    CacheArtifactFormat::Jpeg,
                )
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
pub struct CacheCompletion {
    pub artifacts: Vec<CacheArtifact>,
    pub generated_count: usize,
    pub reused_count: usize,
    pub source_bytes: u64,
    pub preview_bytes: u64,
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
