use super::layouts_v10::ProjectPayloadV10;
use super::*;
use crate::SheetVisuals;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectDocumentV11 {
    document_type: String,
    schema_version: u32,
    pub(super) project_id: String,
    revision: u64,
    project: ProjectPayloadV10,
    sheet_visuals: Vec<SheetVisualsV11>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SheetVisualsV11 {
    sheet_id: String,
    visuals: SheetVisuals,
}

impl ProjectDocumentV11 {
    pub(super) fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        let base = ProjectDocumentV10::from_domain(revision)?;
        Ok(Self {
            document_type: base.document_type,
            schema_version: SCHEMA_VERSION_V11,
            project_id: base.project_id,
            revision: base.revision,
            project: base.project,
            sheet_visuals: revision
                .project
                .sheets()
                .iter()
                .filter(|sheet| !sheet.visuals().is_default())
                .map(|sheet| SheetVisualsV11 {
                    sheet_id: sheet.id().to_string(),
                    visuals: sheet.visuals().clone(),
                })
                .collect(),
        })
    }

    pub(super) fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE || self.schema_version != SCHEMA_VERSION_V11 {
            return Err(document_failure(DocumentFailure::InvalidProjectDocument));
        }
        let mut revision = ProjectDocumentV10 {
            document_type: self.document_type,
            schema_version: SCHEMA_VERSION_V10,
            project_id: self.project_id,
            revision: self.revision,
            project: self.project,
        }
        .into_domain()?;
        let entries = self
            .sheet_visuals
            .into_iter()
            .map(|item| {
                let id = parse_uuid_v4(&item.sheet_id)?;
                Ok((id, item.visuals))
            })
            .collect::<Result<Vec<_>, DecodeFailure>>()?;
        revision.project = revision
            .project
            .restore_sheet_visuals(entries)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
        Ok(revision)
    }
}
