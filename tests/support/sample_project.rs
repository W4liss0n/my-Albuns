use myalbuns_core::{
    AlbumSnapshot, EditorState, FrameSnapshot, MediaCatalogItem, MediaKind, MediaTransform,
    PhotoSnapshot, RectUm, SheetRole, SheetSnapshot,
};

const SHEET_WIDTH_UM: i64 = 600_000;
const SHEET_HEIGHT_UM: i64 = 300_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum SampleProject {
    Horizon,
    Aurora,
}

impl SampleProject {
    pub fn project_id(self) -> &'static str {
        match self {
            Self::Horizon => "project-spike-001",
            Self::Aurora => "project-spike-002",
        }
    }

    pub fn project_name(self) -> &'static str {
        match self {
            Self::Horizon => "Álbum Horizonte",
            Self::Aurora => "Álbum Aurora",
        }
    }

    /// Serializes the deterministic fixture used by the executable spike.
    ///
    /// Consumers still open it through `ProjectCore`, so fixtures cannot become
    /// a second session-construction seam.
    pub fn persisted_source(self, sheet_count: usize) -> Result<String, serde_json::Error> {
        let state = sample_editor_state(sheet_count, self);
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 3,
            "projectId": state.project_id,
            "projectName": state.project_name,
            "revision": state.revision,
            "album": state.album,
        }))
    }
}

fn sample_editor_state(sheet_count: usize, sample_project: SampleProject) -> EditorState {
    let sheet_count = sheet_count.max(2);
    let sheets = (1..=sheet_count)
        .map(|number| sample_sheet(number, sheet_count))
        .collect();

    EditorState {
        project_id: sample_project.project_id().into(),
        project_name: sample_project.project_name().into(),
        album: AlbumSnapshot {
            sheets,
            media: sample_media_catalog(),
        },
        revision: 0,
        saved_revision: 0,
        dirty: false,
        can_undo: false,
        can_redo: false,
    }
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
        overlay_media_id: (number == 1).then(|| "decorative-overlay".into()),
    }
}

fn sample_photo(index: usize) -> PhotoSnapshot {
    let catalog = sample_media_catalog();
    let photos = catalog
        .iter()
        .filter(|item| item.kind == MediaKind::Photo)
        .collect::<Vec<_>>();
    let item = photos[index % photos.len()];
    PhotoSnapshot {
        media_id: item.id.clone(),
        transform: MediaTransform::default(),
    }
}

fn sample_media_catalog() -> Vec<MediaCatalogItem> {
    vec![
        MediaCatalogItem {
            id: "media-serra".into(),
            kind: MediaKind::Photo,
            name: "Serra ao amanhecer.jpg".into(),
            source_width_px: 6_000,
            source_height_px: 4_000,
            palette: ["#153448".into(), "#3c7a89".into(), "#f1c27d".into()],
        },
        MediaCatalogItem {
            id: "media-costa".into(),
            kind: MediaKind::Photo,
            name: "Costa dourada.jpg".into(),
            source_width_px: 6_000,
            source_height_px: 4_000,
            palette: ["#11212d".into(), "#5b7c8d".into(), "#dca15d".into()],
        },
        MediaCatalogItem {
            id: "media-campo".into(),
            kind: MediaKind::Photo,
            name: "Campo de inverno.jpg".into(),
            source_width_px: 6_000,
            source_height_px: 4_000,
            palette: ["#26352e".into(), "#8a9a71".into(), "#e7dcc3".into()],
        },
        MediaCatalogItem {
            id: "decorative-overlay".into(),
            kind: MediaKind::Decorative,
            name: "Overlay translúcido.png".into(),
            source_width_px: 2_400,
            source_height_px: 1_800,
            palette: ["#17344a".into(), "#88b7c5".into(), "#d4a15e".into()],
        },
    ]
}
