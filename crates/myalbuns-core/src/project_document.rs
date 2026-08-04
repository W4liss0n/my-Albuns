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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndSheetFormat {
    Double,
    SinglePage,
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

    fn into_project(self) -> Result<ProjectDocument, ()> {
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

        let project = ProjectDocument::new(document, VisualDefaults::neutral(), Vec::new(), sheets);
        validate_project_state(&project)?;
        Ok(project)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialProject {
    configuration: InitialProjectConfiguration,
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
        Self { configuration }
    }

    pub(crate) fn into_project(self) -> Result<ProjectDocument, ()> {
        self.configuration.into_project()
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
