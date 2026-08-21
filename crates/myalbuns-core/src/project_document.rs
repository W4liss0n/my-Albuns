use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use myalbuns_paths::validate_external_path;
use uuid::Uuid;

use crate::model::{
    MediaKind, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN, PhotoDropTarget,
    PhotoPlacementMode,
};

pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        })
    }

    fn advanced(self, delta_pan_x: f32, delta_pan_y: f32, delta_zoom: f32) -> Result<Self, ()> {
        if !delta_pan_x.is_finite() || !delta_pan_y.is_finite() || !delta_zoom.is_finite() {
            return Err(());
        }
        Self::new(
            (self.pan_x() + delta_pan_x).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX),
            (self.pan_y() + delta_pan_y).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX),
            (self.user_zoom() + delta_zoom).clamp(PHOTO_ZOOM_MIN, PHOTO_ZOOM_MAX),
        )
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
        Self { id, rect, photo }
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDocument {
    document: DocumentSettings,
    visual_defaults: VisualDefaults,
    media: Vec<MediaRef>,
    sheets: Vec<ProjectSheet>,
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
        }
    }

    pub(crate) fn with_dpi(&self, dpi: u32) -> Result<Self, ()> {
        let mut candidate = self.clone();
        candidate.document.dpi = dpi;
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

    pub(crate) fn with_imported_photo(&self, media_id: Uuid, path: PathBuf) -> Result<Self, ()> {
        let mut candidate = self.clone();
        candidate
            .media
            .push(MediaRef::new(media_id, MediaKind::Photo, path));
        validate_project_state(&candidate)?;
        Ok(candidate)
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
            mode,
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
        mode: PhotoPlacementMode,
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
            mode,
        ))
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
    let rect = match mode {
        PhotoPlacementMode::Normal => {
            sheet.frames.push(ProjectFrame::new(
                id,
                ProjectRect::new(0, 0, 1, 1),
                Some(ProjectPhoto::new(
                    media_id,
                    ProjectPhotoTransform::default(),
                )),
            ));
            apply_first_compatible_layout(sheet, sheet_width_um, sheet_height_um)?;
            return Ok(id);
        }
        PhotoPlacementMode::Edit => {
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
            )?
        }
    };
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

/// Deterministic candidate zero of the generated Layout catalog.
fn apply_first_compatible_layout(
    sheet: &mut ProjectSheet,
    sheet_width_um: u64,
    sheet_height_um: u64,
) -> Result<(), ()> {
    let count = sheet.frames.len();
    if count == 0 {
        return Ok(());
    }
    let width = active_surface_width(sheet, sheet_width_um);
    let columns = (count as f64).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns);
    let gap = (width.min(sheet_height_um) / 30).max(1);
    let usable_width = width
        .checked_sub(gap.saturating_mul(columns as u64 + 1))
        .ok_or(())?;
    let usable_height = sheet_height_um
        .checked_sub(gap.saturating_mul(rows as u64 + 1))
        .ok_or(())?;
    let cell_width = usable_width / columns as u64;
    let cell_height = usable_height / rows as u64;
    if cell_width == 0 || cell_height == 0 {
        return Err(());
    }
    for (index, frame) in sheet.frames.iter_mut().enumerate() {
        let column = (index % columns) as u64;
        let row = (index / columns) as u64;
        frame.rect = ProjectRect::new(
            gap + column * (cell_width + gap),
            gap + row * (cell_height + gap),
            cell_width,
            cell_height,
        );
    }
    Ok(())
}

fn photo_drop_target(
    sheet: &ProjectSheet,
    sheet_width_um: u64,
    sheet_height_um: u64,
    x_um: i64,
    y_um: i64,
    _mode: PhotoPlacementMode,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndSheetFormat {
    Double,
    SinglePage,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialProjectValidationError {
    SheetWidthNotPositive,
    SheetWidthAboveSafeInteger,
    SheetWidthNotEven,
    SheetWidthRasterOutOfRange,
    SheetHeightNotPositive,
    SheetHeightAboveSafeInteger,
    SheetHeightRasterOutOfRange,
    DpiOutOfRange,
    SheetCountTooSmall,
    BleedNegative,
    BleedAboveSafeInteger,
    BleedEliminatesCutArea,
    SafetyNegative,
    SafetyAboveSafeInteger,
    SafetyEliminatesSafeArea,
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

    pub fn validation_errors(&self) -> Vec<InitialProjectValidationError> {
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

fn validation_errors(values: InitialProjectValidationValues) -> Vec<InitialProjectValidationError> {
    use InitialProjectValidationError as Error;

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
    micrometers
        .checked_mul(dpi)
        .and_then(|numerator| numerator.checked_add(12_700))
        .map(|numerator| numerator / 25_400)
        .is_some_and(|pixels| (1..=65_535).contains(&pixels))
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
