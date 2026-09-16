use super::decorations_v11::SheetVisualsV11;
use super::layouts_v10::ProjectPayloadV10;
use super::*;
use crate::{MediaFolder, MediaId};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectDocumentV12 {
    document_type: String,
    schema_version: u32,
    pub(super) project_id: String,
    revision: u64,
    project: ProjectPayloadV10,
    sheet_visuals: Vec<SheetVisualsV11>,
    media_folders: Vec<MediaFolderV12>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaFolderV12 {
    id: String,
    kind: MediaKind,
    name: String,
    media_ids: Vec<String>,
}

impl ProjectDocumentV12 {
    pub(super) fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        let base = ProjectDocumentV11::from_domain(revision)?;
        Ok(Self {
            document_type: base.document_type,
            schema_version: SCHEMA_VERSION_V12,
            project_id: base.project_id,
            revision: base.revision,
            project: base.project,
            sheet_visuals: base.sheet_visuals,
            media_folders: revision
                .project
                .media_folders()
                .iter()
                .map(|folder| MediaFolderV12 {
                    id: folder.id.clone(),
                    kind: folder.kind,
                    name: folder.name.clone(),
                    media_ids: folder.media_ids.iter().map(ToString::to_string).collect(),
                })
                .collect(),
        })
    }

    pub(super) fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE || self.schema_version != SCHEMA_VERSION_V12 {
            return Err(document_failure(DocumentFailure::InvalidProjectDocument));
        }
        let mut revision = ProjectDocumentV11 {
            document_type: self.document_type,
            schema_version: SCHEMA_VERSION_V11,
            project_id: self.project_id,
            revision: self.revision,
            project: self.project,
            sheet_visuals: self.sheet_visuals,
        }
        .into_domain()?;
        let folders = self
            .media_folders
            .into_iter()
            .map(|folder| {
                Ok(MediaFolder {
                    id: folder.id,
                    kind: folder.kind,
                    name: folder.name,
                    media_ids: folder
                        .media_ids
                        .into_iter()
                        .map(|id| parse_uuid_v4(&id).map(MediaId::from_uuid))
                        .collect::<Result<Vec<_>, DecodeFailure>>()?,
                })
            })
            .collect::<Result<Vec<_>, DecodeFailure>>()?;
        revision.project = revision
            .project
            .restore_media_folders(folders)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
        Ok(revision)
    }
}
