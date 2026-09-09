use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use myalbuns_paths::validate_external_path;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::model::{
    MediaId, MediaKind, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN,
    PhotoDropTarget, PhotoPlacementMode, ProjectedBackground, ProjectedBackgroundContent,
    ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent, ProjectedVisualDefaults,
    SheetInsertionPosition,
};

pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

mod frame_clipboard;
mod layouts;
pub(crate) use frame_clipboard::FrameClipboard;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DisplayUnit {
    Mm,
    Cm,
    In,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveSides {
    Both,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb([u8; 3]);

impl Rgb {
    pub const WHITE: Self = Self([255, 255, 255]);

    pub fn parse_canonical(source: &str) -> Option<Self> {
        let bytes = source.as_bytes();
        if bytes.len() != 7
            || bytes[0] != b'#'
            || !bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte))
        {
            return None;
        }
        let channel = |start| u8::from_str_radix(&source[start..start + 2], 16).ok();
        Some(Self::new([channel(1)?, channel(3)?, channel(5)?]))
    }

    pub fn canonical_hex(self) -> String {
        let [red, green, blue] = self.channels();
        format!("#{red:02X}{green:02X}{blue:02X}")
    }

    pub fn channels(self) -> [u8; 3] {
        self.0
    }

    pub(crate) fn new(channels: [u8; 3]) -> Self {
        Self(channels)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackgroundContent {
    Color { rgb: Rgb },
    Media { media_id: Uuid },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Background {
    BothSides {
        both: BackgroundContent,
    },
    PerSide {
        left: BackgroundContent,
        right: BackgroundContent,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverlayContent {
    Media { media_id: Uuid },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Overlay {
    BothSides {
        both: Option<OverlayContent>,
    },
    PerSide {
        left: Option<OverlayContent>,
        right: Option<OverlayContent>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameBorder {
    None,
    Solid { rgb: Rgb, width_um: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualDefaults {
    background: Background,
    overlay: Overlay,
    frame_border: FrameBorder,
}

impl VisualDefaults {
    pub fn background(&self) -> &Background {
        &self.background
    }

    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    pub fn frame_border(&self) -> &FrameBorder {
        &self.frame_border
    }

    pub(crate) fn new(background: Background, overlay: Overlay, frame_border: FrameBorder) -> Self {
        Self {
            background,
            overlay,
            frame_border,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSettings {
    display_unit: DisplayUnit,
    sheet_width_um: u64,
    sheet_height_um: u64,
    dpi: u32,
    bleed_um: u64,
    safety_um: u64,
}

impl DocumentSettings {
    pub fn display_unit(&self) -> DisplayUnit {
        self.display_unit
    }

    pub fn sheet_width_um(&self) -> u64 {
        self.sheet_width_um
    }

    pub fn sheet_height_um(&self) -> u64 {
        self.sheet_height_um
    }

    pub fn dpi(&self) -> u32 {
        self.dpi
    }

    pub fn bleed_um(&self) -> u64 {
        self.bleed_um
    }

    pub fn safety_um(&self) -> u64 {
        self.safety_um
    }

    pub(crate) fn new(
        display_unit: DisplayUnit,
        sheet_width_um: u64,
        sheet_height_um: u64,
        dpi: u32,
        bleed_um: u64,
        safety_um: u64,
    ) -> Self {
        Self {
            display_unit,
            sheet_width_um,
            sheet_height_um,
            dpi,
            bleed_um,
            safety_um,
        }
    }

    pub(crate) fn neutral() -> Self {
        Self::new(DisplayUnit::Mm, 600_000, 300_000, 300, 3_000, 3_000)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRef {
    id: Uuid,
    kind: MediaKind,
    path: PathBuf,
}

const TRANSFORM_SCALE: f32 = 1_000_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectRect {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

impl ProjectRect {
    pub const fn x(&self) -> u64 {
        self.x
    }

    pub const fn y(&self) -> u64 {
        self.y
    }

    pub const fn width(&self) -> u64 {
        self.width
    }

    pub const fn height(&self) -> u64 {
        self.height
    }

    pub(crate) const fn new(x: u64, y: u64, width: u64, height: u64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(self, x: u64, y: u64) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectPhotoTransform {
    pan_x_scaled: i32,
    pan_y_scaled: i32,
    user_zoom_scaled: u32,
    quarter_turns: i8,
    mirror_x: bool,
    angle: PhotoFineAngle,
    black_and_white: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PhotoFineAngle(i16);

impl PhotoFineAngle {
    fn new(tenths: i16) -> Result<Self, ()> {
        (-450..=450)
            .contains(&tenths)
            .then_some(Self(tenths))
            .ok_or(())
    }
}

impl Default for ProjectPhotoTransform {
    fn default() -> Self {
        Self::new(0.0, 0.0, 1.0).expect("the neutral Photo transform is valid")
    }
}

impl ProjectPhotoTransform {
    pub fn pan_x(&self) -> f32 {
        self.pan_x_scaled as f32 / TRANSFORM_SCALE
    }

    pub fn pan_y(&self) -> f32 {
        self.pan_y_scaled as f32 / TRANSFORM_SCALE
    }

    pub fn user_zoom(&self) -> f32 {
        self.user_zoom_scaled as f32 / TRANSFORM_SCALE
    }

    pub const fn quarter_turns(&self) -> i8 {
        self.quarter_turns
    }

    pub const fn mirror_x(&self) -> bool {
        self.mirror_x
    }

    pub const fn angle_tenths(&self) -> i16 {
        self.angle.0
    }

    pub const fn black_and_white(&self) -> bool {
        self.black_and_white
    }

    pub(crate) fn with_black_and_white(mut self, enabled: bool) -> Self {
        self.black_and_white = enabled;
        self
    }

    pub fn fine_rotation_degrees(&self) -> f32 {
        self.angle.0 as f32 / 10.0
    }

    pub(crate) fn with_fine_rotation(mut self, tenths: i16) -> Result<Self, ()> {
        self.angle = PhotoFineAngle::new(tenths)?;
        Ok(self)
    }

    pub(crate) fn with_orientation(
        mut self,
        quarter_turns: i8,
        mirror_x: bool,
    ) -> Result<Self, ()> {
        if !(0..=3).contains(&quarter_turns) {
            return Err(());
        }
        self.quarter_turns = quarter_turns;
        self.mirror_x = mirror_x;
        Ok(self)
    }

    pub(crate) fn new(pan_x: f32, pan_y: f32, user_zoom: f32) -> Result<Self, ()> {
        if !pan_x.is_finite()
            || !pan_y.is_finite()
            || !user_zoom.is_finite()
            || !(PHOTO_PAN_MIN..=PHOTO_PAN_MAX).contains(&pan_x)
            || !(PHOTO_PAN_MIN..=PHOTO_PAN_MAX).contains(&pan_y)
            || !(PHOTO_ZOOM_MIN..=PHOTO_ZOOM_MAX).contains(&user_zoom)
        {
            return Err(());
        }
        Ok(Self {
            pan_x_scaled: (pan_x * TRANSFORM_SCALE).round() as i32,
            pan_y_scaled: (pan_y * TRANSFORM_SCALE).round() as i32,
            user_zoom_scaled: (user_zoom * TRANSFORM_SCALE).round() as u32,
            quarter_turns: 0,
            mirror_x: false,
            angle: PhotoFineAngle::default(),
            black_and_white: false,
        })
    }

    fn advanced(self, delta_pan_x: f32, delta_pan_y: f32, delta_zoom: f32) -> Result<Self, ()> {
        if !delta_pan_x.is_finite() || !delta_pan_y.is_finite() || !delta_zoom.is_finite() {
            return Err(());
        }
        let mut next = Self::new(
            (self.pan_x() + delta_pan_x).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX),
            (self.pan_y() + delta_pan_y).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX),
            (self.user_zoom() + delta_zoom).clamp(PHOTO_ZOOM_MIN, PHOTO_ZOOM_MAX),
        )?;
        next.quarter_turns = self.quarter_turns;
        next.mirror_x = self.mirror_x;
        next.angle = self.angle;
        next.black_and_white = self.black_and_white;
        Ok(next)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectPhoto {
    media_id: Uuid,
    transform: ProjectPhotoTransform,
}

impl ProjectPhoto {
    pub const fn media_id(&self) -> Uuid {
        self.media_id
    }

    pub const fn transform(&self) -> ProjectPhotoTransform {
        self.transform
    }

    pub(crate) const fn new(media_id: Uuid, transform: ProjectPhotoTransform) -> Self {
        Self {
            media_id,
            transform,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFrame {
    id: Uuid,
    rect: ProjectRect,
    photo: Option<ProjectPhoto>,
    style: FrameStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FrameStyle {
    Album,
    Custom {
        border: FrameBorderValues,
        opacity_percent: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameBorderValues {
    pub(crate) rgb: Rgb,
    pub(crate) width_um: u64,
}

impl ProjectFrame {
    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub const fn rect(&self) -> ProjectRect {
        self.rect
    }

    pub fn photo(&self) -> Option<&ProjectPhoto> {
        self.photo.as_ref()
    }

    pub(crate) const fn new(id: Uuid, rect: ProjectRect, photo: Option<ProjectPhoto>) -> Self {
        Self {
            id,
            rect,
            photo,
            style: FrameStyle::Album,
        }
    }

    fn resolved_style(&self, defaults: &VisualDefaults) -> (FrameBorderValues, u8) {
        match &self.style {
            FrameStyle::Album => (
                match defaults.frame_border() {
                    FrameBorder::None => FrameBorderValues {
                        rgb: Rgb::new([0, 0, 0]),
                        width_um: 0,
                    },
                    FrameBorder::Solid { rgb, width_um } => FrameBorderValues {
                        rgb: *rgb,
                        width_um: *width_um,
                    },
                },
                100,
            ),
            FrameStyle::Custom {
                border,
                opacity_percent,
            } => (*border, *opacity_percent),
        }
    }

    pub(crate) fn style(&self) -> &FrameStyle {
        &self.style
    }

    pub(crate) fn with_style(mut self, style: FrameStyle) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn projected_style(&self, defaults: &VisualDefaults) -> crate::ProjectedFrameStyle {
        let (border, opacity_percent) = self.resolved_style(defaults);
        crate::ProjectedFrameStyle {
            source: match self.style {
                FrameStyle::Album => crate::FrameStyleSource::Album,
                FrameStyle::Custom { .. } => crate::FrameStyleSource::Custom,
            },
            border_rgb: border.rgb.canonical_hex(),
            border_width_um: border.width_um,
            opacity_percent,
        }
    }
}

impl MediaRef {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn new(id: Uuid, kind: MediaKind, path: PathBuf) -> Self {
        Self { id, kind, path }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSheet {
    id: Uuid,
    active_sides: ActiveSides,
    frames: Vec<ProjectFrame>,
    last_layout: Option<crate::StoredLayout>,
}

impl ProjectSheet {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn active_sides(&self) -> ActiveSides {
        self.active_sides
    }

    pub fn frames(&self) -> &[ProjectFrame] {
        &self.frames
    }

    pub(crate) fn new(id: Uuid, active_sides: ActiveSides) -> Self {
        Self {
            id,
            active_sides,
            frames: Vec::new(),
            last_layout: None,
        }
    }

    pub(crate) fn with_frames(
        id: Uuid,
        active_sides: ActiveSides,
        frames: Vec<ProjectFrame>,
    ) -> Self {
        Self {
            id,
            active_sides,
            frames,
            last_layout: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDocument {
    document: DocumentSettings,
    visual_defaults: VisualDefaults,
    media: Vec<MediaRef>,
    sheets: Vec<ProjectSheet>,
    layout_settings: crate::LayoutSettings,
}

impl ProjectDocument {
    pub fn document(&self) -> &DocumentSettings {
        &self.document
    }

    pub fn visual_defaults(&self) -> &VisualDefaults {
        &self.visual_defaults
    }

    pub fn media(&self) -> &[MediaRef] {
        &self.media
    }

    pub fn sheets(&self) -> &[ProjectSheet] {
        &self.sheets
    }

    pub(crate) fn new(
        document: DocumentSettings,
        visual_defaults: VisualDefaults,
        media: Vec<MediaRef>,
        sheets: Vec<ProjectSheet>,
    ) -> Self {
        Self {
            document,
            visual_defaults,
            media,
            sheets,
            layout_settings: crate::LayoutSettings::default(),
        }
    }

    pub(crate) fn with_dpi(&self, dpi: u32) -> Result<Self, ()> {
        let mut candidate = self.clone();
        candidate.document.dpi = dpi;
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn with_visual_defaults(
        &self,
        visual_defaults: ProjectedVisualDefaults,
    ) -> Result<Self, ()> {
        let mut candidate = self.clone();
        candidate.visual_defaults = visual_defaults_from_projection(self, visual_defaults)?;
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn with_relinked_media(&self, media_id: Uuid, path: PathBuf) -> Result<Self, ()> {
        let mut candidate = self.clone();
        let media = candidate
            .media
            .iter_mut()
            .find(|media| media.id == media_id)
            .ok_or(())?;
        media.path = path;
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn with_added_sheet(
        &self,
        anchor_sheet_id: Uuid,
        position: SheetInsertionPosition,
    ) -> Result<(Self, Uuid), ()> {
        let mut candidate = self.clone();
        let anchor_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == anchor_sheet_id)
            .ok_or(())?;
        let insertion_index = match position {
            SheetInsertionPosition::Before => anchor_index,
            SheetInsertionPosition::After => anchor_index + 1,
        };
        let sheet_id = Uuid::new_v4();
        candidate.sheets.insert(
            insertion_index,
            ProjectSheet::new(sheet_id, ActiveSides::Both),
        );
        validate_project_state(&candidate)?;
        Ok((candidate, sheet_id))
    }

    pub(crate) fn with_deleted_sheet(&self, sheet_id: Uuid) -> Result<(Self, Uuid), ()> {
        let mut candidate = self.clone();
        if candidate.sheets.len() <= 2 {
            return Err(());
        }
        let deleted_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        candidate.sheets.remove(deleted_index);
        let neighbor_index = deleted_index.min(candidate.sheets.len() - 1);
        let neighbor_id = candidate.sheets[neighbor_index].id;
        validate_project_state(&candidate)?;
        Ok((candidate, neighbor_id))
    }

    pub(crate) fn with_converted_edge_sheet(&self, sheet_id: Uuid) -> Result<Self, ()> {
        let mut candidate = self.clone();
        let sheet_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        let last_index = candidate.sheets.len() - 1;
        let sheet = &mut candidate.sheets[sheet_index];
        sheet.active_sides = match (sheet_index, sheet.active_sides) {
            (0, ActiveSides::Both) => ActiveSides::Right,
            (0, ActiveSides::Right) => ActiveSides::Both,
            (index, ActiveSides::Both) if index == last_index => ActiveSides::Left,
            (index, ActiveSides::Left) if index == last_index => ActiveSides::Both,
            _ => return Err(()),
        };
        candidate.reorganize_sheet(sheet_id).map_err(|_| ())?;
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn with_reordered_sheet(
        &self,
        sheet_id: Uuid,
        target_index: usize,
    ) -> Result<Self, ()> {
        if target_index >= self.sheets.len() {
            return Err(());
        }
        let mut candidate = self.clone();
        let source_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        if source_index == target_index {
            return Err(());
        }
        let sheet = candidate.sheets.remove(source_index);
        candidate.sheets.insert(target_index, sheet);
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn validate_album_information(
        &self,
        information: &AlbumInformation,
    ) -> AlbumInformationValidation {
        let mut errors = information.configuration(self.sheets.len()).map_or_else(
            || vec![ProjectConfigurationValidationError::SheetCountTooSmall],
            |configuration| configuration.validation_errors(),
        );
        let dimensions_are_valid = !errors.iter().any(|error| {
            matches!(
                error,
                ProjectConfigurationValidationError::SheetWidthNotPositive
                    | ProjectConfigurationValidationError::SheetWidthAboveSafeInteger
                    | ProjectConfigurationValidationError::SheetWidthNotEven
                    | ProjectConfigurationValidationError::SheetWidthRasterOutOfRange
                    | ProjectConfigurationValidationError::SheetHeightNotPositive
                    | ProjectConfigurationValidationError::SheetHeightAboveSafeInteger
                    | ProjectConfigurationValidationError::SheetHeightRasterOutOfRange
            )
        });
        let dimensions_changed = information.sheet_width_um
            != signed_persisted_value(self.document.sheet_width_um())
            || information.sheet_height_um
                != signed_persisted_value(self.document.sheet_height_um());
        if dimensions_are_valid
            && dimensions_changed
            && !dimensions_keep_proportion(
                self.document.sheet_width_um(),
                self.document.sheet_height_um(),
                information.sheet_width_um,
                information.sheet_height_um,
            )
        {
            errors.push(ProjectConfigurationValidationError::SheetDimensionsNotProportional);
        }
        if dimensions_are_valid
            && dimensions_changed
            && self.sheets.iter().any(|sheet| !sheet.frames.is_empty())
        {
            errors.push(
                ProjectConfigurationValidationError::SheetDimensionsRequireContentTransformation,
            );
        }
        let impact = errors
            .is_empty()
            .then(|| album_information_impact(information))
            .flatten();
        AlbumInformationValidation { errors, impact }
    }

    pub(crate) fn with_album_information(
        &self,
        information: AlbumInformation,
    ) -> Result<Self, Vec<ProjectConfigurationValidationError>> {
        let validation = self.validate_album_information(&information);
        if !validation.errors.is_empty() {
            return Err(validation.errors);
        }

        let mut candidate = self.clone();
        candidate.document = DocumentSettings::new(
            information.display_unit,
            u64::try_from(information.sheet_width_um)
                .map_err(|_| vec![ProjectConfigurationValidationError::SheetWidthNotPositive])?,
            u64::try_from(information.sheet_height_um)
                .map_err(|_| vec![ProjectConfigurationValidationError::SheetHeightNotPositive])?,
            u32::try_from(information.dpi)
                .map_err(|_| vec![ProjectConfigurationValidationError::DpiOutOfRange])?,
            u64::try_from(information.bleed_um)
                .map_err(|_| vec![ProjectConfigurationValidationError::BleedNegative])?,
            u64::try_from(information.safety_um)
                .map_err(|_| vec![ProjectConfigurationValidationError::SafetyNegative])?,
        );
        let last_index = candidate.sheets.len() - 1;
        for (index, sides, error) in [
            (0, information.first_sheet.active_sides(true), ProjectConfigurationValidationError::FirstSheetConversionRequiresContentReorganization),
            (last_index, information.last_sheet.active_sides(false), ProjectConfigurationValidationError::LastSheetConversionRequiresContentReorganization),
        ] {
            if candidate.sheets[index].active_sides != sides {
                candidate.sheets[index].active_sides = sides;
                candidate.reorganize_sheet(candidate.sheets[index].id).map_err(|_| vec![error])?;
            }
        }
        validate_project_state(&candidate).map_err(|()| validation.errors)?;
        Ok(candidate)
    }

    pub(crate) fn with_imported_photos(&self, links: Vec<(Uuid, PathBuf)>) -> Result<Self, ()> {
        let mut candidate = self.clone();
        candidate.media.extend(
            links
                .into_iter()
                .map(|(media_id, path)| MediaRef::new(media_id, MediaKind::Photo, path)),
        );
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn with_added_frame(&self, sheet_id: Uuid) -> Result<(Self, Uuid), ()> {
        let mut candidate = self.clone();
        let sheet = candidate
            .sheets
            .iter_mut()
            .find(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        let rect = proportional_frame_rect(
            sheet,
            candidate.document.sheet_width_um,
            candidate.document.sheet_height_um,
            None,
        )?;
        let frame_id = Uuid::new_v4();
        sheet.frames.push(ProjectFrame::new(frame_id, rect, None));
        validate_project_state(&candidate)?;
        Ok((candidate, frame_id))
    }

    pub(crate) fn with_added_photo(
        &self,
        sheet_id: Uuid,
        media_id: Uuid,
        mode: PhotoPlacementMode,
    ) -> Result<(Self, Uuid), ()> {
        self.ensure_photo(media_id)?;
        let mut candidate = self.clone();
        let sheet_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        let affected =
            if let Some(frame_index) = leftmost_placeholder(&candidate.sheets[sheet_index]) {
                fill_frame(
                    &mut candidate.sheets[sheet_index].frames[frame_index],
                    media_id,
                )
            } else {
                add_frame(
                    &mut candidate.sheets[sheet_index],
                    candidate.document.sheet_width_um,
                    candidate.document.sheet_height_um,
                    media_id,
                    mode,
                    None,
                )?
            };
        if mode == PhotoPlacementMode::Normal
            && candidate.sheets[sheet_index].frames.len() != self.sheets[sheet_index].frames.len()
        {
            candidate.reorganize_sheet(sheet_id).map_err(|_| ())?;
        }
        validate_project_state(&candidate)?;
        Ok((candidate, affected))
    }

    pub(crate) fn with_dropped_photo(
        &self,
        sheet_id: Uuid,
        media_id: Uuid,
        x_um: i64,
        y_um: i64,
        mode: PhotoPlacementMode,
    ) -> Result<(Self, Uuid), ()> {
        self.ensure_photo(media_id)?;
        let mut candidate = self.clone();
        let sheet_index = candidate
            .sheets
            .iter()
            .position(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        let target = photo_drop_target(
            &candidate.sheets[sheet_index],
            candidate.document.sheet_width_um,
            candidate.document.sheet_height_um,
            x_um,
            y_um,
        );
        let affected = match target {
            PhotoDropTarget::Frame { frame_id } => {
                let frame_id = Uuid::parse_str(&frame_id).map_err(|_| ())?;
                let frame = candidate.sheets[sheet_index]
                    .frames
                    .iter_mut()
                    .find(|frame| frame.id == frame_id)
                    .ok_or(())?;
                fill_frame(frame, media_id)
            }
            PhotoDropTarget::Sheet { .. } => add_frame(
                &mut candidate.sheets[sheet_index],
                candidate.document.sheet_width_um,
                candidate.document.sheet_height_um,
                media_id,
                mode,
                Some((x_um, y_um)),
            )?,
            PhotoDropTarget::Invalid => return Err(()),
        };
        if mode == PhotoPlacementMode::Normal
            && candidate.sheets[sheet_index].frames.len() != self.sheets[sheet_index].frames.len()
        {
            candidate.reorganize_sheet(sheet_id).map_err(|_| ())?;
        }
        validate_project_state(&candidate)?;
        Ok((candidate, affected))
    }

    pub(crate) fn with_transformed_photo(
        &self,
        frame_id: Uuid,
        delta_pan_x: f32,
        delta_pan_y: f32,
        delta_zoom: f32,
    ) -> Result<Self, ()> {
        let mut candidate = self.clone();
        let frame = candidate
            .sheets
            .iter_mut()
            .flat_map(|sheet| &mut sheet.frames)
            .find(|frame| frame.id == frame_id)
            .ok_or(())?;
        let photo = frame.photo.as_mut().ok_or(())?;
        photo.transform = photo
            .transform
            .advanced(delta_pan_x, delta_pan_y, delta_zoom)?;
        validate_project_state(&candidate)?;
        Ok(candidate)
    }

    pub(crate) fn photo_drop_target(
        &self,
        sheet_id: Uuid,
        x_um: i64,
        y_um: i64,
    ) -> Result<PhotoDropTarget, ()> {
        let sheet = self
            .sheets
            .iter()
            .find(|sheet| sheet.id == sheet_id)
            .ok_or(())?;
        Ok(photo_drop_target(
            sheet,
            self.document.sheet_width_um,
            self.document.sheet_height_um,
            x_um,
            y_um,
        ))
    }

    fn frame_selection(
        &self,
        frame_ids: &[String],
    ) -> Result<(usize, std::collections::HashSet<Uuid>), ()> {
        let ids = frame_ids
            .iter()
            .map(|id| Uuid::parse_str(id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ())?;
        let first = ids.first().ok_or(())?;
        let sheet_index = self
            .sheets
            .iter()
            .position(|sheet| sheet.frames.iter().any(|frame| &frame.id == first))
            .ok_or(())?;
        let frames = &self.sheets[sheet_index].frames;
        let selected = ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if selected.len() != ids.len()
            || !ids
                .iter()
                .all(|id| frames.iter().any(|frame| &frame.id == id))
        {
            return Err(());
        }
        Ok((sheet_index, selected))
    }

    pub(crate) fn with_frame_style(
        &self,
        edit: &crate::FrameStyleEdit,
    ) -> Result<Self, crate::CoreError> {
        let (sheet_index, selected) = self
            .frame_selection(&edit.frame_ids)
            .map_err(|()| crate::CoreError::InvalidFrameStyleSelection)?;
        match &edit.change {
            crate::FrameStyleChange::Opacity { opacity_percent } if *opacity_percent > 100 => {
                return Err(crate::CoreError::InvalidFrameOpacity);
            }
            crate::FrameStyleChange::BorderWidth { width_um } if *width_um > MAX_SAFE_INTEGER => {
                return Err(crate::CoreError::InvalidFrameBorder);
            }
            crate::FrameStyleChange::BorderColor { rgb } if Rgb::parse_canonical(rgb).is_none() => {
                return Err(crate::CoreError::InvalidFrameBorder);
            }
            _ => {}
        }
        let mut candidate = self.clone();
        for frame in candidate.sheets[sheet_index]
            .frames
            .iter_mut()
            .filter(|frame| selected.contains(&frame.id))
        {
            let (mut border, mut opacity_percent) = frame.resolved_style(&self.visual_defaults);
            match &edit.change {
                crate::FrameStyleChange::RestoreAlbum => {
                    frame.style = FrameStyle::Album;
                    continue;
                }
                crate::FrameStyleChange::Opacity {
                    opacity_percent: value,
                } => opacity_percent = *value,
                crate::FrameStyleChange::BorderWidth { width_um } => border.width_um = *width_um,
                crate::FrameStyleChange::BorderColor { rgb } => {
                    border.rgb = Rgb::parse_canonical(rgb).expect("validated Frame border color")
                }
            }
            frame.style = FrameStyle::Custom {
                border,
                opacity_percent,
            };
        }
        Ok(candidate)
    }

    pub(crate) fn with_oriented_photos(
        &self,
        frame_ids: &[String],
        action: crate::PhotoOrientationAction,
    ) -> Result<Self, crate::CoreError> {
        let (sheet_index, selected) = self
            .frame_selection(frame_ids)
            .map_err(|()| crate::CoreError::InvalidPhotoOrientationSelection)?;
        let transforms: Vec<_> = self.sheets[sheet_index]
            .frames
            .iter()
            .filter(|frame| selected.contains(&frame.id))
            .filter_map(|frame| frame.photo.as_ref().map(|photo| photo.transform))
            .collect();
        let Some(first) = transforms.first() else {
            return Ok(self.clone());
        };
        // Mixed controls make one absolute choice for every compatible Photo.
        let next_turn = if transforms
            .iter()
            .all(|t| t.quarter_turns == first.quarter_turns)
        {
            (first.quarter_turns + 3) % 4
        } else {
            3
        };
        let next_mirror = !transforms.iter().all(|t| t.mirror_x);
        let mut candidate = self.clone();
        for photo in candidate.sheets[sheet_index]
            .frames
            .iter_mut()
            .filter(|frame| selected.contains(&frame.id))
            .filter_map(|frame| frame.photo.as_mut())
        {
            match action {
                crate::PhotoOrientationAction::RotateCounterClockwise => {
                    photo.transform.quarter_turns = next_turn
                }
                crate::PhotoOrientationAction::ResetRotation => photo.transform.quarter_turns = 0,
                crate::PhotoOrientationAction::ToggleHorizontalMirror => {
                    photo.transform.mirror_x = next_mirror
                }
            }
        }
        Ok(candidate)
    }

    pub(crate) fn with_toggled_photo_black_and_white(
        &self,
        frame_ids: &[String],
    ) -> Result<Self, crate::CoreError> {
        let (sheet_index, selected) = self
            .frame_selection(frame_ids)
            .map_err(|()| crate::CoreError::InvalidPhotoEffectSelection)?;
        let enabled = !self.sheets[sheet_index]
            .frames
            .iter()
            .filter(|frame| selected.contains(&frame.id))
            .filter_map(|frame| frame.photo.as_ref())
            .all(|photo| photo.transform.black_and_white);
        let mut candidate = self.clone();
        for photo in candidate.sheets[sheet_index]
            .frames
            .iter_mut()
            .filter(|frame| selected.contains(&frame.id))
            .filter_map(|frame| frame.photo.as_mut())
        {
            photo.transform.black_and_white = enabled;
        }
        Ok(candidate)
    }

    pub(crate) fn with_photo_angle(
        &self,
        edit: &crate::PhotoAngleEdit,
    ) -> Result<Self, crate::CoreError> {
        let angle = PhotoFineAngle::new(edit.angle_tenths)
            .map_err(|()| crate::CoreError::InvalidPhotoAngle)?;
        let (sheet_index, selected) = self
            .frame_selection(&edit.frame_ids)
            .map_err(|()| crate::CoreError::InvalidPhotoOrientationSelection)?;
        let mut candidate = self.clone();
        for photo in candidate.sheets[sheet_index]
            .frames
            .iter_mut()
            .filter(|frame| selected.contains(&frame.id))
            .filter_map(|frame| frame.photo.as_mut())
        {
            photo.transform.angle = angle;
        }
        Ok(candidate)
    }

    pub(crate) fn with_swapped_sheet_sides(
        &self,
        sheet_id: &str,
    ) -> Result<Self, crate::CoreError> {
        let id = Uuid::parse_str(sheet_id)
            .map_err(|_| crate::CoreError::SheetNotFound(sheet_id.into()))?;
        let sheet_index = self
            .sheets
            .iter()
            .position(|sheet| sheet.id == id)
            .ok_or_else(|| crate::CoreError::SheetNotFound(sheet_id.into()))?;
        if self.sheets[sheet_index].active_sides != ActiveSides::Both {
            return Err(crate::CoreError::InvalidSheetSideSwap);
        }
        let page_width = self.document.sheet_width_um / 2;
        let mut candidate = self.clone();
        for frame in &mut candidate.sheets[sheet_index].frames {
            if frame.rect.x + frame.rect.width <= page_width {
                frame.rect.x += page_width;
            } else if frame.rect.x >= page_width {
                frame.rect.x -= page_width;
            }
        }
        Ok(candidate)
    }

    pub(crate) fn with_swapped_frame_contents(
        &self,
        frame_ids: &[String],
    ) -> Result<Self, crate::CoreError> {
        use crate::CoreError::InvalidFrameContentSwapSelection;
        let selected = frame_ids
            .iter()
            .map(|id| Uuid::parse_str(id).map_err(|_| InvalidFrameContentSwapSelection))
            .collect::<Result<std::collections::HashSet<_>, _>>()?;
        if frame_ids.len() != 2 || selected.len() != 2 {
            return Err(InvalidFrameContentSwapSelection);
        }
        let mut candidate = self.clone();
        let mut frames = candidate
            .sheets
            .iter_mut()
            .flat_map(|sheet| sheet.frames.iter_mut())
            .filter(|frame| selected.contains(&frame.id));
        let first = frames.next().ok_or(InvalidFrameContentSwapSelection)?;
        let second = frames.next().ok_or(InvalidFrameContentSwapSelection)?;
        if first.photo.is_none() && second.photo.is_none() {
            return Err(InvalidFrameContentSwapSelection);
        }
        std::mem::swap(&mut first.photo, &mut second.photo);
        Ok(candidate)
    }

    pub(crate) fn with_deleted_frames(
        &self,
        frame_ids: &[String],
        mode: PhotoPlacementMode,
    ) -> Result<Self, crate::CoreError> {
        let (sheet_index, selected) = self
            .frame_selection(frame_ids)
            .map_err(|()| crate::CoreError::InvalidFrameDeletionSelection)?;
        let mut candidate = self.clone();
        candidate.sheets[sheet_index]
            .frames
            .retain(|frame| !selected.contains(&frame.id));
        if mode == PhotoPlacementMode::Normal {
            candidate.reorganize_sheet(candidate.sheets[sheet_index].id)?;
        }
        Ok(candidate)
    }

    pub(crate) fn with_arranged_frames(
        &self,
        frame_ids: &[String],
        action: crate::FrameStackAction,
    ) -> Result<Self, crate::CoreError> {
        use crate::FrameStackAction::*;
        let (sheet_index, selected) = self
            .frame_selection(frame_ids)
            .map_err(|()| crate::CoreError::InvalidFrameStackSelection)?;
        let frames = &self.sheets[sheet_index].frames;
        let mut candidate = self.clone();
        let arranged = &mut candidate.sheets[sheet_index].frames;
        match action {
            Advance => {
                for index in (0..arranged.len().saturating_sub(1)).rev() {
                    if selected.contains(&arranged[index].id)
                        && !selected.contains(&arranged[index + 1].id)
                    {
                        arranged.swap(index, index + 1);
                    }
                }
            }
            Recede => {
                for index in 1..arranged.len() {
                    if selected.contains(&arranged[index].id)
                        && !selected.contains(&arranged[index - 1].id)
                    {
                        arranged.swap(index, index - 1);
                    }
                }
            }
            BringToFront | SendToBack => {
                let selected_first = action == SendToBack;
                *arranged = frames
                    .iter()
                    .filter(|frame| selected.contains(&frame.id) == selected_first)
                    .chain(
                        frames
                            .iter()
                            .filter(|frame| selected.contains(&frame.id) != selected_first),
                    )
                    .cloned()
                    .collect();
            }
        }
        Ok(candidate)
    }

    pub(crate) fn frame_geometry_edit(
        &self,
        edit: &crate::FrameGeometryEdit,
    ) -> Result<Vec<(Uuid, ProjectRect)>, crate::CoreError> {
        let first = edit
            .frames
            .first()
            .ok_or(crate::CoreError::InvalidFrameGeometrySelection)?;
        let first_id = Uuid::parse_str(&first.frame_id)
            .map_err(|_| crate::CoreError::FrameNotFound(first.frame_id.clone()))?;
        let sheet = self
            .sheets
            .iter()
            .find(|sheet| sheet.frames.iter().any(|frame| frame.id == first_id))
            .ok_or_else(|| crate::CoreError::FrameNotFound(first.frame_id.clone()))?;
        let mut ids = Vec::with_capacity(edit.frames.len());
        let mut rects = Vec::with_capacity(edit.frames.len());
        for target in &edit.frames {
            let id = Uuid::parse_str(&target.frame_id)
                .map_err(|_| crate::CoreError::FrameNotFound(target.frame_id.clone()))?;
            if ids.contains(&id) {
                return Err(crate::CoreError::InvalidFrameGeometrySelection);
            }
            let frame = sheet
                .frames
                .iter()
                .find(|frame| frame.id == id)
                .ok_or(crate::CoreError::InvalidFrameGeometrySelection)?;
            if crate::RectUm::from(frame.rect) != target.expected_rect {
                return Err(crate::CoreError::FrameGeometryChanged);
            }
            ids.push(id);
            rects.push(frame.rect);
        }
        let rects = crate::frame_geometry::edited_rects(
            &rects,
            active_surface_width(sheet, self.document.sheet_width_um),
            self.document.sheet_height_um,
            &edit.gesture,
        );
        Ok(ids.into_iter().zip(rects).collect())
    }

    pub(crate) fn with_edited_frame_geometry(
        &self,
        edit: &crate::FrameGeometryEdit,
    ) -> Result<Self, crate::CoreError> {
        let edits = self.frame_geometry_edit(edit)?;
        let mut candidate = self.clone();
        for frame in candidate
            .sheets
            .iter_mut()
            .flat_map(|sheet| &mut sheet.frames)
        {
            if let Some((_, rect)) = edits.iter().find(|(id, _)| *id == frame.id) {
                frame.rect = *rect;
            }
        }
        Ok(candidate)
    }

    fn ensure_photo(&self, media_id: Uuid) -> Result<(), ()> {
        self.media
            .iter()
            .any(|media| media.id == media_id && media.kind == MediaKind::Photo)
            .then_some(())
            .ok_or(())
    }
}

fn active_surface_width(sheet: &ProjectSheet, sheet_width_um: u64) -> u64 {
    match sheet.active_sides {
        ActiveSides::Both => sheet_width_um,
        ActiveSides::Left | ActiveSides::Right => sheet_width_um / 2,
    }
}

fn leftmost_placeholder(sheet: &ProjectSheet) -> Option<usize> {
    sheet
        .frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame.photo.is_none())
        .min_by_key(|(_, frame)| (frame.rect.x, frame.rect.y))
        .map(|(index, _)| index)
}

fn fill_frame(frame: &mut ProjectFrame, media_id: Uuid) -> Uuid {
    frame.photo = Some(ProjectPhoto::new(
        media_id,
        ProjectPhotoTransform::default(),
    ));
    frame.id
}

fn add_frame(
    sheet: &mut ProjectSheet,
    sheet_width_um: u64,
    sheet_height_um: u64,
    media_id: Uuid,
    mode: PhotoPlacementMode,
    point: Option<(i64, i64)>,
) -> Result<Uuid, ()> {
    let id = Uuid::new_v4();
    let initial_point = if mode == PhotoPlacementMode::Edit {
        point
    } else {
        None
    };
    let rect = proportional_frame_rect(sheet, sheet_width_um, sheet_height_um, initial_point)?;
    sheet.frames.push(ProjectFrame::new(
        id,
        rect,
        Some(ProjectPhoto::new(
            media_id,
            ProjectPhotoTransform::default(),
        )),
    ));
    Ok(id)
}

fn proportional_frame_rect(
    sheet: &ProjectSheet,
    sheet_width_um: u64,
    sheet_height_um: u64,
    point: Option<(i64, i64)>,
) -> Result<ProjectRect, ()> {
    let width = active_surface_width(sheet, sheet_width_um);
    let frame_width = (width.saturating_mul(2) / 5)
        .min(sheet_height_um.saturating_mul(3) / 2)
        .max(1);
    let frame_height = (frame_width.saturating_mul(2) / 3).max(1);
    let (center_x, center_y) = point.unwrap_or((
        i64::try_from(width / 2).map_err(|_| ())?,
        i64::try_from(sheet_height_um / 2).map_err(|_| ())?,
    ));
    centered_inside_rect(
        width,
        sheet_height_um,
        frame_width,
        frame_height,
        center_x,
        center_y,
    )
}

fn centered_inside_rect(
    surface_width: u64,
    surface_height: u64,
    frame_width: u64,
    frame_height: u64,
    center_x: i64,
    center_y: i64,
) -> Result<ProjectRect, ()> {
    if frame_width > surface_width || frame_height > surface_height {
        return Err(());
    }
    let half_width = i64::try_from(frame_width / 2).map_err(|_| ())?;
    let half_height = i64::try_from(frame_height / 2).map_err(|_| ())?;
    let max_x = i64::try_from(surface_width - frame_width).map_err(|_| ())?;
    let max_y = i64::try_from(surface_height - frame_height).map_err(|_| ())?;
    let x = (center_x - half_width).clamp(0, max_x);
    let y = (center_y - half_height).clamp(0, max_y);
    Ok(ProjectRect::new(
        u64::try_from(x).map_err(|_| ())?,
        u64::try_from(y).map_err(|_| ())?,
        frame_width,
        frame_height,
    ))
}

fn photo_drop_target(
    sheet: &ProjectSheet,
    sheet_width_um: u64,
    sheet_height_um: u64,
    x_um: i64,
    y_um: i64,
) -> PhotoDropTarget {
    let Ok(x) = u64::try_from(x_um) else {
        return PhotoDropTarget::Invalid;
    };
    let Ok(y) = u64::try_from(y_um) else {
        return PhotoDropTarget::Invalid;
    };
    let surface_width = active_surface_width(sheet, sheet_width_um);
    if x >= surface_width || y >= sheet_height_um {
        return PhotoDropTarget::Invalid;
    }
    if let Some(frame) = sheet
        .frames
        .iter()
        .rev()
        .find(|frame| frame.rect.contains(x, y))
    {
        return PhotoDropTarget::Frame {
            frame_id: frame.id.hyphenated().to_string(),
        };
    }
    PhotoDropTarget::Sheet {
        sheet_id: sheet.id.hyphenated().to_string(),
    }
}

fn visual_defaults_from_projection(
    project: &ProjectDocument,
    defaults: ProjectedVisualDefaults,
) -> Result<VisualDefaults, ()> {
    let background = match defaults.background {
        ProjectedBackground::BothSides { both } => Background::BothSides {
            both: background_content_from_projection(project, both)?,
        },
        ProjectedBackground::PerSide { left, right } => Background::PerSide {
            left: background_content_from_projection(project, left)?,
            right: background_content_from_projection(project, right)?,
        },
    };
    let overlay = match defaults.overlay {
        ProjectedOverlay::BothSides { both } => Overlay::BothSides {
            both: overlay_content_from_projection(project, both)?,
        },
        ProjectedOverlay::PerSide { left, right } => Overlay::PerSide {
            left: overlay_content_from_projection(project, left)?,
            right: overlay_content_from_projection(project, right)?,
        },
    };
    let frame_border = match defaults.frame_border {
        ProjectedFrameBorder::None => FrameBorder::None,
        ProjectedFrameBorder::Solid { rgb, width_um } => {
            if !frame_border_width_is_valid(width_um) {
                return Err(());
            }
            FrameBorder::Solid {
                rgb: Rgb::parse_canonical(&rgb).ok_or(())?,
                width_um,
            }
        }
    };
    Ok(VisualDefaults::new(background, overlay, frame_border))
}

fn background_content_from_projection(
    project: &ProjectDocument,
    content: ProjectedBackgroundContent,
) -> Result<BackgroundContent, ()> {
    match content {
        ProjectedBackgroundContent::Color { rgb } => Ok(BackgroundContent::Color {
            rgb: Rgb::parse_canonical(&rgb).ok_or(())?,
        }),
        ProjectedBackgroundContent::Media { media_id } => Ok(BackgroundContent::Media {
            media_id: decorative_media_id(project, media_id)?,
        }),
    }
}

fn overlay_content_from_projection(
    project: &ProjectDocument,
    content: Option<ProjectedOverlayContent>,
) -> Result<Option<OverlayContent>, ()> {
    content
        .map(|ProjectedOverlayContent::Media { media_id }| {
            Ok(OverlayContent::Media {
                media_id: decorative_media_id(project, media_id)?,
            })
        })
        .transpose()
}

fn decorative_media_id(project: &ProjectDocument, media_id: MediaId) -> Result<Uuid, ()> {
    let media_id = media_id.into_uuid();
    project
        .media
        .iter()
        .any(|media| media.id() == media_id && media.kind() == MediaKind::Decorative)
        .then_some(media_id)
        .ok_or(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EndSheetFormat {
    Double,
    SinglePage,
}

impl EndSheetFormat {
    fn active_sides(self, first: bool) -> ActiveSides {
        match (self, first) {
            (Self::Double, _) => ActiveSides::Both,
            (Self::SinglePage, true) => ActiveSides::Right,
            (Self::SinglePage, false) => ActiveSides::Left,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialBackgroundContent {
    Color { rgb: Rgb },
    Media { path: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialBackground {
    BothSides {
        both: InitialBackgroundContent,
    },
    PerSide {
        left: InitialBackgroundContent,
        right: InitialBackgroundContent,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialOverlayContent {
    Media { path: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialOverlay {
    BothSides {
        both: Option<InitialOverlayContent>,
    },
    PerSide {
        left: Option<InitialOverlayContent>,
        right: Option<InitialOverlayContent>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InitialFrameBorder {
    None,
    Solid { rgb: Rgb, width_um: i64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialProjectPersonalization {
    background: InitialBackground,
    overlay: InitialOverlay,
    frame_border: InitialFrameBorder,
}

impl InitialProjectPersonalization {
    pub fn new(
        background: InitialBackground,
        overlay: InitialOverlay,
        frame_border: InitialFrameBorder,
    ) -> Self {
        Self {
            background,
            overlay,
            frame_border,
        }
    }

    pub fn neutral() -> Self {
        Self::new(
            InitialBackground::BothSides {
                both: InitialBackgroundContent::Color { rgb: Rgb::WHITE },
            },
            InitialOverlay::BothSides { both: None },
            InitialFrameBorder::None,
        )
    }

    fn into_domain(self) -> Result<(VisualDefaults, Vec<MediaRef>), ()> {
        let mut media = InitialMediaCatalog::default();
        let background = match self.background {
            InitialBackground::BothSides { both } => Background::BothSides {
                both: initial_background_content(both, &mut media)?,
            },
            InitialBackground::PerSide { left, right } => Background::PerSide {
                left: initial_background_content(left, &mut media)?,
                right: initial_background_content(right, &mut media)?,
            },
        };
        let overlay = match self.overlay {
            InitialOverlay::BothSides { both } => Overlay::BothSides {
                both: both
                    .map(|content| initial_overlay_content(content, &mut media))
                    .transpose()?,
            },
            InitialOverlay::PerSide { left, right } => Overlay::PerSide {
                left: left
                    .map(|content| initial_overlay_content(content, &mut media))
                    .transpose()?,
                right: right
                    .map(|content| initial_overlay_content(content, &mut media))
                    .transpose()?,
            },
        };
        let frame_border = match self.frame_border {
            InitialFrameBorder::None => FrameBorder::None,
            InitialFrameBorder::Solid { rgb, width_um } => {
                let width_um = u64::try_from(width_um).map_err(|_| ())?;
                if !frame_border_width_is_valid(width_um) {
                    return Err(());
                }
                FrameBorder::Solid { rgb, width_um }
            }
        };
        Ok((
            VisualDefaults::new(background, overlay, frame_border),
            media.into_items(),
        ))
    }
}

#[derive(Default)]
struct InitialMediaCatalog {
    items: Vec<MediaRef>,
}

impl InitialMediaCatalog {
    fn id_for_path(&mut self, path: PathBuf) -> Result<Uuid, ()> {
        validate_external_path(&path).map_err(|_| ())?;
        if let Some(existing) = self.items.iter().find(|media| media.path() == path) {
            return Ok(existing.id());
        }
        let id = Uuid::new_v4();
        self.items
            .push(MediaRef::new(id, MediaKind::Decorative, path));
        Ok(id)
    }

    fn into_items(self) -> Vec<MediaRef> {
        self.items
    }
}

fn initial_background_content(
    content: InitialBackgroundContent,
    media: &mut InitialMediaCatalog,
) -> Result<BackgroundContent, ()> {
    match content {
        InitialBackgroundContent::Color { rgb } => Ok(BackgroundContent::Color { rgb }),
        InitialBackgroundContent::Media { path } => Ok(BackgroundContent::Media {
            media_id: media.id_for_path(path)?,
        }),
    }
}

fn initial_overlay_content(
    content: InitialOverlayContent,
    media: &mut InitialMediaCatalog,
) -> Result<OverlayContent, ()> {
    match content {
        InitialOverlayContent::Media { path } => Ok(OverlayContent::Media {
            media_id: media.id_for_path(path)?,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectConfigurationValidationError {
    SheetWidthNotPositive,
    SheetWidthAboveSafeInteger,
    SheetWidthNotEven,
    SheetWidthRasterOutOfRange,
    SheetHeightNotPositive,
    SheetHeightAboveSafeInteger,
    SheetHeightRasterOutOfRange,
    SheetDimensionsNotProportional,
    SheetDimensionsRequireContentTransformation,
    FirstSheetConversionRequiresContentReorganization,
    LastSheetConversionRequiresContentReorganization,
    DpiOutOfRange,
    SheetCountTooSmall,
    BleedNegative,
    BleedAboveSafeInteger,
    BleedEliminatesCutArea,
    SafetyNegative,
    SafetyAboveSafeInteger,
    SafetyEliminatesSafeArea,
}

fn dimensions_keep_proportion(
    current_width_um: u64,
    current_height_um: u64,
    next_width_um: i64,
    next_height_um: i64,
) -> bool {
    i128::from(current_width_um) * i128::from(next_height_um)
        == i128::from(current_height_um) * i128::from(next_width_um)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInformation {
    pub display_unit: DisplayUnit,
    pub sheet_width_um: i64,
    pub sheet_height_um: i64,
    pub dpi: i64,
    pub bleed_um: i64,
    pub safety_um: i64,
    pub first_sheet: EndSheetFormat,
    pub last_sheet: EndSheetFormat,
}

impl AlbumInformation {
    fn configuration(self, sheet_count: usize) -> Option<InitialProjectConfiguration> {
        Some(InitialProjectConfiguration::new(
            self.display_unit,
            self.sheet_width_um,
            self.sheet_height_um,
            self.dpi,
            self.bleed_um,
            self.safety_um,
            i64::try_from(sheet_count).ok()?,
            self.first_sheet,
            self.last_sheet,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInformationValidation {
    pub errors: Vec<ProjectConfigurationValidationError>,
    pub impact: Option<AlbumInformationImpact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInformationImpact {
    pub sheet_width_px: u32,
    pub page_width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialProjectConfiguration {
    display_unit: DisplayUnit,
    sheet_width_um: i64,
    sheet_height_um: i64,
    dpi: i64,
    bleed_um: i64,
    safety_um: i64,
    sheet_count: i64,
    first_sheet: EndSheetFormat,
    last_sheet: EndSheetFormat,
}

impl InitialProjectConfiguration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display_unit: DisplayUnit,
        sheet_width_um: i64,
        sheet_height_um: i64,
        dpi: i64,
        bleed_um: i64,
        safety_um: i64,
        sheet_count: i64,
        first_sheet: EndSheetFormat,
        last_sheet: EndSheetFormat,
    ) -> Self {
        Self {
            display_unit,
            sheet_width_um,
            sheet_height_um,
            dpi,
            bleed_um,
            safety_um,
            sheet_count,
            first_sheet,
            last_sheet,
        }
    }

    pub fn validation_errors(&self) -> Vec<ProjectConfigurationValidationError> {
        validation_errors(InitialProjectValidationValues {
            sheet_width_um: i128::from(self.sheet_width_um),
            sheet_height_um: i128::from(self.sheet_height_um),
            dpi: i128::from(self.dpi),
            bleed_um: i128::from(self.bleed_um),
            safety_um: i128::from(self.safety_um),
            sheet_count: i128::from(self.sheet_count),
        })
    }

    fn into_project(
        self,
        personalization: InitialProjectPersonalization,
    ) -> Result<ProjectDocument, ()> {
        if !self.validation_errors().is_empty() {
            return Err(());
        }
        let document = DocumentSettings::new(
            self.display_unit,
            u64::try_from(self.sheet_width_um).map_err(|_| ())?,
            u64::try_from(self.sheet_height_um).map_err(|_| ())?,
            u32::try_from(self.dpi).map_err(|_| ())?,
            u64::try_from(self.bleed_um).map_err(|_| ())?,
            u64::try_from(self.safety_um).map_err(|_| ())?,
        );

        let sheet_count = usize::try_from(self.sheet_count).map_err(|_| ())?;
        let mut sheets = Vec::new();
        sheets.try_reserve_exact(sheet_count).map_err(|_| ())?;
        for index in 0..sheet_count {
            let active_sides = if index == 0 {
                match self.first_sheet {
                    EndSheetFormat::Double => ActiveSides::Both,
                    EndSheetFormat::SinglePage => ActiveSides::Right,
                }
            } else if index == sheet_count - 1 {
                match self.last_sheet {
                    EndSheetFormat::Double => ActiveSides::Both,
                    EndSheetFormat::SinglePage => ActiveSides::Left,
                }
            } else {
                ActiveSides::Both
            };
            sheets.push(ProjectSheet::new(Uuid::new_v4(), active_sides));
        }

        let (visual_defaults, media) = personalization.into_domain()?;
        let project = ProjectDocument::new(document, visual_defaults, media, sheets);
        validate_project_state(&project)?;
        Ok(project)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialProject {
    configuration: InitialProjectConfiguration,
    personalization: InitialProjectPersonalization,
}

impl InitialProject {
    pub fn neutral() -> Self {
        let document = DocumentSettings::neutral();
        Self::configured(InitialProjectConfiguration::new(
            document.display_unit(),
            i64::try_from(document.sheet_width_um()).expect("neutral width fits i64"),
            i64::try_from(document.sheet_height_um()).expect("neutral height fits i64"),
            i64::from(document.dpi()),
            i64::try_from(document.bleed_um()).expect("neutral bleed fits i64"),
            i64::try_from(document.safety_um()).expect("neutral safety fits i64"),
            2,
            EndSheetFormat::Double,
            EndSheetFormat::Double,
        ))
    }

    pub fn configured(configuration: InitialProjectConfiguration) -> Self {
        Self {
            configuration,
            personalization: InitialProjectPersonalization::neutral(),
        }
    }

    pub fn with_personalization(mut self, personalization: InitialProjectPersonalization) -> Self {
        self.personalization = personalization;
        self
    }

    pub(crate) fn into_project(self) -> Result<ProjectDocument, ()> {
        self.configuration.into_project(self.personalization)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectRevision {
    pub(crate) project_id: Uuid,
    pub(crate) revision: u64,
    pub(crate) project: ProjectDocument,
}

impl ProjectRevision {
    pub(crate) fn new(project_id: Uuid, revision: u64, project: ProjectDocument) -> Self {
        Self {
            project_id,
            revision,
            project,
        }
    }
}

pub(crate) fn validate_project_state(project: &ProjectDocument) -> Result<(), ()> {
    if !project.layout_state_is_valid() {
        return Err(());
    }
    let settings = project.document();
    let configuration = InitialProjectConfiguration::new(
        settings.display_unit(),
        signed_persisted_value(settings.sheet_width_um()),
        signed_persisted_value(settings.sheet_height_um()),
        i64::from(settings.dpi()),
        signed_persisted_value(settings.bleed_um()),
        signed_persisted_value(settings.safety_um()),
        i64::try_from(project.sheets().len()).map_err(|_| ())?,
        EndSheetFormat::Double,
        EndSheetFormat::Double,
    );
    if !configuration.validation_errors().is_empty() {
        return Err(());
    }

    let mut media_by_id = HashMap::new();
    let mut media_paths = HashSet::new();
    for media in project.media() {
        if validate_external_path(media.path()).is_err()
            || media_by_id.insert(media.id(), media.kind()).is_some()
            || !media_paths.insert((media.kind(), media.path().to_path_buf()))
        {
            return Err(());
        }
    }
    for media_id in referenced_media(project.visual_defaults()) {
        if media_by_id.get(&media_id) != Some(&MediaKind::Decorative) {
            return Err(());
        }
    }
    if let FrameBorder::Solid { width_um, .. } = project.visual_defaults().frame_border()
        && !frame_border_width_is_valid(*width_um)
    {
        return Err(());
    }

    let mut sheet_ids = HashSet::new();
    let mut frame_ids = HashSet::new();
    for (index, sheet) in project.sheets().iter().enumerate() {
        if !sheet_ids.insert(sheet.id()) {
            return Err(());
        }
        let last = project.sheets().len() - 1;
        let valid_sides = if index == 0 {
            matches!(sheet.active_sides(), ActiveSides::Both | ActiveSides::Right)
        } else if index == last {
            matches!(sheet.active_sides(), ActiveSides::Both | ActiveSides::Left)
        } else {
            sheet.active_sides() == ActiveSides::Both
        };
        if !valid_sides {
            return Err(());
        }
        let surface_width = active_surface_width(sheet, settings.sheet_width_um());
        for frame in sheet.frames() {
            if let FrameStyle::Custom {
                border,
                opacity_percent,
            } = frame.style()
                && (border.width_um > MAX_SAFE_INTEGER || *opacity_percent > 100)
            {
                return Err(());
            }
            let rect = frame.rect();
            if !frame_ids.insert(frame.id())
                || rect.width() == 0
                || rect.height() == 0
                || rect
                    .x()
                    .checked_add(rect.width())
                    .is_none_or(|far_x| far_x > surface_width)
                || rect
                    .y()
                    .checked_add(rect.height())
                    .is_none_or(|far_y| far_y > settings.sheet_height_um())
                || frame.photo().is_some_and(|photo| {
                    media_by_id.get(&photo.media_id()) != Some(&MediaKind::Photo)
                })
            {
                return Err(());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct InitialProjectValidationValues {
    sheet_width_um: i128,
    sheet_height_um: i128,
    dpi: i128,
    bleed_um: i128,
    safety_um: i128,
    sheet_count: i128,
}

fn validation_errors(
    values: InitialProjectValidationValues,
) -> Vec<ProjectConfigurationValidationError> {
    use ProjectConfigurationValidationError as Error;

    let mut errors = Vec::new();
    let safe_integer = i128::from(MAX_SAFE_INTEGER);
    let dpi_is_valid = (1..=1_200).contains(&values.dpi);
    let width_is_positive = values.sheet_width_um > 0;
    let width_is_safe = values.sheet_width_um <= safe_integer;
    let width_is_even = values.sheet_width_um % 2 == 0;
    let height_is_positive = values.sheet_height_um > 0;
    let height_is_safe = values.sheet_height_um <= safe_integer;

    if !width_is_positive {
        errors.push(Error::SheetWidthNotPositive);
    } else if !width_is_safe {
        errors.push(Error::SheetWidthAboveSafeInteger);
    } else if !width_is_even {
        errors.push(Error::SheetWidthNotEven);
    } else if dpi_is_valid
        && (!raster_axis_is_valid(values.sheet_width_um, values.dpi)
            || !raster_axis_is_valid(values.sheet_width_um / 2, values.dpi))
    {
        errors.push(Error::SheetWidthRasterOutOfRange);
    }

    if !height_is_positive {
        errors.push(Error::SheetHeightNotPositive);
    } else if !height_is_safe {
        errors.push(Error::SheetHeightAboveSafeInteger);
    } else if dpi_is_valid && !raster_axis_is_valid(values.sheet_height_um, values.dpi) {
        errors.push(Error::SheetHeightRasterOutOfRange);
    }

    if !dpi_is_valid {
        errors.push(Error::DpiOutOfRange);
    }
    if values.sheet_count < 2 {
        errors.push(Error::SheetCountTooSmall);
    }

    let bleed_is_nonnegative = values.bleed_um >= 0;
    let bleed_is_safe = values.bleed_um <= safe_integer;
    if !bleed_is_nonnegative {
        errors.push(Error::BleedNegative);
    } else if !bleed_is_safe {
        errors.push(Error::BleedAboveSafeInteger);
    }

    let safety_is_nonnegative = values.safety_um >= 0;
    let safety_is_safe = values.safety_um <= safe_integer;
    if !safety_is_nonnegative {
        errors.push(Error::SafetyNegative);
    } else if !safety_is_safe {
        errors.push(Error::SafetyAboveSafeInteger);
    }

    let dimensions_admit_margins =
        width_is_positive && width_is_safe && width_is_even && height_is_positive && height_is_safe;
    if dimensions_admit_margins && bleed_is_nonnegative && bleed_is_safe {
        let page_width = values.sheet_width_um / 2;
        let bleed_eliminates_cut_area = values.bleed_um >= page_width
            || values
                .bleed_um
                .checked_mul(2)
                .is_none_or(|vertical_inset| vertical_inset >= values.sheet_height_um);
        if bleed_eliminates_cut_area {
            errors.push(Error::BleedEliminatesCutArea);
        } else if safety_is_nonnegative && safety_is_safe {
            let safety_eliminates_safe_area = values
                .bleed_um
                .checked_add(values.safety_um)
                .is_none_or(|total_inset| {
                    total_inset >= page_width
                        || total_inset
                            .checked_mul(2)
                            .is_none_or(|vertical_inset| vertical_inset >= values.sheet_height_um)
                });
            if safety_eliminates_safe_area {
                errors.push(Error::SafetyEliminatesSafeArea);
            }
        }
    }

    errors
}

fn raster_axis_is_valid(micrometers: i128, dpi: i128) -> bool {
    raster_axis_pixels(micrometers, dpi).is_some_and(|pixels| (1..=65_535).contains(&pixels))
}

fn raster_axis_pixels(micrometers: i128, dpi: i128) -> Option<i128> {
    micrometers
        .checked_mul(dpi)
        .and_then(|numerator| numerator.checked_add(12_700))
        .map(|numerator| numerator / 25_400)
}

fn album_information_impact(information: &AlbumInformation) -> Option<AlbumInformationImpact> {
    let width = i128::from(information.sheet_width_um);
    let height = i128::from(information.sheet_height_um);
    let dpi = i128::from(information.dpi);
    Some(AlbumInformationImpact {
        sheet_width_px: u32::try_from(raster_axis_pixels(width, dpi)?).ok()?,
        page_width_px: u32::try_from(raster_axis_pixels(width / 2, dpi)?).ok()?,
        height_px: u32::try_from(raster_axis_pixels(height, dpi)?).ok()?,
    })
}

fn signed_persisted_value(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn frame_border_width_is_valid(width_um: u64) -> bool {
    (1..=MAX_SAFE_INTEGER).contains(&width_um)
}

fn referenced_media(defaults: &VisualDefaults) -> Vec<Uuid> {
    let mut media = Vec::new();
    match defaults.background() {
        Background::BothSides { both } => push_background_reference(both, &mut media),
        Background::PerSide { left, right } => {
            push_background_reference(left, &mut media);
            push_background_reference(right, &mut media);
        }
    }
    match defaults.overlay() {
        Overlay::BothSides { both } => push_overlay_reference(both.as_ref(), &mut media),
        Overlay::PerSide { left, right } => {
            push_overlay_reference(left.as_ref(), &mut media);
            push_overlay_reference(right.as_ref(), &mut media);
        }
    }
    media
}

fn push_background_reference(content: &BackgroundContent, media: &mut Vec<Uuid>) {
    if let BackgroundContent::Media { media_id } = content {
        media.push(*media_id);
    }
}

fn push_overlay_reference(content: Option<&OverlayContent>, media: &mut Vec<Uuid>) {
    if let Some(OverlayContent::Media { media_id }) = content {
        media.push(*media_id);
    }
}
