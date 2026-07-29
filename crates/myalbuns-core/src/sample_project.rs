use crate::model::{
    AlbumSnapshot, EditorState, FrameSnapshot, MediaCatalogItem, PhotoSnapshot, RectUm,
    SHEET_HEIGHT_UM, SHEET_WIDTH_UM, SheetRole, SheetSnapshot,
};

pub(crate) fn sample_editor_state(sheet_count: usize) -> EditorState {
    sample_editor_state_with_identity(sheet_count, "project-spike-001", "Álbum Horizonte")
}

pub(crate) fn sample_editor_state_with_identity(
    sheet_count: usize,
    project_id: &str,
    project_name: &str,
) -> EditorState {
    let sheet_count = sheet_count.max(2);
    let sheets = (1..=sheet_count)
        .map(|number| sample_sheet(number, sheet_count))
        .collect();

    EditorState {
        project_id: project_id.into(),
        project_name: project_name.into(),
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
        has_overlay: number.is_multiple_of(3),
    }
}

fn sample_photo(index: usize) -> PhotoSnapshot {
    let catalog = sample_media_catalog();
    let item = &catalog[index % catalog.len()];
    PhotoSnapshot::from_catalog_item(item)
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
