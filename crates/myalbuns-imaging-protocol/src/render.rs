use myalbuns_core::MediaId;
use myalbuns_paths::NativePathDto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSource {
    media_id: MediaId,
    source_path: NativePathDto,
}

impl RenderSource {
    pub fn new(media_id: MediaId, source_path: impl Into<NativePathDto>) -> Result<Self, String> {
        let source = Self {
            media_id,
            source_path: source_path.into(),
        };
        source.validate()?;
        Ok(source)
    }

    pub fn media_id(&self) -> MediaId {
        self.media_id
    }

    pub fn source_path(&self) -> &std::path::Path {
        self.source_path.as_path()
    }

    fn validate(&self) -> Result<(), String> {
        if !self.source_path().is_absolute() {
            return Err(format!(
                "o caminho da mídia {} não é absoluto",
                self.media_id
            ));
        }
        Ok(())
    }
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
        validate_media_identity_path(&self.media_id, self.source_path())?;
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

fn validate_media_identity_path(
    media_id: &str,
    source_path: &std::path::Path,
) -> Result<(), String> {
    if media_id.trim().is_empty() {
        return Err("a identidade de mídia é inválida".into());
    }
    if !source_path.is_absolute() {
        return Err(format!("o caminho da mídia {media_id} não é absoluto"));
    }
    Ok(())
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
