use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROJECT_SCHEMA_VERSION: u32 = 1;
pub const SHEET_WIDTH_UM: i64 = 600_000;
pub const SHEET_HEIGHT_UM: i64 = 300_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectUm {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTransform {
    pub pan_x: f32,
    pub pan_y: f32,
    pub user_zoom: f32,
    pub quarter_turns: i8,
    pub fine_rotation_degrees: f32,
    pub mirror_x: bool,
}

impl Default for MediaTransform {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            user_zoom: 1.0,
            quarter_turns: 0,
            fine_rotation_degrees: 0.0,
            mirror_x: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSnapshot {
    pub media_id: String,
    pub name: String,
    pub source_width_px: u32,
    pub source_height_px: u32,
    pub palette: [String; 3],
    pub transform: MediaTransform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSnapshot {
    pub id: String,
    pub rect: RectUm,
    pub z_index: u32,
    pub photo: Option<PhotoSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SheetRole {
    Initial,
    Internal,
    Final,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetSnapshot {
    pub id: String,
    pub number: usize,
    pub role: SheetRole,
    pub width_um: i64,
    pub height_um: i64,
    pub frames: Vec<FrameSnapshot>,
    pub has_overlay: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCatalogItem {
    pub id: String,
    pub name: String,
    pub palette: [String; 3],
    pub usage_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSnapshot {
    pub sheets: Vec<SheetSnapshot>,
    pub media: Vec<MediaCatalogItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    pub project_id: String,
    pub project_name: String,
    pub album: AlbumSnapshot,
    pub revision: u64,
    pub saved_revision: u64,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedPhoto {
    pub media_id: String,
    pub name: String,
    pub draw_rect: RectUm,
    pub rotation_degrees: f32,
    pub mirror_x: bool,
    pub palette: [String; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedFrame {
    pub frame_id: String,
    pub clip_rect: RectUm,
    pub z_index: u32,
    pub photo: Option<ComposedPhoto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedSheet {
    pub sheet_id: String,
    pub number: usize,
    pub width_um: i64,
    pub height_um: i64,
    pub has_overlay: bool,
    pub frames: Vec<ComposedFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionPlan {
    pub sheets: Vec<ComposedSheet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSnapshot {
    pub schema_version: u32,
    pub project_id: String,
    pub project_name: String,
    pub revision: u64,
    pub unit: String,
    pub composition: CompositionPlan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectIntent {
    PanPhoto {
        frame_id: String,
        delta_x: f32,
        delta_y: f32,
    },
    ZoomPhoto {
        frame_id: String,
        delta: f32,
    },
    FillLeftmostPlaceholder {
        sheet_id: String,
        media_id: String,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    #[error("Frame não encontrado: {0}")]
    FrameNotFound(String),
    #[error("O Frame não contém uma Foto: {0}")]
    FrameHasNoPhoto(String),
    #[error("Lâmina não encontrada: {0}")]
    SheetNotFound(String),
    #[error("Foto não encontrada no Projeto: {0}")]
    MediaNotFound(String),
    #[error("A Lâmina não possui Frame placeholder: {0}")]
    PlaceholderNotFound(String),
    #[error("Documento de Projeto inválido: {0}")]
    InvalidProject(String),
    #[error("Snapshot de renderização inválido: {0}")]
    InvalidSnapshot(String),
    #[error("Versão de documento não suportada: {0}")]
    UnsupportedSchema(u32),
}

impl RenderSnapshot {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchema(self.schema_version));
        }
        if self.project_id.trim().is_empty() {
            return Err(CoreError::InvalidSnapshot(
                "a Identidade do Projeto está vazia".into(),
            ));
        }
        if self.unit != "micrometers" {
            return Err(CoreError::InvalidSnapshot(format!(
                "unidade não suportada: {}",
                self.unit
            )));
        }
        if self.composition.sheets.is_empty() {
            return Err(CoreError::InvalidSnapshot(
                "a composição não contém Lâminas".into(),
            ));
        }

        let mut sheet_ids = HashSet::new();
        let mut frame_ids = HashSet::new();
        for sheet in &self.composition.sheets {
            if sheet.sheet_id.trim().is_empty() || !sheet_ids.insert(sheet.sheet_id.as_str()) {
                return Err(CoreError::InvalidSnapshot(format!(
                    "Identificador de Lâmina vazio ou duplicado: {}",
                    sheet.sheet_id
                )));
            }
            validate_positive_dimensions(
                sheet.width_um,
                sheet.height_um,
                "Lâmina",
                &sheet.sheet_id,
                CoreError::InvalidSnapshot,
            )?;

            let mut previous_stack_key: Option<(u32, &str)> = None;
            for frame in &sheet.frames {
                if frame.frame_id.trim().is_empty() || !frame_ids.insert(frame.frame_id.as_str()) {
                    return Err(CoreError::InvalidSnapshot(format!(
                        "Identificador de Frame vazio ou duplicado: {}",
                        frame.frame_id
                    )));
                }
                validate_rect_within(
                    &frame.clip_rect,
                    sheet.width_um,
                    sheet.height_um,
                    "Frame",
                    &frame.frame_id,
                    CoreError::InvalidSnapshot,
                )?;
                let stack_key = (frame.z_index, frame.frame_id.as_str());
                if previous_stack_key.is_some_and(|previous| previous > stack_key) {
                    return Err(CoreError::InvalidSnapshot(format!(
                        "Pilha visual de Frames inválida na Lâmina {}",
                        sheet.sheet_id
                    )));
                }
                previous_stack_key = Some(stack_key);

                if let Some(photo) = &frame.photo {
                    if photo.media_id.trim().is_empty() {
                        return Err(CoreError::InvalidSnapshot(
                            "Identificador de Foto vazio".into(),
                        ));
                    }
                    validate_positive_dimensions(
                        photo.draw_rect.width,
                        photo.draw_rect.height,
                        "Foto composta",
                        &photo.media_id,
                        CoreError::InvalidSnapshot,
                    )?;
                    if !photo.rotation_degrees.is_finite() {
                        return Err(CoreError::InvalidSnapshot(format!(
                            "rotação inválida para a Foto {}",
                            photo.media_id
                        )));
                    }
                    validate_palette(&photo.palette, &photo.media_id, CoreError::InvalidSnapshot)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedProject {
    schema_version: u32,
    project_id: String,
    project_name: String,
    revision: u64,
    album: AlbumSnapshot,
}

pub struct ProjectSession {
    state: EditorState,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

struct HistoryEntry {
    album: AlbumSnapshot,
    revision: u64,
}

impl ProjectSession {
    pub fn state(&self) -> EditorState {
        self.state.clone()
    }

    pub fn apply(&mut self, intent: ProjectIntent) -> Result<EditorState, CoreError> {
        let previous = HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        };

        match intent {
            ProjectIntent::PanPhoto {
                frame_id,
                delta_x,
                delta_y,
            } => {
                let frame = find_frame_mut(&mut self.state.album, &frame_id)
                    .ok_or_else(|| CoreError::FrameNotFound(frame_id.clone()))?;
                let photo = frame
                    .photo
                    .as_mut()
                    .ok_or_else(|| CoreError::FrameHasNoPhoto(frame_id.clone()))?;

                photo.transform.pan_x = (photo.transform.pan_x + delta_x).clamp(-1.0, 1.0);
                photo.transform.pan_y = (photo.transform.pan_y + delta_y).clamp(-1.0, 1.0);
            }
            ProjectIntent::ZoomPhoto { frame_id, delta } => {
                let frame = find_frame_mut(&mut self.state.album, &frame_id)
                    .ok_or_else(|| CoreError::FrameNotFound(frame_id.clone()))?;
                let photo = frame
                    .photo
                    .as_mut()
                    .ok_or_else(|| CoreError::FrameHasNoPhoto(frame_id.clone()))?;
                photo.transform.user_zoom = (photo.transform.user_zoom + delta).clamp(1.0, 4.0);
            }
            ProjectIntent::FillLeftmostPlaceholder { sheet_id, media_id } => {
                let media = self
                    .state
                    .album
                    .media
                    .iter()
                    .find(|item| item.id == media_id)
                    .cloned()
                    .ok_or_else(|| CoreError::MediaNotFound(media_id.clone()))?;
                let sheet = self
                    .state
                    .album
                    .sheets
                    .iter_mut()
                    .find(|sheet| sheet.id == sheet_id)
                    .ok_or_else(|| CoreError::SheetNotFound(sheet_id.clone()))?;
                let frame = sheet
                    .frames
                    .iter_mut()
                    .filter(|frame| frame.photo.is_none())
                    .min_by_key(|frame| (frame.rect.x, frame.rect.y))
                    .ok_or_else(|| CoreError::PlaceholderNotFound(sheet_id.clone()))?;
                frame.photo = Some(photo_from_catalog_item(&media));
            }
        }

        self.undo.push(previous);
        self.redo.clear();
        self.state.revision += 1;
        self.refresh_history_flags();
        Ok(self.state())
    }

    pub fn undo(&mut self) -> Option<EditorState> {
        let previous = self.undo.pop()?;
        self.redo.push(HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        });
        self.state.album = previous.album;
        self.state.revision = previous.revision;
        self.refresh_history_flags();
        Some(self.state())
    }

    pub fn redo(&mut self) -> Option<EditorState> {
        let next = self.redo.pop()?;
        self.undo.push(HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        });
        self.state.album = next.album;
        self.state.revision = next.revision;
        self.refresh_history_flags();
        Some(self.state())
    }

    pub fn composition_plan(&self) -> CompositionPlan {
        CompositionCore::compose(&self.state.album)
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        build_render_snapshot(
            &self.state.project_id,
            &self.state.project_name,
            self.state.revision,
            &self.state.album,
        )
    }

    pub fn persisted_revision(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&PersistedProject {
            schema_version: PROJECT_SCHEMA_VERSION,
            project_id: self.state.project_id.clone(),
            project_name: self.state.project_name.clone(),
            revision: self.state.revision,
            album: self.state.album.clone(),
        })
        .map_err(|error| CoreError::InvalidProject(error.to_string()))
    }

    fn refresh_history_flags(&mut self) {
        self.state.can_undo = !self.undo.is_empty();
        self.state.can_redo = !self.redo.is_empty();
        self.state.dirty = self.state.revision != self.state.saved_revision;
    }
}

pub struct CompositionCore;

impl CompositionCore {
    pub fn compose(album: &AlbumSnapshot) -> CompositionPlan {
        CompositionPlan {
            sheets: album
                .sheets
                .iter()
                .map(|sheet| {
                    let mut frames = sheet
                        .frames
                        .iter()
                        .map(|frame| ComposedFrame {
                            frame_id: frame.id.clone(),
                            clip_rect: frame.rect.clone(),
                            z_index: frame.z_index,
                            photo: frame
                                .photo
                                .as_ref()
                                .map(|photo| compose_photo(&frame.rect, photo)),
                        })
                        .collect::<Vec<_>>();
                    frames.sort_by(|left, right| {
                        left.z_index
                            .cmp(&right.z_index)
                            .then_with(|| left.frame_id.cmp(&right.frame_id))
                    });

                    ComposedSheet {
                        sheet_id: sheet.id.clone(),
                        number: sheet.number,
                        width_um: sheet.width_um,
                        height_um: sheet.height_um,
                        has_overlay: sheet.has_overlay,
                        frames,
                    }
                })
                .collect(),
        }
    }
}

pub struct ProjectCore;

impl ProjectCore {
    pub fn open_sample_project(sheet_count: usize) -> ProjectSession {
        let sheet_count = sheet_count.max(2);
        let sheets = (1..=sheet_count)
            .map(|number| sample_sheet(number, sheet_count))
            .collect();

        ProjectSession {
            state: EditorState {
                project_id: "project-spike-001".into(),
                project_name: "Álbum Horizonte".into(),
                album: AlbumSnapshot {
                    sheets,
                    media: sample_media_catalog(),
                },
                revision: 0,
                saved_revision: 0,
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn load_persisted_revision(source: &str) -> Result<LoadedProjectRevision, CoreError> {
        let project: PersistedProject = serde_json::from_str(source)
            .map_err(|error| CoreError::InvalidProject(error.to_string()))?;
        if project.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchema(project.schema_version));
        }
        if project.project_id.trim().is_empty() {
            return Err(CoreError::InvalidProject(
                "a Identidade do Projeto está vazia".into(),
            ));
        }
        validate_album(&project.album)?;

        Ok(LoadedProjectRevision { project })
    }
}

pub struct LoadedProjectRevision {
    project: PersistedProject,
}

impl LoadedProjectRevision {
    pub fn revision(&self) -> u64 {
        self.project.revision
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        build_render_snapshot(
            &self.project.project_id,
            &self.project.project_name,
            self.project.revision,
            &self.project.album,
        )
    }
}

fn build_render_snapshot(
    project_id: &str,
    project_name: &str,
    revision: u64,
    album: &AlbumSnapshot,
) -> RenderSnapshot {
    RenderSnapshot {
        schema_version: PROJECT_SCHEMA_VERSION,
        project_id: project_id.into(),
        project_name: project_name.into(),
        revision,
        unit: "micrometers".into(),
        composition: CompositionCore::compose(album),
    }
}

fn compose_photo(frame: &RectUm, photo: &PhotoSnapshot) -> ComposedPhoto {
    let frame_ratio = frame.width as f64 / frame.height as f64;
    let source_ratio = photo.source_width_px as f64 / photo.source_height_px as f64;

    let (fill_width, fill_height) = if source_ratio >= frame_ratio {
        (
            (frame.height as f64 * source_ratio).round() as i64,
            frame.height,
        )
    } else {
        (
            frame.width,
            (frame.width as f64 / source_ratio).round() as i64,
        )
    };

    let zoom = photo.transform.user_zoom.max(1.0) as f64;
    let draw_width = (fill_width as f64 * zoom).round() as i64;
    let draw_height = (fill_height as f64 * zoom).round() as i64;
    let overflow_x = draw_width - frame.width;
    let overflow_y = draw_height - frame.height;
    let pan_x = photo.transform.pan_x.clamp(-1.0, 1.0) as f64;
    let pan_y = photo.transform.pan_y.clamp(-1.0, 1.0) as f64;

    ComposedPhoto {
        media_id: photo.media_id.clone(),
        name: photo.name.clone(),
        draw_rect: RectUm {
            x: frame.x - overflow_x / 2 + (pan_x * overflow_x as f64 / 2.0).round() as i64,
            y: frame.y - overflow_y / 2 + (pan_y * overflow_y as f64 / 2.0).round() as i64,
            width: draw_width,
            height: draw_height,
        },
        rotation_degrees: photo.transform.quarter_turns as f32 * 90.0
            + photo.transform.fine_rotation_degrees,
        mirror_x: photo.transform.mirror_x,
        palette: photo.palette.clone(),
    }
}

fn find_frame_mut<'a>(
    album: &'a mut AlbumSnapshot,
    frame_id: &str,
) -> Option<&'a mut FrameSnapshot> {
    album
        .sheets
        .iter_mut()
        .flat_map(|sheet| sheet.frames.iter_mut())
        .find(|frame| frame.id == frame_id)
}

fn validate_album(album: &AlbumSnapshot) -> Result<(), CoreError> {
    if album.sheets.is_empty() {
        return Err(CoreError::InvalidProject(
            "o Álbum não contém Lâminas".into(),
        ));
    }

    let mut media_ids = HashSet::new();
    for media in &album.media {
        if media.id.trim().is_empty() || !media_ids.insert(media.id.as_str()) {
            return Err(CoreError::InvalidProject(format!(
                "Identificador de Foto vazio ou duplicado: {}",
                media.id
            )));
        }
        validate_palette(&media.palette, &media.id, CoreError::InvalidProject)?;
    }

    let mut sheet_ids = HashSet::new();
    let mut frame_ids = HashSet::new();
    for sheet in &album.sheets {
        if sheet.id.trim().is_empty() || !sheet_ids.insert(sheet.id.as_str()) {
            return Err(CoreError::InvalidProject(format!(
                "Identificador de Lâmina vazio ou duplicado: {}",
                sheet.id
            )));
        }
        validate_positive_dimensions(
            sheet.width_um,
            sheet.height_um,
            "Lâmina",
            &sheet.id,
            CoreError::InvalidProject,
        )?;

        for frame in &sheet.frames {
            if frame.id.trim().is_empty() || !frame_ids.insert(frame.id.as_str()) {
                return Err(CoreError::InvalidProject(format!(
                    "Identificador de Frame vazio ou duplicado: {}",
                    frame.id
                )));
            }
            validate_rect_within(
                &frame.rect,
                sheet.width_um,
                sheet.height_um,
                "Frame",
                &frame.id,
                CoreError::InvalidProject,
            )?;

            if let Some(photo) = &frame.photo {
                if !media_ids.contains(photo.media_id.as_str()) {
                    return Err(CoreError::InvalidProject(format!(
                        "Foto não pertence ao catálogo: {}",
                        photo.media_id
                    )));
                }
                if photo.source_width_px == 0 || photo.source_height_px == 0 {
                    return Err(CoreError::InvalidProject(format!(
                        "dimensões de origem inválidas para a Foto {}",
                        photo.media_id
                    )));
                }
                let transform = &photo.transform;
                if !transform.pan_x.is_finite()
                    || !transform.pan_y.is_finite()
                    || !transform.user_zoom.is_finite()
                    || !transform.fine_rotation_degrees.is_finite()
                    || !(-1.0..=1.0).contains(&transform.pan_x)
                    || !(-1.0..=1.0).contains(&transform.pan_y)
                    || !(1.0..=4.0).contains(&transform.user_zoom)
                {
                    return Err(CoreError::InvalidProject(format!(
                        "transformação inválida para a Foto {}",
                        photo.media_id
                    )));
                }
                validate_palette(&photo.palette, &photo.media_id, CoreError::InvalidProject)?;
            }
        }
    }

    Ok(())
}

fn validate_positive_dimensions(
    width: i64,
    height: i64,
    kind: &str,
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    if width <= 0 || height <= 0 {
        return Err(error(format!("dimensões inválidas para {kind} {id}")));
    }
    Ok(())
}

fn validate_rect_within(
    rect: &RectUm,
    surface_width: i64,
    surface_height: i64,
    kind: &str,
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    validate_positive_dimensions(rect.width, rect.height, kind, id, error)?;
    let right = rect.x.checked_add(rect.width);
    let bottom = rect.y.checked_add(rect.height);
    if rect.x < 0
        || rect.y < 0
        || right.is_none_or(|value| value > surface_width)
        || bottom.is_none_or(|value| value > surface_height)
    {
        return Err(error(format!(
            "{kind} {id} ultrapassa a superfície da Lâmina"
        )));
    }
    Ok(())
}

fn validate_palette(
    palette: &[String; 3],
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    if palette.iter().any(|color| {
        color.len() != 7
            || !color.starts_with('#')
            || !color[1..].bytes().all(|value| value.is_ascii_hexdigit())
    }) {
        return Err(error(format!("paleta inválida para {id}")));
    }
    Ok(())
}

fn sample_sheet(number: usize, sheet_count: usize) -> SheetSnapshot {
    let role = if number == 1 {
        SheetRole::Initial
    } else if number == sheet_count {
        SheetRole::Final
    } else {
        SheetRole::Internal
    };
    let left_photo = (number != 2).then(|| sample_photo(number % 3));
    let right_photo = (number != 2).then(|| sample_photo((number + 1) % 3));

    SheetSnapshot {
        id: format!("lamina-{number:02}"),
        number,
        role,
        width_um: SHEET_WIDTH_UM,
        height_um: SHEET_HEIGHT_UM,
        frames: vec![
            FrameSnapshot {
                id: format!("frame-{number:02}-a"),
                rect: RectUm {
                    x: 26_000,
                    y: 28_000,
                    width: 252_000,
                    height: 244_000,
                },
                z_index: 1,
                photo: left_photo,
            },
            FrameSnapshot {
                id: format!("frame-{number:02}-b"),
                rect: RectUm {
                    x: 322_000,
                    y: 42_000,
                    width: 250_000,
                    height: 216_000,
                },
                z_index: 2,
                photo: right_photo,
            },
        ],
        has_overlay: number.is_multiple_of(3),
    }
}

fn sample_photo(index: usize) -> PhotoSnapshot {
    let catalog = sample_media_catalog();
    let item = &catalog[index % catalog.len()];

    photo_from_catalog_item(item)
}

fn photo_from_catalog_item(item: &MediaCatalogItem) -> PhotoSnapshot {
    PhotoSnapshot {
        media_id: item.id.clone(),
        name: item.name.clone(),
        source_width_px: 6_000,
        source_height_px: 4_000,
        palette: item.palette.clone(),
        transform: MediaTransform::default(),
    }
}

fn sample_media_catalog() -> Vec<MediaCatalogItem> {
    vec![
        MediaCatalogItem {
            id: "media-serra".into(),
            name: "Serra ao amanhecer.jpg".into(),
            palette: ["#153448".into(), "#3c7a89".into(), "#f1c27d".into()],
            usage_count: 8,
        },
        MediaCatalogItem {
            id: "media-costa".into(),
            name: "Costa dourada.jpg".into(),
            palette: ["#11212d".into(), "#5b7c8d".into(), "#dca15d".into()],
            usage_count: 8,
        },
        MediaCatalogItem {
            id: "media-campo".into(),
            name: "Campo de inverno.jpg".into(),
            palette: ["#26352e".into(), "#8a9a71".into(), "#e7dcc3".into()],
            usage_count: 8,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{CompositionCore, ProjectCore, ProjectIntent, RectUm};

    #[test]
    fn opens_a_representative_long_album() {
        let session = ProjectCore::open_sample_project(12);

        assert_eq!(session.state().album.sheets.len(), 12);
    }

    #[test]
    fn commits_one_domain_revision_for_a_completed_pan_gesture() {
        let mut session = ProjectCore::open_sample_project(12);

        let state = session
            .apply(ProjectIntent::PanPhoto {
                frame_id: "frame-01-a".into(),
                delta_x: 0.25,
                delta_y: -0.10,
            })
            .expect("the sample frame accepts a pan gesture");

        assert_eq!(state.revision, 1);
        assert_eq!(
            state.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .expect("the sample frame has a photo")
                .transform
                .pan_x,
            0.25
        );
        assert!(state.dirty);
        assert!(state.can_undo);
    }

    #[test]
    fn undo_and_redo_restore_the_document_without_storing_view_state() {
        let mut session = ProjectCore::open_sample_project(12);
        session
            .apply(ProjectIntent::PanPhoto {
                frame_id: "frame-01-a".into(),
                delta_x: 0.40,
                delta_y: 0.20,
            })
            .expect("pan is valid");

        let undone = session.undo().expect("the gesture can be undone");
        assert_eq!(
            undone.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .expect("photo remains in the frame")
                .transform
                .pan_x,
            0.0
        );
        assert!(!undone.dirty);
        assert!(undone.can_redo);

        let redone = session.redo().expect("the gesture can be redone");
        assert_eq!(
            redone.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .expect("photo remains in the frame")
                .transform
                .pan_x,
            0.40
        );
        assert!(redone.dirty);
    }

    #[test]
    fn render_snapshot_uses_the_composition_plan_and_excludes_canvas_navigation() {
        let session = ProjectCore::open_sample_project(12);

        let snapshot = session.render_snapshot();
        let photo = snapshot.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the first composed frame has a photo");

        assert_eq!(
            photo.draw_rect,
            RectUm {
                x: -31_000,
                y: 28_000,
                width: 366_000,
                height: 244_000,
            }
        );

        let wire = serde_json::to_string(&snapshot).expect("snapshot is serializable");
        assert!(!wire.contains("viewport"));
        assert!(!wire.contains("selection"));
    }

    #[test]
    fn loads_a_persisted_revision_for_rendering_without_an_editable_session() {
        let mut session = ProjectCore::open_sample_project(12);
        session
            .apply(ProjectIntent::PanPhoto {
                frame_id: "frame-01-a".into(),
                delta_x: -0.35,
                delta_y: 0.0,
            })
            .expect("pan is valid");
        let persisted = session
            .persisted_revision()
            .expect("sample project can be serialized");

        let loaded = ProjectCore::load_persisted_revision(&persisted)
            .expect("persisted revision can be loaded read-only");

        assert_eq!(loaded.render_snapshot(), session.render_snapshot());
        assert_eq!(loaded.revision(), 1);
    }

    #[test]
    fn the_core_fills_the_leftmost_placeholder_for_a_photo_intent() {
        let mut session = ProjectCore::open_sample_project(12);

        let state = session
            .apply(ProjectIntent::FillLeftmostPlaceholder {
                sheet_id: "lamina-02".into(),
                media_id: "media-campo".into(),
            })
            .expect("the sample sheet has compatible placeholders");

        assert_eq!(
            state.album.sheets[1].frames[0]
                .photo
                .as_ref()
                .expect("leftmost placeholder was filled")
                .media_id,
            "media-campo"
        );
        assert!(state.album.sheets[1].frames[1].photo.is_none());
        assert_eq!(state.revision, 1);
    }

    #[test]
    fn photo_zoom_is_persistent_and_never_goes_below_fill() {
        let mut session = ProjectCore::open_sample_project(12);

        let zoomed = session
            .apply(ProjectIntent::ZoomPhoto {
                frame_id: "frame-01-a".into(),
                delta: 0.35,
            })
            .expect("the sample photo accepts zoom");
        assert_eq!(
            zoomed.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .unwrap()
                .transform
                .user_zoom,
            1.35
        );

        let reset = session
            .apply(ProjectIntent::ZoomPhoto {
                frame_id: "frame-01-a".into(),
                delta: -2.0,
            })
            .expect("zoom is clamped to fill");
        assert_eq!(
            reset.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .unwrap()
                .transform
                .user_zoom,
            1.0
        );
    }

    #[test]
    fn composition_plan_orders_frames_by_visual_stack() {
        let session = ProjectCore::open_sample_project(12);
        let mut album = session.state().album;
        album.sheets[0].frames.reverse();

        let composition = CompositionCore::compose(&album);
        let frame_ids = composition.sheets[0]
            .frames
            .iter()
            .map(|frame| frame.frame_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(frame_ids, ["frame-01-a", "frame-01-b"]);
    }

    #[test]
    fn persisted_revision_rejects_invalid_media_dimensions() {
        let session = ProjectCore::open_sample_project(12);
        let persisted = session
            .persisted_revision()
            .expect("sample project can be serialized");
        let mut document: serde_json::Value =
            serde_json::from_str(&persisted).expect("sample JSON is valid");
        document["album"]["sheets"][0]["frames"][0]["photo"]["sourceWidthPx"] =
            serde_json::json!(0);

        let error = ProjectCore::load_persisted_revision(
            &serde_json::to_string(&document).expect("modified JSON is valid"),
        )
        .err()
        .expect("invalid dimensions must be rejected");

        assert!(error.to_string().contains("dimensões"));
    }

    #[test]
    fn render_snapshot_rejects_empty_internal_identifiers() {
        let session = ProjectCore::open_sample_project(12);
        let mut snapshot = session.render_snapshot();
        snapshot.composition.sheets[0].frames[0].frame_id.clear();

        let error = snapshot
            .validate()
            .expect_err("an empty Frame identifier must be rejected");

        assert!(error.to_string().contains("Identificador de Frame"));
    }
}
