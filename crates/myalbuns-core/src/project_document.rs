use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use uuid::Uuid;

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

    pub(crate) fn neutral() -> Self {
        Self::new(
            Background::BothSides {
                both: BackgroundContent::Color { rgb: Rgb::WHITE },
            },
            Overlay::BothSides { both: None },
            FrameBorder::None,
        )
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
pub struct DecorativeMedia {
    id: Uuid,
    path: PathBuf,
}

impl DecorativeMedia {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn new(id: Uuid, path: PathBuf) -> Self {
        Self { id, path }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSheet {
    id: Uuid,
    active_sides: ActiveSides,
}

impl ProjectSheet {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn active_sides(&self) -> ActiveSides {
        self.active_sides
    }

    pub(crate) fn new(id: Uuid, active_sides: ActiveSides) -> Self {
        Self { id, active_sides }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDocument {
    document: DocumentSettings,
    visual_defaults: VisualDefaults,
    media: Vec<DecorativeMedia>,
    sheets: Vec<ProjectSheet>,
}

impl ProjectDocument {
    pub fn document(&self) -> &DocumentSettings {
        &self.document
    }

    pub fn visual_defaults(&self) -> &VisualDefaults {
        &self.visual_defaults
    }

    pub fn media(&self) -> &[DecorativeMedia] {
        &self.media
    }

    pub fn sheets(&self) -> &[ProjectSheet] {
        &self.sheets
    }

    pub(crate) fn new(
        document: DocumentSettings,
        visual_defaults: VisualDefaults,
        media: Vec<DecorativeMedia>,
        sheets: Vec<ProjectSheet>,
    ) -> Self {
        Self {
            document,
            visual_defaults,
            media,
            sheets,
        }
    }

    pub(crate) fn neutral() -> Self {
        Self::new(
            DocumentSettings::neutral(),
            VisualDefaults::neutral(),
            Vec::new(),
            vec![
                ProjectSheet::new(Uuid::new_v4(), ActiveSides::Both),
                ProjectSheet::new(Uuid::new_v4(), ActiveSides::Both),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialProject {
    project: ProjectDocument,
}

impl InitialProject {
    pub fn neutral() -> Self {
        Self {
            project: ProjectDocument::neutral(),
        }
    }

    pub fn project(&self) -> &ProjectDocument {
        &self.project
    }

    pub(crate) fn into_project(self) -> ProjectDocument {
        self.project
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
    if !settings.sheet_width_um().is_multiple_of(2)
        || raster_pixels(settings.sheet_width_um(), settings.dpi()).is_none()
        || raster_pixels(settings.sheet_height_um(), settings.dpi()).is_none()
        || !margins_preserve_positive_areas(settings)
    {
        return Err(());
    }

    let mut media_ids = HashSet::new();
    let mut media_paths = HashSet::new();
    for media in project.media() {
        if !media_ids.insert(media.id()) || !media_paths.insert(media.path().to_path_buf()) {
            return Err(());
        }
    }
    for media_id in referenced_media(project.visual_defaults()) {
        if !media_ids.contains(&media_id) {
            return Err(());
        }
    }

    if project.sheets().len() < 2 {
        return Err(());
    }
    let mut sheet_ids = HashSet::new();
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
    }
    Ok(())
}

fn margins_preserve_positive_areas(settings: &DocumentSettings) -> bool {
    let page_width = settings.sheet_width_um() / 2;
    let bleed = settings.bleed_um();
    let Some(total_inset) = bleed.checked_add(settings.safety_um()) else {
        return false;
    };
    [bleed, total_inset].into_iter().all(|inset| {
        inset
            .checked_mul(2)
            .is_some_and(|diameter| diameter < page_width && diameter < settings.sheet_height_um())
    })
}

fn raster_pixels(micrometers: u64, dpi: u32) -> Option<u32> {
    let numerator = micrometers
        .checked_mul(u64::from(dpi))?
        .checked_add(12_700)?;
    let pixels = numerator / 25_400;
    u32::try_from(pixels)
        .ok()
        .filter(|pixels| (1..=65_535).contains(pixels))
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
