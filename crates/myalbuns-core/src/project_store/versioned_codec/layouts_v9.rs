use super::layouts_v8::{LayoutSettingsV8, ProjectPayloadV8, SheetV8, StoredLayoutV8};
use super::*;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectDocumentV9 {
    pub(super) document_type: String,
    pub(super) schema_version: u32,
    pub(super) project_id: String,
    pub(super) revision: u64,
    pub(super) project: ProjectPayloadV9,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectPayloadV9 {
    pub(super) document: DocumentSettingsV1,
    pub(super) visual_defaults: VisualDefaultsV1,
    pub(super) layout_settings: LayoutSettingsV8,
    pub(super) media: Vec<MediaRefV2>,
    pub(super) sheets: Vec<SheetV9>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SheetV9 {
    id: String,
    active_sides: ActiveSidesV1,
    frames: Vec<FrameV7>,
    #[serde(deserialize_with = "Option::deserialize")]
    last_layout: Option<StoredLayoutV8>,
    layout_locked: bool,
}

impl ProjectDocumentV9 {
    pub(super) fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        let base = ProjectDocumentV8::from_domain(revision)?;
        Ok(Self {
            document_type: base.document_type,
            schema_version: SCHEMA_VERSION_V9,
            project_id: base.project_id,
            revision: base.revision,
            project: ProjectPayloadV9 {
                document: base.project.document,
                visual_defaults: base.project.visual_defaults,
                layout_settings: base.project.layout_settings,
                media: base.project.media,
                sheets: base
                    .project
                    .sheets
                    .into_iter()
                    .zip(revision.project.sheets())
                    .map(|(sheet, source)| SheetV9 {
                        id: sheet.id,
                        active_sides: sheet.active_sides,
                        frames: sheet.frames,
                        last_layout: sheet.last_layout,
                        layout_locked: source.layout_locked(),
                    })
                    .collect(),
            },
        })
    }

    pub(super) fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE || self.schema_version != SCHEMA_VERSION_V9 {
            return Err(document_failure(DocumentFailure::InvalidProjectDocument));
        }
        let mut locks = Vec::with_capacity(self.project.sheets.len());
        let sheets = self
            .project
            .sheets
            .into_iter()
            .map(|sheet| {
                locks.push(sheet.layout_locked);
                SheetV8 {
                    id: sheet.id,
                    active_sides: sheet.active_sides,
                    frames: sheet.frames,
                    last_layout: sheet.last_layout,
                }
            })
            .collect();
        let mut revision = ProjectDocumentV8 {
            document_type: self.document_type,
            schema_version: SCHEMA_VERSION_V8,
            project_id: self.project_id,
            revision: self.revision,
            project: ProjectPayloadV8 {
                document: self.project.document,
                visual_defaults: self.project.visual_defaults,
                layout_settings: self.project.layout_settings,
                media: self.project.media,
                sheets,
            },
        }
        .into_domain()?;
        revision.project = revision
            .project
            .restore_layout_locks(locks)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
        Ok(revision)
    }
}
