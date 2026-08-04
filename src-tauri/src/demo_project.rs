//! Projeto demonstrativo temporário usado enquanto o fluxo de criar/abrir
//! Projetos ainda não faz parte do produto.
//!
//! Este módulo pertence somente ao composition root do desktop. Ele não é uma
//! segunda forma de construir Sessões: o documento é sempre aberto pelo
//! `ProjectCore`, exatamente como um Projeto persistido.

use myalbuns_core::{
    AlbumSnapshot, EditorState, FrameSnapshot, MediaCatalogItem, MediaKind, MediaTransform,
    PhotoSnapshot, ProjectCore, RectUm, SheetRole, SheetSnapshot,
};
use myalbuns_imaging_protocol::MediaSource;
use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};

use crate::project_host::ProjectHost;

const PROJECT_ID: &str = "demo-project-horizon";
const PROJECT_NAME: &str = "Álbum Horizonte";
const SHEET_COUNT: usize = 12;
const SHEET_WIDTH_UM: i64 = 600_000;
const SHEET_HEIGHT_UM: i64 = 300_000;
const DEMO_IMAGE: &[u8] = include_bytes!("../icons/128x128.png");

pub(crate) fn open(app_paths: &AppPaths) -> Result<ProjectHost, String> {
    let core = ProjectCore::new();
    let source = persisted_source().map_err(|error| error.to_string())?;
    let session = core
        .open_demo_editable_session(&source)
        .map_err(|error| error.to_string())?;
    Ok(ProjectHost::new(session, materialize_media(app_paths)?))
}

fn materialize_media(app_paths: &AppPaths) -> Result<Vec<MediaSource>, String> {
    let media_directory = app_paths.state_dir().join("Demo").join("Media");
    std::fs::create_dir_all(&media_directory)
        .map_err(|error| format!("Não foi possível preparar as mídias da demonstração: {error}"))?;
    let source_path = media_directory.join("demo-image.png");
    if std::fs::read(&source_path).ok().as_deref() != Some(DEMO_IMAGE) {
        std::fs::write(&source_path, DEMO_IMAGE).map_err(|error| {
            format!("Não foi possível materializar a mídia da demonstração: {error}")
        })?;
    }
    let source_sha256 = format!("{:x}", Sha256::digest(DEMO_IMAGE));
    [
        "media-serra",
        "media-costa",
        "media-campo",
        "decorative-overlay",
    ]
    .into_iter()
    .map(|media_id| {
        MediaSource::new(
            media_id,
            source_path.clone(),
            DEMO_IMAGE.len() as u64,
            source_sha256.clone(),
        )
    })
    .collect()
}

fn persisted_source() -> Result<String, serde_json::Error> {
    let state = editor_state();
    serde_json::to_string_pretty(&serde_json::json!({
        "schemaVersion": 3,
        "projectId": state.project_id,
        "projectName": state.project_name,
        "revision": state.revision,
        "album": state.album,
    }))
}

fn editor_state() -> EditorState {
    EditorState {
        project_id: PROJECT_ID.into(),
        project_name: PROJECT_NAME.into(),
        album: AlbumSnapshot {
            sheets: (1..=SHEET_COUNT).map(sheet).collect(),
            media: media_catalog(),
        },
        revision: 0,
        saved_revision: 0,
        dirty: false,
        can_undo: false,
        can_redo: false,
    }
}

fn sheet(number: usize) -> SheetSnapshot {
    let role = if number == 1 {
        SheetRole::Initial
    } else if number == SHEET_COUNT {
        SheetRole::Final
    } else {
        SheetRole::Internal
    };
    let left_photo = (number != 2).then(|| photo(number % 3));
    let right_photo = (number != 2).then(|| photo((number + 1) % 3));

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

fn photo(index: usize) -> PhotoSnapshot {
    let catalog = media_catalog();
    let photos = catalog
        .iter()
        .filter(|item| item.kind == MediaKind::Photo)
        .collect::<Vec<_>>();
    PhotoSnapshot {
        media_id: photos[index % photos.len()].id.clone(),
        transform: MediaTransform::default(),
    }
}

fn media_catalog() -> Vec<MediaCatalogItem> {
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

#[cfg(test)]
mod tests {
    use myalbuns_paths::AppPaths;

    use super::{PROJECT_ID, SHEET_COUNT, open};

    #[test]
    fn demo_still_enters_through_project_core_as_one_editable_session() {
        let directory = tempfile::tempdir().expect("the temporary root exists");
        let paths = AppPaths::from_roots(directory.path(), directory.path(), directory.path());
        let host = open(&paths).expect("the temporary demo project opens");
        let projection = host.projection().expect("the demo session is available");

        assert!(
            paths
                .state_dir()
                .join("Demo")
                .join("Media")
                .join("demo-image.png")
                .is_file(),
            "the demo source is materialized under the centralized State category"
        );
        assert!(
            !paths.local_root().join("Demo").exists(),
            "the demo does not create an application-root data category"
        );
        assert_eq!(projection.state.project_id, PROJECT_ID);
        assert_eq!(projection.state.album.sheets.len(), SHEET_COUNT);
        assert_eq!(projection.composition.sheets.len(), SHEET_COUNT);
    }
}
