use super::*;
use crate::{
    LayoutDefinition, LayoutOrigin, LayoutParameters, LayoutPermission, LayoutScope,
    LayoutSettings, LayoutSurface, LayoutSurfaceKind, RectUm, StoredLayout,
};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectDocumentV8 {
    pub(super) document_type: String,
    pub(super) schema_version: u32,
    pub(super) project_id: String,
    pub(super) revision: u64,
    pub(super) project: ProjectPayloadV8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProjectPayloadV8 {
    pub(super) document: DocumentSettingsV1,
    pub(super) visual_defaults: VisualDefaultsV1,
    pub(super) layout_settings: LayoutSettingsV8,
    pub(super) media: Vec<MediaRefV2>,
    pub(super) sheets: Vec<SheetV8>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SheetV8 {
    pub(super) id: String,
    pub(super) active_sides: ActiveSidesV1,
    pub(super) frames: Vec<FrameV7>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub(super) last_layout: Option<StoredLayoutV8>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LayoutSettingsV8 {
    pub(super) permission: LayoutPermission,
    pub(super) margin_um: i64,
    pub(super) gap_um: i64,
    pub(super) minimum_side_um: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredLayoutV8 {
    definition: LayoutDefinitionV8,
    pub(super) origin: LayoutOrigin,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutDefinitionV8 {
    surface: LayoutSurfaceV8,
    scope: LayoutScope,
    positions: Vec<RectV3>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutSurfaceV8 {
    #[serde(rename = "type")]
    kind: LayoutSurfaceKind,
    width_um: i64,
    height_um: i64,
}

impl ProjectDocumentV8 {
    pub(super) fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        // Frame, Photo, style and native-path encodings retain their existing
        // versioned contract. Only the Layout payload is added in v8.
        let base = ProjectDocumentV7::from_domain(revision)?;
        let settings = revision.project.layout_settings();
        Ok(Self {
            document_type: base.document_type,
            schema_version: SCHEMA_VERSION_V8,
            project_id: base.project_id,
            revision: base.revision,
            project: ProjectPayloadV8 {
                document: base.project.document,
                visual_defaults: base.project.visual_defaults,
                layout_settings: LayoutSettingsV8 {
                    permission: settings.permission,
                    margin_um: settings.parameters.margin_um,
                    gap_um: settings.parameters.gap_um,
                    minimum_side_um: settings.parameters.minimum_side_um,
                },
                media: base.project.media,
                sheets: base
                    .project
                    .sheets
                    .into_iter()
                    .zip(revision.project.sheets())
                    .map(|(sheet, source)| SheetV8 {
                        id: sheet.id,
                        active_sides: sheet.active_sides,
                        frames: sheet.frames,
                        last_layout: source.last_layout().map(StoredLayoutV8::from_domain),
                    })
                    .collect(),
            },
        })
    }

    pub(super) fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE || self.schema_version != SCHEMA_VERSION_V8 {
            return Err(document_failure(DocumentFailure::InvalidProjectDocument));
        }
        let settings = self.project.layout_settings;
        let settings = LayoutSettings {
            permission: settings.permission,
            parameters: LayoutParameters {
                margin_um: settings.margin_um,
                gap_um: settings.gap_um,
                minimum_side_um: settings.minimum_side_um,
            },
        };
        let mut last_layouts = Vec::with_capacity(self.project.sheets.len());
        let mut sheets = Vec::with_capacity(self.project.sheets.len());
        for sheet in self.project.sheets {
            last_layouts.push(
                sheet
                    .last_layout
                    .map(StoredLayoutV8::into_domain)
                    .transpose()?,
            );
            sheets.push(SheetWithFrames {
                id: sheet.id,
                active_sides: sheet.active_sides,
                frames: sheet.frames,
            });
        }
        let mut revision = map_document(ProjectDocumentV7 {
            document_type: self.document_type,
            schema_version: SCHEMA_VERSION_V7,
            project_id: self.project_id,
            revision: self.revision,
            project: FramedProjectPayload {
                document: self.project.document,
                visual_defaults: self.project.visual_defaults,
                media: self.project.media,
                sheets,
            },
        })?;
        revision.project = revision
            .project
            .restore_layout_state(settings, last_layouts)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
        Ok(revision)
    }
}

impl StoredLayoutV8 {
    fn from_domain(layout: &StoredLayout) -> Self {
        let definition = &layout.definition;
        Self {
            origin: layout.origin,
            definition: LayoutDefinitionV8 {
                surface: LayoutSurfaceV8 {
                    kind: definition.surface.kind,
                    width_um: definition.surface.width_um,
                    height_um: definition.surface.height_um,
                },
                scope: definition.scope,
                positions: definition
                    .positions
                    .iter()
                    .map(|r| RectV3 {
                        x: r.x as u64,
                        y: r.y as u64,
                        width: r.width as u64,
                        height: r.height as u64,
                    })
                    .collect(),
            },
        }
    }

    fn into_domain(self) -> Result<StoredLayout, DecodeFailure> {
        let positions = self
            .definition
            .positions
            .into_iter()
            .map(|r| {
                let signed = |n| {
                    i64::try_from(n)
                        .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))
                };
                Ok(RectUm {
                    x: signed(r.x)?,
                    y: signed(r.y)?,
                    width: signed(r.width)?,
                    height: signed(r.height)?,
                })
            })
            .collect::<Result<Vec<_>, DecodeFailure>>()?;
        Ok(StoredLayout {
            origin: self.origin,
            definition: LayoutDefinition {
                surface: LayoutSurface {
                    kind: self.definition.surface.kind,
                    width_um: self.definition.surface.width_um,
                    height_um: self.definition.surface.height_um,
                },
                scope: self.definition.scope,
                positions,
            },
        })
    }
}
