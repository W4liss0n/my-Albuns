use myalbuns_core::MediaKind;
use myalbuns_paths::{CacheArtifactFormat, CachePathPlan, NativePathDto, RootBindingPlan};
use serde::{Deserialize, Serialize};

use crate::{IMAGING_PROTOCOL_VERSION, is_safe_identifier};

pub const CACHE_REPRESENTATION_VERSION: u32 = 1;
pub const CACHE_MAX_EDGE_PX: u32 = 1_600;
pub const CACHE_MAX_DECODED_PIXELS: u64 = 134_217_728;
pub const CACHE_MAX_DECODER_ALLOC_BYTES: u64 = 512 * 1024 * 1024;
pub const CACHE_JPEG_QUALITY: u8 = 84;
pub const CACHE_FINGERPRINT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheBasicColorProfile {
    Srgb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheRepresentationPolicy {
    pub representation_version: u32,
    pub max_edge_px: u32,
    pub max_decoded_pixels: u64,
    pub max_decoder_alloc_bytes: u64,
    pub jpeg_quality: u8,
    pub accept_single_page_tiff: bool,
    pub reject_multi_page_tiff: bool,
    pub embed_srgb_profile: bool,
}

impl CacheRepresentationPolicy {
    pub const fn measured_v1() -> Self {
        Self {
            representation_version: CACHE_REPRESENTATION_VERSION,
            max_edge_px: CACHE_MAX_EDGE_PX,
            max_decoded_pixels: CACHE_MAX_DECODED_PIXELS,
            max_decoder_alloc_bytes: CACHE_MAX_DECODER_ALLOC_BYTES,
            jpeg_quality: CACHE_JPEG_QUALITY,
            accept_single_page_tiff: true,
            reject_multi_page_tiff: true,
            embed_srgb_profile: true,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if *self != Self::measured_v1() {
            return Err("a política de representação reduzida não é suportada".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheFingerprint {
    pub version: u32,
    pub algorithm: String,
    pub source_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_created_unix_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_modified_unix_ms: Option<u64>,
    pub value: String,
}

impl CacheFingerprint {
    pub fn sha256_full_file(source_bytes: u64, value: impl Into<String>) -> Result<Self, String> {
        Self::sha256_full_file_with_timestamps(source_bytes, None, None, value)
    }

    pub fn sha256_full_file_with_timestamps(
        source_bytes: u64,
        source_created_unix_ms: Option<u64>,
        source_modified_unix_ms: Option<u64>,
        value: impl Into<String>,
    ) -> Result<Self, String> {
        let fingerprint = Self {
            version: CACHE_FINGERPRINT_VERSION,
            algorithm: "sha256-full-file-v1".into(),
            source_bytes,
            source_created_unix_ms,
            source_modified_unix_ms,
            value: value.into(),
        };
        fingerprint.validate()?;
        Ok(fingerprint)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != CACHE_FINGERPRINT_VERSION
            || self.algorithm != "sha256-full-file-v1"
            || self.value.len() != 64
            || !self.value.bytes().all(|value| value.is_ascii_hexdigit())
        {
            return Err("o fingerprint versionado da mídia é inválido".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMediaSource {
    media_id: String,
    kind: MediaKind,
    source_path: NativePathDto,
}

impl CacheMediaSource {
    pub fn new(
        media_id: impl Into<String>,
        kind: MediaKind,
        source_path: impl Into<NativePathDto>,
    ) -> Result<Self, String> {
        let source = Self {
            media_id: media_id.into(),
            kind,
            source_path: source_path.into(),
        };
        source.validate()?;
        Ok(source)
    }

    pub fn media_id(&self) -> &str {
        &self.media_id
    }

    pub const fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn source_path(&self) -> &std::path::Path {
        self.source_path.as_path()
    }

    fn validate(&self) -> Result<(), String> {
        if self.media_id.trim().is_empty() {
            return Err("a identidade de mídia é inválida".into());
        }
        if !self.source_path().is_absolute() {
            return Err(format!(
                "o caminho da mídia {} não é absoluto",
                self.media_id
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheReusableGeneration {
    pub generation_id: String,
    pub format: CacheArtifactFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub preview_bytes: u64,
    pub exif_orientation: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_page_count: Option<u32>,
    pub basic_color_profile: CacheBasicColorProfile,
    pub fingerprint: CacheFingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheArtifactProperties {
    format: CacheArtifactFormat,
    width_px: u32,
    height_px: u32,
    preview_bytes: u64,
    exif_orientation: Option<u8>,
    source_page_count: Option<u32>,
    basic_color_profile: CacheBasicColorProfile,
}

impl CacheArtifactProperties {
    pub const fn new(
        format: CacheArtifactFormat,
        width_px: u32,
        height_px: u32,
        preview_bytes: u64,
        exif_orientation: Option<u8>,
        source_page_count: Option<u32>,
        basic_color_profile: CacheBasicColorProfile,
    ) -> Self {
        Self {
            format,
            width_px,
            height_px,
            preview_bytes,
            exif_orientation,
            source_page_count,
            basic_color_profile,
        }
    }
}

impl CacheReusableGeneration {
    pub fn new(
        generation_id: impl Into<String>,
        properties: CacheArtifactProperties,
        fingerprint: CacheFingerprint,
    ) -> Result<Self, String> {
        let generation = Self {
            generation_id: generation_id.into(),
            format: properties.format,
            width_px: properties.width_px,
            height_px: properties.height_px,
            preview_bytes: properties.preview_bytes,
            exif_orientation: properties.exif_orientation,
            source_page_count: properties.source_page_count,
            basic_color_profile: properties.basic_color_profile,
            fingerprint,
        };
        generation.validate()?;
        Ok(generation)
    }

    fn validate(&self) -> Result<(), String> {
        if !is_safe_identifier(&self.generation_id)
            || self.width_px == 0
            || self.height_px == 0
            || self.width_px > CACHE_MAX_EDGE_PX
            || self.height_px > CACHE_MAX_EDGE_PX
            || self.preview_bytes == 0
            || self
                .exif_orientation
                .is_some_and(|orientation| !(1..=8).contains(&orientation))
            || self
                .source_page_count
                .is_some_and(|page_count| page_count != 1)
        {
            return Err("a geração reutilizável do Cache é inválida".into());
        }
        self.fingerprint.validate()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub project_id: String,
    pub cache_paths: CachePathPlan,
    pub jobs: Vec<CacheJob>,
    pub policy: CacheRepresentationPolicy,
    pub root_bindings: RootBindingPlan,
}

impl CacheRequest {
    pub fn new(
        request_id: impl Into<String>,
        project_id: impl Into<String>,
        cache_paths: CachePathPlan,
        jobs: Vec<CacheJob>,
        policy: CacheRepresentationPolicy,
        root_bindings: RootBindingPlan,
    ) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            project_id: project_id.into(),
            cache_paths,
            jobs,
            policy,
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
            return Err("a solicitação de Cache não contém mídias".into());
        }
        self.policy.validate()?;
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
                    &job.candidate_generation_id,
                    CacheArtifactFormat::Jpeg,
                )
                .map_err(|error| error.to_string())?;
            if let Some(reusable) = &job.reusable {
                self.cache_paths
                    .preview_file(source.media_id(), &reusable.generation_id, reusable.format)
                    .map_err(|error| error.to_string())?;
            }
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
    pub source: CacheMediaSource,
    pub candidate_generation_id: String,
    pub reusable: Option<CacheReusableGeneration>,
}

impl CacheJob {
    pub fn new(
        source: CacheMediaSource,
        candidate_generation_id: impl Into<String>,
        reusable: Option<CacheReusableGeneration>,
    ) -> Result<Self, String> {
        let job = Self {
            source,
            candidate_generation_id: candidate_generation_id.into(),
            reusable,
        };
        job.validate()?;
        Ok(job)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.source.validate()?;
        if !is_safe_identifier(&self.candidate_generation_id) {
            return Err(format!(
                "a geração candidata da mídia {} é inválida",
                self.source.media_id()
            ));
        }
        if let Some(reusable) = &self.reusable {
            reusable.validate()?;
            if reusable.generation_id == self.candidate_generation_id {
                return Err("a geração candidata deve ser nova e imutável".into());
            }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_page_count: Option<u32>,
    pub basic_color_profile: CacheBasicColorProfile,
    pub fingerprint: CacheFingerprint,
}
