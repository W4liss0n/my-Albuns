use super::*;
use crate::{LayoutRules, LayoutSurface, LayoutSurfaceKind, RectUm};

type Failure = ProjectConfigurationValidationError;

pub(super) fn proportion_is_allowed(w0: u64, h0: u64, w1: i64, h1: i64) -> bool {
    let a = i128::from(w1) * i128::from(h0);
    let b = i128::from(w0) * i128::from(h1);
    a > 0 && b > 0 && 10 * a.max(b) <= 11 * a.min(b)
}

/// All inputs have passed the canonical integer bounds before entering this map.
struct SurfaceMap {
    old: LayoutSurface,
    new: LayoutSurface,
}

impl SurfaceMap {
    fn edge(value: i64, from: i64, to: i64) -> i64 {
        ((i128::from(value) * i128::from(to) + i128::from(from / 2)) / i128::from(from)) as i64
    }

    fn scalar(&self, value: i64) -> Result<i64, Failure> {
        let (from, to) = if i128::from(self.new.width_um) * i128::from(self.old.height_um)
            <= i128::from(self.new.height_um) * i128::from(self.old.width_um)
        {
            (self.old.width_um, self.new.width_um)
        } else {
            (self.old.height_um, self.new.height_um)
        };
        let result = (i128::from(value) * i128::from(to) + i128::from(from / 2)) / i128::from(from);
        if !(0..=i128::from(MAX_SAFE_INTEGER)).contains(&result) {
            return Err(Failure::SheetDimensionsInvalidContent);
        }
        Ok(result as i64)
    }

    fn rect(&self, rect: RectUm) -> Result<RectUm, Failure> {
        let x = Self::edge(rect.x, self.old.width_um, self.new.width_um);
        let y = Self::edge(rect.y, self.old.height_um, self.new.height_um);
        let right = Self::edge(rect.x + rect.width, self.old.width_um, self.new.width_um);
        let bottom = Self::edge(rect.y + rect.height, self.old.height_um, self.new.height_um);
        let result = RectUm {
            x,
            y,
            width: right - x,
            height: bottom - y,
        };
        if x < 0
            || y < 0
            || result.width <= 0
            || result.height <= 0
            || right > self.new.width_um
            || bottom > self.new.height_um
            || (self.old.kind == LayoutSurfaceKind::DoubleSheet
                && side(&rect, self.old.width_um) != side(&result, self.new.width_um))
        {
            return Err(Failure::SheetDimensionsInvalidContent);
        }
        Ok(result)
    }
}

fn side(rect: &RectUm, width: i64) -> u8 {
    if rect.x + rect.width <= width / 2 {
        0
    } else if rect.x >= width / 2 {
        1
    } else {
        2
    }
}

fn surface(sides: ActiveSides, width: i64, height: i64) -> LayoutSurface {
    LayoutSurface {
        kind: if sides == ActiveSides::Both {
            LayoutSurfaceKind::DoubleSheet
        } else {
            LayoutSurfaceKind::SinglePage
        },
        width_um: if sides == ActiveSides::Both {
            width
        } else {
            width / 2
        },
        height_um: height,
    }
}

impl ProjectDocument {
    pub(crate) fn validate_album_information(
        &self,
        information: &AlbumInformation,
        custom: &[crate::CustomLayout],
        sources: &PhotoDimensions,
    ) -> AlbumInformationValidation {
        match self.with_album_information(*information, custom, sources) {
            Ok(candidate) => {
                let mut validation = self.validate_album_information_fields(information);
                if (
                    self.document.sheet_width_um as i64,
                    self.document.sheet_height_um as i64,
                ) != (information.sheet_width_um, information.sheet_height_um)
                    && let Some(impact) = &mut validation.impact
                {
                    let proportion_changed = !dimensions_keep_proportion(
                        self.document.sheet_width_um,
                        self.document.sheet_height_um,
                        information.sheet_width_um,
                        information.sheet_height_um,
                    );
                    impact.dimensional_change = Some(AlbumDimensionChange {
                        proportion_changed,
                        confirmation_key: self.dimension_confirmation_key(
                            &candidate,
                            sources,
                            proportion_changed,
                        ),
                    });
                }
                validation
            }
            Err(errors) => AlbumInformationValidation {
                errors,
                impact: None,
            },
        }
    }

    pub(super) fn resized_composition(
        &self,
        information: &AlbumInformation,
        sources: &PhotoDimensions,
    ) -> Result<Self, Failure> {
        let mut candidate = self.clone();
        let old_width = self.document.sheet_width_um as i64;
        let old_height = self.document.sheet_height_um as i64;
        if old_width == information.sheet_width_um && old_height == information.sheet_height_um {
            return Ok(candidate);
        }
        let same_proportion = dimensions_keep_proportion(
            self.document.sheet_width_um,
            self.document.sheet_height_um,
            information.sheet_width_um,
            information.sheet_height_um,
        );
        let global = SurfaceMap {
            old: surface(ActiveSides::Both, old_width, old_height),
            new: surface(
                ActiveSides::Both,
                information.sheet_width_um,
                information.sheet_height_um,
            ),
        };
        if let FrameBorder::Solid { width_um, .. } = &mut candidate.visual_defaults.frame_border {
            *width_um = global.scalar(*width_um as i64)? as u64;
        }
        let parameters = &mut candidate.layout_settings.parameters;
        parameters.margin_um = global.scalar(parameters.margin_um)?;
        parameters.gap_um = global.scalar(parameters.gap_um)?;
        parameters.minimum_side_um = global.scalar(parameters.minimum_side_um)?;
        if !parameters.is_valid() {
            return Err(Failure::SheetDimensionsInvalidContent);
        }
        for sheet in &mut candidate.sheets {
            let map = SurfaceMap {
                old: surface(sheet.active_sides, old_width, old_height),
                new: surface(
                    sheet.active_sides,
                    information.sheet_width_um,
                    information.sheet_height_um,
                ),
            };
            for frame in &mut sheet.frames {
                let before = RectUm::from(frame.rect);
                let after = map.rect(before.clone())?;
                if !same_proportion && let Some(photo) = &mut frame.photo {
                    let source = sources
                        .get(&photo.media_id)
                        .ok_or(Failure::SheetDimensionsUnknownPhotoSize)?;
                    let t = &mut photo.transform;
                    let transform = crate::model::MediaTransform {
                        pan_x: t.pan_x(),
                        pan_y: t.pan_y(),
                        user_zoom: t.user_zoom(),
                        quarter_turns: t.quarter_turns(),
                        mirror_x: t.mirror_x(),
                        fine_rotation_degrees: t.fine_rotation_degrees(),
                        black_and_white: t.black_and_white(),
                    };
                    let pan =
                        crate::composition::resized_photo_pan(&before, &after, &transform, *source);
                    t.pan_x_scaled = (pan.x * f64::from(TRANSFORM_SCALE)).round() as i32;
                    t.pan_y_scaled = (pan.y * f64::from(TRANSFORM_SCALE)).round() as i32;
                }
                frame.rect = ProjectRect::new(
                    after.x as u64,
                    after.y as u64,
                    after.width as u64,
                    after.height as u64,
                );
                if let FrameStyle::Custom { border, .. } = &mut frame.style {
                    border.width_um = global.scalar(border.width_um as i64)? as u64;
                }
            }
            if let Some(last) = &mut sheet.last_layout {
                let reference = &last.definition.surface;
                if reference.kind == map.old.kind
                    && i128::from(reference.width_um) * i128::from(map.old.height_um)
                        == i128::from(reference.height_um) * i128::from(map.old.width_um)
                {
                    let last_map = SurfaceMap {
                        old: reference.clone(),
                        new: map.new.clone(),
                    };
                    last.definition.positions = last
                        .definition
                        .positions
                        .iter()
                        .cloned()
                        .map(|rect| last_map.rect(rect))
                        .collect::<Result<_, _>>()?;
                    last.definition.surface = map.new;
                    if !LayoutRules::definition_is_valid(&last.definition) {
                        return Err(Failure::SheetDimensionsInvalidContent);
                    }
                }
            }
        }
        Ok(candidate)
    }

    /// Semantic input/output facts, independent of History revision, file names,
    /// folder organization and disposable thumbnail state.
    fn dimension_confirmation_key(
        &self,
        candidate: &Self,
        sources: &PhotoDimensions,
        uses_sources: bool,
    ) -> String {
        use sha2::{Digest, Sha256};
        let facts = |project: &Self| {
            let sheets: Vec<_> = project
                .sheets
                .iter()
                .map(|sheet| {
                    let frames: Vec<_> = sheet
                        .frames
                        .iter()
                        .map(|frame| {
                            let photo = frame.photo.as_ref().map(|photo| {
                                let t = photo.transform;
                                (
                                    photo.media_id.to_string(),
                                    t.pan_x_scaled,
                                    t.pan_y_scaled,
                                    t.user_zoom_scaled,
                                    t.quarter_turns,
                                    t.mirror_x,
                                    t.angle.0,
                                    t.black_and_white,
                                    if uses_sources {
                                        sources.get(&photo.media_id).copied()
                                    } else {
                                        None
                                    },
                                )
                            });
                            let (border, opacity) = frame.resolved_style(&project.visual_defaults);
                            serde_json::json!([
                                frame.id.to_string(),
                                RectUm::from(frame.rect),
                                photo,
                                matches!(frame.style, FrameStyle::Album),
                                border.width_um,
                                border.rgb.channels(),
                                opacity
                            ])
                        })
                        .collect();
                    serde_json::json!([
                        sheet.id.to_string(),
                        surface(
                            sheet.active_sides,
                            project.document.sheet_width_um as i64,
                            project.document.sheet_height_um as i64
                        ),
                        sheet.layout_locked,
                        sheet.last_layout,
                        sheet.visuals,
                        frames
                    ])
                })
                .collect();
            let border = match project.visual_defaults.frame_border {
                FrameBorder::None => None,
                FrameBorder::Solid { rgb, width_um } => Some((rgb.channels(), width_um)),
            };
            serde_json::json!([sheets, project.layout_settings, border])
        };
        format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(facts(self), facts(candidate)))
                    .expect("dimensional facts serialize")
            )
        )
    }
}
