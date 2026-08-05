use myalbuns_core::ComposedOutputUnit;
use myalbuns_paths::{NativePathDto, RootBindingPlan};
use serde::{Deserialize, Serialize};

use crate::{IMAGING_PROTOCOL_VERSION, is_safe_identifier};

pub fn has_jpeg_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jpg"))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub project_id: String,
    pub revision: u64,
    pub prepared_output_path: NativePathDto,
    pub unit: ComposedOutputUnit,
    pub dpi: u32,
    pub sources: Vec<MediaSource>,
    pub root_bindings: RootBindingPlan,
}

impl ImagingRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: impl Into<String>,
        project_id: impl Into<String>,
        revision: u64,
        prepared_output_path: NativePathDto,
        unit: ComposedOutputUnit,
        dpi: u32,
        sources: Vec<MediaSource>,
        root_bindings: RootBindingPlan,
    ) -> Result<Self, String> {
        let request = Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            project_id: project_id.into(),
            revision,
            prepared_output_path,
            unit,
            dpi,
            sources,
            root_bindings,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn prepared_output_path(&self) -> &std::path::Path {
        self.prepared_output_path.as_path()
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
        if self.revision > 9_007_199_254_740_991 {
            return Err("a Revisão visível está fora do intervalo interoperável".into());
        }
        if !self.prepared_output_path().is_absolute() {
            return Err("o caminho da preparação não é absoluto".into());
        }
        if !has_jpeg_extension(self.prepared_output_path()) {
            return Err("a preparação da Exportação precisa usar a extensão .jpg".into());
        }
        self.root_bindings
            .validate()
            .map_err(|error| format!("o plano de raízes é inválido: {error}"))?;
        if !self.root_bindings.covers(self.prepared_output_path()) {
            return Err("o caminho da preparação não pertence ao plano de raízes".into());
        }
        validate_render_content(&self.unit, self.dpi, &self.sources)?;
        for source in &self.sources {
            if !self.root_bindings.covers(source.source_path()) {
                return Err(format!(
                    "a raiz da mídia {} não pertence ao plano da operação",
                    source.media_id()
                ));
            }
        }
        Ok(())
    }
}

pub fn validate_render_content(
    unit: &ComposedOutputUnit,
    dpi: u32,
    sources: &[MediaSource],
) -> Result<(), String> {
    if !(1..=1_200).contains(&dpi) {
        return Err("a resolução da Exportação é inválida".into());
    }
    unit.validate()
        .map_err(|error| format!("unidade composta inválida: {error}"))?;
    let required_media = unit
        .sheet
        .referenced_media_ids()
        .collect::<std::collections::HashSet<_>>();
    let mut supplied_media = std::collections::HashSet::new();
    for source in sources {
        source.validate()?;
        if !supplied_media.insert(source.media_id()) {
            return Err("a Exportação contém mídia duplicada".into());
        }
    }
    if supplied_media != required_media {
        return Err("as fontes da Exportação não correspondem às mídias da Lâmina".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSource {
    media_id: String,
    source_path: NativePathDto,
    source_bytes: u64,
    source_sha256: String,
}

impl MediaSource {
    pub fn new(
        media_id: impl Into<String>,
        source_path: impl Into<NativePathDto>,
        source_bytes: u64,
        source_sha256: impl Into<String>,
    ) -> Result<Self, String> {
        let source = Self {
            media_id: media_id.into(),
            source_path: source_path.into(),
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
        self.source_path.as_path()
    }

    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.media_id.trim().is_empty() {
            return Err("a identidade de mídia é inválida".into());
        }
        if !self.source_path().is_absolute() {
            return Err(format!(
                "o caminho da mídia {} não é absoluto",
                self.media_id
            ));
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
pub struct RenderCompletion {
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: u32,
    pub source_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub output_sha256: String,
}
