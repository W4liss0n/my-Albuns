use std::{collections::HashSet, path::Path};

use myalbuns_paths::{CacheArtifactFormat, CachePathPlan, NativePathDto, RootBindingPlan};
use serde::{Deserialize, Serialize};

use crate::{
    CacheFingerprint, CacheRepresentationPolicy, CacheReusableGeneration, IMAGING_PROTOCOL_VERSION,
    is_safe_identifier,
};

/// Correlates a selected source before Core creates any creative MediaId.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhotoImportSourceId(String);

impl PhotoImportSourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err("a chave da fonte de importação é inválida".into());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoImportCandidate {
    pub source_id: PhotoImportSourceId,
    pub source_path: NativePathDto,
    pub generation_id: String,
}

impl PhotoImportCandidate {
    pub fn path(&self) -> &Path {
        self.source_path.as_path()
    }
}

pub const PHOTO_IMPORT_PROCESS_BATCH: usize = 32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoImportRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub attempt_id: String,
    pub project_id: String,
    pub cache_paths: CachePathPlan,
    pub candidates: Vec<PhotoImportCandidate>,
    pub policy: CacheRepresentationPolicy,
    pub root_bindings: RootBindingPlan,
}

impl PhotoImportRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != IMAGING_PROTOCOL_VERSION
            || !is_safe_identifier(&self.request_id)
            || !is_safe_identifier(&self.attempt_id)
            || self.project_id.trim().is_empty()
            || self.candidates.is_empty()
            || self.candidates.len() > PHOTO_IMPORT_PROCESS_BATCH
        {
            return Err("a solicitação de importação é inválida".into());
        }
        self.policy.validate()?;
        self.cache_paths
            .validate()
            .map_err(|error| error.to_string())?;
        self.root_bindings
            .validate()
            .map_err(|error| error.to_string())?;
        if !self.root_bindings.covers(self.cache_paths.root()) {
            return Err("a raiz do Cache não pertence à tentativa".into());
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        for candidate in &self.candidates {
            if !is_safe_identifier(candidate.source_id.as_str())
                || !is_safe_identifier(&candidate.generation_id)
                || !ids.insert(&candidate.source_id)
                || !paths.insert(candidate.path())
                || !self.root_bindings.covers(candidate.path())
            {
                return Err(
                    "a solicitação contém fonte duplicada, inválida ou fora da tentativa".into(),
                );
            }
            self.cache_paths
                .import_preview_file(
                    &self.attempt_id,
                    candidate.source_id.as_str(),
                    &candidate.generation_id,
                    CacheArtifactFormat::Jpeg,
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedPhotoDimensions {
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImportedPhotoPreview {
    Prepared {
        generation: Box<CacheReusableGeneration>,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PhotoImportOutcome {
    Validated {
        dimensions: ImportedPhotoDimensions,
        fingerprint: CacheFingerprint,
        preview: ImportedPhotoPreview,
    },
    /// Original import has a broader policy than Cache decoding; the existing
    /// inspector must run under the same frozen plan when this terminal is returned.
    InspectionRequired { reason: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPhotoImport {
    pub source_id: PhotoImportSourceId,
    pub outcome: PhotoImportOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoImportCompletion {
    pub photos: Vec<PreparedPhotoImport>,
}

impl PhotoImportCompletion {
    pub fn validate_for(&self, request: &PhotoImportRequest) -> Result<(), String> {
        if self.photos.len() != request.candidates.len() {
            return Err("a conclusão não corresponde ao lote de importação".into());
        }
        let mut seen = HashSet::new();
        for photo in &self.photos {
            let candidate = request
                .candidates
                .iter()
                .find(|candidate| candidate.source_id == photo.source_id)
                .ok_or("a conclusão contém uma fonte não solicitada")?;
            if !seen.insert(&photo.source_id) {
                return Err("a conclusão repete uma fonte de importação".into());
            }
            if let PhotoImportOutcome::Validated {
                dimensions,
                fingerprint,
                preview,
            } = &photo.outcome
            {
                if dimensions.width_px == 0
                    || dimensions.height_px == 0
                    || u64::from(dimensions.width_px) * u64::from(dimensions.height_px)
                        > request.policy.max_decoded_pixels
                {
                    return Err("as dimensões da Foto validada são inválidas".into());
                }
                fingerprint.validate()?;
                if let ImportedPhotoPreview::Prepared { generation } = preview {
                    generation.validate()?;
                    if generation.generation_id != candidate.generation_id
                        || generation.fingerprint != *fingerprint
                        || generation.width_px > dimensions.width_px
                        || generation.height_px > dimensions.height_px
                        || generation.width_px > request.policy.max_edge_px
                        || generation.height_px > request.policy.max_edge_px
                    {
                        return Err("a prévia não corresponde à fonte e geração solicitadas".into());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CacheArtifactProperties, CacheBasicColorProfile, ImagingCommand, decode_command,
        encode_command,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext, project_data_namespace};

    fn request(root: &std::path::Path) -> PhotoImportRequest {
        let paths = AppPaths::from_roots(&root.join("roaming"), &root.join("local"))
            .project_cache(&project_data_namespace("import-test"))
            .unwrap();
        let mut context = OperationPathContext::new();
        context.capture(paths.root()).unwrap();
        let source_path = root.join("photo.jpg");
        context.capture(&source_path).unwrap();
        PhotoImportRequest {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: "request-1".into(),
            attempt_id: "attempt-1".into(),
            project_id: "import-test".into(),
            cache_paths: paths,
            candidates: vec![PhotoImportCandidate {
                source_id: PhotoImportSourceId::new("source-1").unwrap(),
                source_path: source_path.into(),
                generation_id: "generation-1".into(),
            }],
            policy: CacheRepresentationPolicy::measured_v1(),
            root_bindings: context.freeze(),
        }
    }

    fn completion() -> PhotoImportCompletion {
        let fingerprint = CacheFingerprint::sha256_full_file(300, "a".repeat(64)).unwrap();
        let generation = CacheReusableGeneration::new(
            "generation-1",
            CacheArtifactProperties::new(
                CacheArtifactFormat::Jpeg,
                100,
                50,
                200,
                Some(1),
                None,
                CacheBasicColorProfile::Srgb,
            ),
            fingerprint.clone(),
        )
        .unwrap();
        PhotoImportCompletion {
            photos: vec![PreparedPhotoImport {
                source_id: PhotoImportSourceId::new("source-1").unwrap(),
                outcome: PhotoImportOutcome::Validated {
                    dimensions: ImportedPhotoDimensions {
                        width_px: 100,
                        height_px: 50,
                    },
                    fingerprint,
                    preview: ImportedPhotoPreview::Prepared {
                        generation: Box::new(generation),
                    },
                },
            }],
        }
    }

    #[test]
    fn source_keys_round_trip_without_creative_media_ids_and_requests_are_bounded() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        request.validate().unwrap();
        let command = ImagingCommand::PreparePhotoImport(request.clone());
        let encoded = encode_command(&command).unwrap();
        assert_eq!(decode_command(&encoded).unwrap(), command);
        assert!(!String::from_utf8(encoded).unwrap().contains("mediaId"));
        let mut duplicate = request.clone();
        duplicate.candidates.push(duplicate.candidates[0].clone());
        assert!(duplicate.validate().is_err());
        let mut oversized = request;
        oversized.candidates = (0..=PHOTO_IMPORT_PROCESS_BATCH)
            .map(|index| PhotoImportCandidate {
                source_id: PhotoImportSourceId::new(format!("source-{index}")).unwrap(),
                source_path: root.path().join(format!("source-{index}.jpg")).into(),
                generation_id: format!("generation-{index}"),
            })
            .collect();
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn terminals_must_match_every_requested_source_generation_and_dimensions() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        let valid = completion();
        valid.validate_for(&request).unwrap();
        let mut missing = valid.clone();
        missing.photos.clear();
        assert!(missing.validate_for(&request).is_err());
        let mut foreign = valid.clone();
        foreign.photos[0].source_id = PhotoImportSourceId::new("foreign").unwrap();
        assert!(foreign.validate_for(&request).is_err());
        let mut malformed = valid.clone();
        if let PhotoImportOutcome::Validated { dimensions, .. } = &mut malformed.photos[0].outcome {
            dimensions.width_px = 0;
        }
        assert!(malformed.validate_for(&request).is_err());
        let mut oversized = valid.clone();
        if let PhotoImportOutcome::Validated { dimensions, .. } = &mut oversized.photos[0].outcome {
            dimensions.width_px = u32::MAX;
        }
        assert!(oversized.validate_for(&request).is_err());
        let mut generation = valid;
        if let PhotoImportOutcome::Validated {
            preview: ImportedPhotoPreview::Prepared { generation },
            ..
        } = &mut generation.photos[0].outcome
        {
            generation.generation_id = "foreign".into();
        }
        assert!(generation.validate_for(&request).is_err());
    }
}
