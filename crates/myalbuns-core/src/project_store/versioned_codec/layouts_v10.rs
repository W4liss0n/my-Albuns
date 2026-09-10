use super::layouts_v8::{LayoutSettingsV8, StoredLayoutV8};
use super::layouts_v9::{ProjectPayloadV9, SheetV9};
use super::*;
use crate::{FavoriteLayout, LayoutFavoriteId};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectDocumentV10 {
    document_type: String,
    schema_version: u32,
    pub(super) project_id: String,
    revision: u64,
    project: ProjectPayloadV10,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectPayloadV10 {
    document: DocumentSettingsV1,
    visual_defaults: VisualDefaultsV1,
    layout_settings: LayoutSettingsV8,
    media: Vec<MediaRefV2>,
    sheets: Vec<SheetV9>,
    favorite_layouts: Vec<FavoriteLayoutV10>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FavoriteLayoutV10 {
    id: LayoutFavoriteId,
    order: u64,
    layout: StoredLayoutV8,
}

impl ProjectDocumentV10 {
    pub(super) fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        let base = ProjectDocumentV9::from_domain(revision)?;
        Ok(Self {
            document_type: base.document_type,
            schema_version: SCHEMA_VERSION_V10,
            project_id: base.project_id,
            revision: base.revision,
            project: ProjectPayloadV10 {
                document: base.project.document,
                visual_defaults: base.project.visual_defaults,
                layout_settings: base.project.layout_settings,
                media: base.project.media,
                sheets: base.project.sheets,
                favorite_layouts: revision
                    .project
                    .favorite_layouts()
                    .iter()
                    .map(|item| FavoriteLayoutV10 {
                        id: item.id,
                        order: item.order,
                        layout: StoredLayoutV8::from_domain(&item.layout),
                    })
                    .collect(),
            },
        })
    }

    pub(super) fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE || self.schema_version != SCHEMA_VERSION_V10 {
            return Err(document_failure(DocumentFailure::InvalidProjectDocument));
        }
        let favorites = self
            .project
            .favorite_layouts
            .into_iter()
            .map(|item| {
                Ok(FavoriteLayout {
                    id: item.id,
                    order: item.order,
                    layout: item.layout.into_domain()?,
                })
            })
            .collect::<Result<Vec<_>, DecodeFailure>>()?;
        let mut revision = ProjectDocumentV9 {
            document_type: self.document_type,
            schema_version: SCHEMA_VERSION_V9,
            project_id: self.project_id,
            revision: self.revision,
            project: ProjectPayloadV9 {
                document: self.project.document,
                visual_defaults: self.project.visual_defaults,
                layout_settings: self.project.layout_settings,
                media: self.project.media,
                sheets: self.project.sheets,
            },
        }
        .into_domain()?;
        revision.project = revision
            .project
            .restore_favorite_layouts(favorites)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
        Ok(revision)
    }
}
