#![cfg(windows)]

//! Projects of the old myAlbuns (SQLite `.myalbuns`, formats 2.0–2.2) open
//! converted in memory and are replaced by the current format only on the
//! first confirmed save (ADR 0012).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use myalbuns_core::{
    DocumentFailure, LegacyConversionNote, LoadProjectError, LoadProjectRequest, MediaId,
    MediaKind, OpenProjectError, OpenProjectRequest, PhotoSourceMetadata, ProjectCore,
    ProjectLocation, SaveAsAuthorization, SaveAsProjectRequest, SaveProjectError,
    SaveProjectOutcome,
};
use myalbuns_paths::OperationPathContext;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

fn project_location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the public path seam captures the Project root");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project_core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

// ---------------------------------------------------------------------------
// Old myAlbuns fixtures, written with the schema of format 2.2

struct LegacyFixture {
    metadata: Value,
    images: Vec<(String, Value)>,
    sheets: Vec<Value>,
    format_version: &'static str,
}

impl LegacyFixture {
    /// Two double Sheets of 5811 × 3614 px at 300 DPI, like the RR templates.
    fn new() -> Self {
        Self {
            metadata: json!({
                "default_background": { "type": "color", "value": "#fefaef" },
                "default_background_type": "spread",
                "default_overlay": null,
                "default_overlay_type": "individual",
                "default_frame_border_enabled": true,
                "default_frame_border_width": 8,
                "default_frame_border_color": "#ffe7b9",
            }),
            images: Vec::new(),
            sheets: vec![
                sheet("a0000000-0000-4000-8000-000000000001", true, true, vec![]),
                sheet("a0000000-0000-4000-8000-000000000002", true, true, vec![]),
            ],
            format_version: "2.2",
        }
    }

    fn image(mut self, id: &str, file_name: &str, is_resource: bool, folder: &str) -> Self {
        self.images.push((
            id.into(),
            json!({
                "filename": file_name,
                "relative_path": format!("{id}.png"),
                "source_relative_path": file_name,
                "size_bytes": 1,
                "source_mtime_ns": 1,
                "width": 3000,
                "height": 2000,
                "format": "PNG",
                "import_date": "2026-09-10T22:24:39.988248",
                "is_resource": is_resource,
                "original_path": format!("S:\\\\Antigo\\\\{file_name}"),
                "folder": folder,
            }),
        ));
        self
    }

    fn write(&self, path: &Path, image_folders: &[&str]) {
        let _ = fs::remove_file(path);
        let connection = Connection::open(path).expect("the old database is created");
        connection
            .execute_batch(
                "CREATE TABLE project_metadata (uuid TEXT PRIMARY KEY, name TEXT NOT NULL, \
                 format_version TEXT NOT NULL, created_date TEXT NOT NULL, modified_date TEXT NOT NULL, \
                 width REAL NOT NULL, height REAL NOT NULL, dpi INTEGER NOT NULL, last_save_date TEXT, \
                 default_settings TEXT, original_file_path TEXT, cut_margin REAL, safe_margin REAL, \
                 image_folders TEXT, layout_favorites_state TEXT, canonical_model_version INTEGER, \
                 file_identity TEXT);
                 CREATE TABLE laminas (uuid TEXT PRIMARY KEY, index_position INTEGER NOT NULL, data TEXT NOT NULL);
                 CREATE TABLE images (image_id TEXT PRIMARY KEY, content_hash TEXT NOT NULL, data TEXT NOT NULL);",
            )
            .expect("the old schema is created");
        connection
            .execute(
                "INSERT INTO project_metadata VALUES (?1, 'Modelo', ?2, '2026-09-10T22:24:39', \
                 '2026-09-22T07:55:41', 5811.0, 3614.0, 300, NULL, ?3, NULL, 0.3, 0.6, ?4, ?5, 2, NULL)",
                params![
                    "2cc441f7-6fd9-5d3e-8254-dc6c4f549bf1",
                    self.format_version,
                    self.metadata.to_string(),
                    json!(image_folders).to_string(),
                    json!({
                        "version": "1.0",
                        "favorite_ids": ["limpo2"],
                        "favorite_templates": { "limpo2": { "frames": [
                            { "x": 0.0, "y": 0.0, "width": 50.0, "height": 100.0 },
                            { "x": 50.0, "y": 0.0, "width": 50.0, "height": 100.0 }
                        ] } }
                    })
                    .to_string(),
                ],
            )
            .expect("the old metadata is written");
        for (id, data) in &self.images {
            connection
                .execute(
                    "INSERT INTO images VALUES (?1, 'hash', ?2)",
                    params![id, data.to_string()],
                )
                .expect("an old image is written");
        }
        for (index, data) in self.sheets.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO laminas VALUES (?1, ?2, ?3)",
                    params![data["id"].as_str().unwrap(), index as i64, data.to_string()],
                )
                .expect("an old Sheet is written");
        }
    }
}

fn sheet(id: &str, left: bool, right: bool, frames: Vec<Value>) -> Value {
    json!({
        "id": id,
        "width": 5811,
        "height": 3614,
        "left_page": { "side": "left", "enabled": left, "background": null, "overlay": null },
        "right_page": { "side": "right", "enabled": right, "background": null, "overlay": null },
        "frames": frames,
        "background": { "mode": "default", "type": "spread",
            "spread": { "type": "color", "value": "#fefaef" }, "left": null, "right": null },
        "overlay": { "mode": "default", "type": "individual", "spread": null, "left": null, "right": null },
        "locked_layout_id": null,
    })
}

fn frame(id: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({
        "id": id,
        "image_id": null,
        "image_size": null,
        "fill_scale": 1.0,
        "geometry": { "x": x, "y": y, "width": width, "height": height },
        "border": { "width": 8, "color": "#ffe7b9", "mode": "default" },
        "transform": { "scale": 1.0, "pan_x": 0, "pan_y": 0, "rotation": 0,
            "angle_offset": 0.0, "flip_horizontal": false },
        "adjustments": { "brightness": 0.0, "contrast": 0.0, "saturation": 0.0 },
        "filter": "none",
        "state": "normal",
        "opacity": 1.0,
        "is_placeholder": true,
    })
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("the saved file is readable"))
        .expect("the saved file is JSON")
}

struct Opened {
    _root: tempfile::TempDir,
    core: ProjectCore,
    path: PathBuf,
}

fn fixture_file(fixture: &LegacyFixture) -> Opened {
    let root = tempfile::tempdir().expect("temporary old Project");
    let path = root.path().join("Modelo RR.myalbuns");
    fixture.write(&path, &[]);
    Opened {
        core: project_core(root.path()),
        path,
        _root: root,
    }
}

// ---------------------------------------------------------------------------
// Opening, saving and identity

#[test]
fn an_old_project_opens_unsaved_without_writing_the_file() {
    let fixture = fixture_file(&LegacyFixture::new());
    let before = fs::read(&fixture.path).unwrap();

    let project = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .expect("the old Project opens");

    assert!(project.requires_format_conversion());
    assert!(project.has_unsaved_changes());
    let projection = project.projection();
    assert!(projection.state.dirty);
    assert!(projection.state.format_conversion_pending);
    assert_eq!(projection.state.project_name, "Modelo RR");
    assert_eq!(project.project().sheets().len(), 2);
    drop(project);
    assert_eq!(
        fs::read(&fixture.path).unwrap(),
        before,
        "opening never writes"
    );
}

#[test]
fn the_first_save_needs_confirmation_and_then_replaces_the_old_file() {
    let fixture = fixture_file(&LegacyFixture::new());
    let before = fs::read(&fixture.path).unwrap();
    let mut project = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .unwrap();
    let revision = project.revision();

    assert_eq!(
        project.save(revision),
        Err(SaveProjectError::FormatConversionConfirmationRequired)
    );
    assert_eq!(
        fs::read(&fixture.path).unwrap(),
        before,
        "an unconfirmed save writes nothing"
    );

    assert_eq!(
        project.save_converting_format(revision),
        Ok(SaveProjectOutcome::Saved { revision })
    );
    assert!(!project.requires_format_conversion());
    assert!(!project.has_unsaved_changes());
    let saved = saved_json(&fixture.path);
    assert_eq!(saved["documentType"], "myalbuns.project");
    assert_eq!(saved["projectId"], project.project_id().to_string());
    let project_id = project.project_id();
    drop(project);

    let reopened = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .expect("the saved Project reopens in the current format");
    assert!(!reopened.requires_format_conversion());
    assert!(!reopened.has_unsaved_changes());
    assert_eq!(reopened.project_id(), project_id);
}

#[test]
fn reopening_the_same_unsaved_old_file_keeps_its_pending_identity() {
    let fixture = fixture_file(&LegacyFixture::new());
    let open = || {
        fixture
            .core
            .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
            .unwrap()
    };
    let first = open();
    let first_id = first.project_id();
    assert_eq!(first_id.get_version_num(), 4);
    assert_ne!(first_id.to_string(), "2cc441f7-6fd9-5d3e-8254-dc6c4f549bf1");
    drop(first);

    assert_eq!(open().project_id(), first_id);
}

#[test]
fn a_second_opening_while_the_old_project_is_open_focuses_the_first() {
    let fixture = fixture_file(&LegacyFixture::new());
    let first = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .unwrap();

    let second = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)));

    assert!(
        matches!(second, Err(OpenProjectError::FocusExisting { project_id, .. }) if project_id == first.project_id()),
        "{second:?}"
    );
}

#[test]
fn save_as_writes_the_current_format_elsewhere_and_keeps_the_old_file() {
    let fixture = fixture_file(&LegacyFixture::new());
    let before = fs::read(&fixture.path).unwrap();
    let copy = fixture.path.with_file_name("Cópia.myalbuns");
    let mut project = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .unwrap();

    project
        .save_as(SaveAsProjectRequest::new(
            project.revision(),
            project_location(&copy),
            SaveAsAuthorization::CreateOnly,
        ))
        .expect("the old Project is saved elsewhere");

    assert!(!project.requires_format_conversion());
    assert!(!project.has_unsaved_changes());
    assert_eq!(saved_json(&copy)["documentType"], "myalbuns.project");
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn a_read_only_load_converts_without_writing() {
    let fixture = fixture_file(&LegacyFixture::new());
    let before = fs::read(&fixture.path).unwrap();

    let loaded = fixture
        .core
        .load_persisted_revision(LoadProjectRequest::new(project_location(&fixture.path)))
        .expect("batch operations read the old Project");

    assert_eq!(loaded.project().sheets().len(), 2);
    assert_eq!(fs::read(&fixture.path).unwrap(), before);
}

#[test]
fn an_old_project_being_written_by_the_old_program_is_refused() {
    let fixture = fixture_file(&LegacyFixture::new());
    let mut journal = fixture.path.clone().into_os_string();
    journal.push("-journal");
    fs::write(&journal, b"pending transaction").unwrap();

    let error = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .expect_err("a journal means the old program is writing");
    assert_eq!(
        error,
        OpenProjectError::Document(DocumentFailure::LegacyProjectInUse)
    );
    assert_eq!(
        fixture
            .core
            .load_persisted_revision(LoadProjectRequest::new(project_location(&fixture.path)))
            .err(),
        Some(LoadProjectError::Document(
            DocumentFailure::LegacyProjectInUse
        ))
    );
}

#[test]
fn formats_before_2_0_and_single_sided_middle_sheets_are_refused() {
    let mut old = LegacyFixture::new();
    old.format_version = "1.1";
    let fixture = fixture_file(&old);
    assert_eq!(
        fixture
            .core
            .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
            .err(),
        Some(OpenProjectError::Document(
            DocumentFailure::LegacyProjectOldVersion
        ))
    );

    let mut middle = LegacyFixture::new();
    middle.sheets.insert(
        1,
        sheet("a0000000-0000-4000-8000-000000000003", false, true, vec![]),
    );
    let fixture = fixture_file(&middle);
    assert_eq!(
        fixture
            .core
            .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
            .err(),
        Some(OpenProjectError::Document(
            DocumentFailure::LegacyProjectUnsupportedStructure {
                sheet_number: Some(2)
            }
        ))
    );
}

// ---------------------------------------------------------------------------
// Conversion

fn convert(fixture: &LegacyFixture, files: &[&str]) -> (Value, Vec<LegacyConversionNote>) {
    let opened = fixture_file(fixture);
    for file in files {
        fs::write(opened.path.with_file_name(file), b"image").unwrap();
    }
    let mut project = opened
        .core
        .open_editable(OpenProjectRequest::new(project_location(&opened.path)))
        .expect("the old Project converts");
    let notes = project.legacy_conversion_notes().to_vec();
    project
        .save_converting_format(project.revision())
        .expect("the converted Project is saved");
    (saved_json(&opened.path), notes)
}

#[test]
fn album_measures_margins_and_defaults_keep_the_old_output() {
    let (saved, _) = convert(&LegacyFixture::new(), &[]);
    let project = &saved["project"];

    assert_eq!(
        project["album"],
        json!({
            "displayUnit": "cm",
            "sheetWidthUm": 492000,
            "sheetHeightUm": 306000,
            "dpi": 300,
            "bleedUm": 3000,
            // The old safety margin was measured from the outer edge.
            "safetyUm": 3000,
        })
    );
    assert_eq!(
        project["visualDefaults"],
        json!({
            "background": { "sides": "both", "both": { "kind": "color", "rgb": "#FEFAEF" } },
            "overlay": { "sides": "perSide", "left": { "kind": "none" }, "right": { "kind": "none" } },
            "frameBorder": { "kind": "solid", "rgb": "#FFE7B9", "widthUm": 2800 },
        })
    );
    assert_eq!(project["layoutSettings"]["permission"], "pagesAndSheet");
}

#[test]
fn images_resolve_beside_the_project_keep_their_folders_and_roles() {
    let mut fixture = LegacyFixture::new()
        .image("img_moldura", "MOLDURA.png", true, "")
        .image("img_foto", "Foto 1.jpg", false, "P02326")
        .image("img_ausente", "Ausente.jpg", false, "P02326");
    fixture.metadata["default_overlay"] = json!({ "type": "image", "value": "img_moldura" });
    let mut photo_frame = frame(
        "b0000000-0000-4000-8000-000000000001",
        100.0,
        100.0,
        1000.0,
        1200.0,
    );
    photo_frame["image_id"] = json!("img_foto");
    photo_frame["image_size"] = json!([3000, 2000]);
    fixture.sheets[1]["frames"] = json!([photo_frame]);

    let (saved, notes) = convert(&fixture, &["MOLDURA.png", "Foto 1.jpg"]);
    let project = &saved["project"];
    let media = project["media"].as_array().unwrap();

    let kinds: Vec<_> = media
        .iter()
        .map(|item| item["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["decorative", "photo", "photo"]);
    assert!(
        media[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("\\MOLDURA.png")
    );
    assert!(media[1]["path"].as_str().unwrap().ends_with("\\Foto 1.jpg"));
    // Found nowhere: the path beside the Project is kept, so the current
    // program shows it as missing and can locate it.
    assert!(
        media[2]["path"]
            .as_str()
            .unwrap()
            .ends_with("\\Ausente.jpg")
    );
    assert_ne!(media[2]["path"], "S:\\Antigo\\Ausente.jpg");
    assert!(notes.contains(&LegacyConversionNote::MissingImages { count: 1 }));

    let folders = project["mediaFolders"].as_array().unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0]["name"], "P02326");
    assert_eq!(folders[0]["kind"], "photo");
    assert_eq!(folders[0]["mediaIds"].as_array().unwrap().len(), 2);

    let overlay = &project["visualDefaults"]["overlay"];
    assert_eq!(overlay["left"]["mediaId"], media[0]["id"]);
    assert_eq!(overlay["right"]["mediaId"], media[0]["id"]);
    assert_eq!(
        project["sheets"][1]["frames"][0]["photo"]["mediaId"],
        media[1]["id"]
    );
}

#[test]
fn a_cover_frame_is_placed_relative_to_its_single_active_page() {
    let mut fixture = LegacyFixture::new();
    fixture.sheets[0] = sheet(
        "a0000000-0000-4000-8000-000000000001",
        false,
        true,
        vec![frame(
            "b0000000-0000-4000-8000-000000000001",
            3391.8313421828907,
            315.75907079646015,
            1900.4140117994093,
            2305.5231563421826,
        )],
    );
    let (saved, _) = convert(&fixture, &[]);
    let sheet = &saved["project"]["sheets"][0];

    assert_eq!(sheet["activeSides"], "right");
    let frame = &sheet["frames"][0];
    // 3391.83 px from the Sheet edge is 287 175 µm; the right page starts
    // at 246 000 µm.
    assert_eq!(frame["xUm"], 41175);
    assert_eq!(frame["yUm"], 26734);
    assert_eq!(frame["widthUm"], 160902);
    assert_eq!(frame["heightUm"], 195201);
    assert!(
        frame.get("style").is_none(),
        "the default border follows the Album"
    );
}

#[test]
fn sheet_visuals_keep_only_what_differs_from_the_album() {
    let mut fixture = LegacyFixture::new()
        .image("img_moldura", "MOLDURA.png", true, "")
        .image("img_ab", "AB.png", true, "")
        .image("img_final", "Mensagem Final.png", true, "");
    fixture.metadata["default_overlay"] = json!({ "type": "image", "value": "img_moldura" });
    fixture.sheets[0]["overlay"] = json!({ "mode": "custom", "type": "individual", "spread": null,
        "left": { "type": "image", "value": "img_moldura" },
        "right": { "type": "image", "value": "img_ab" } });
    fixture.sheets[1]["background"] = json!({ "mode": "custom", "type": "individual", "spread": null,
        "left": { "type": "color", "value": "#fefaef" },
        "right": { "type": "image", "value": "img_final" } });
    fixture.sheets[1]["overlay"] = json!({ "mode": "custom", "type": "individual", "spread": null,
        "left": { "type": "image", "value": "img_moldura" }, "right": null });

    let (saved, _) = convert(&fixture, &[]);
    let sheets = &saved["project"]["sheets"];
    let media = &saved["project"]["media"];

    assert_eq!(
        sheets[0]["visuals"],
        json!({ "overlay": { "sides": "perSide",
            "right": { "content": { "kind": "media", "mediaId": media[1]["id"] } } } })
    );
    assert_eq!(
        sheets[1]["visuals"],
        json!({
            "background": { "sides": "perSide",
                "right": { "content": { "kind": "media", "mediaId": media[2]["id"] } } },
            "overlay": { "sides": "perSide", "right": { "content": { "kind": "none" } } },
        })
    );
}

#[test]
fn frame_style_follows_the_old_effective_border_and_opacity() {
    let mut fixture = LegacyFixture::new();
    let mut without_border = frame(
        "b0000000-0000-4000-8000-000000000001",
        10.0,
        10.0,
        500.0,
        500.0,
    );
    without_border["border"]["width"] = json!(0);
    let mut translucent = frame(
        "b0000000-0000-4000-8000-000000000002",
        600.0,
        10.0,
        500.0,
        500.0,
    );
    translucent["opacity"] = json!(0.65);
    fixture.sheets[1]["frames"] = json!([without_border, translucent]);

    let (saved, _) = convert(&fixture, &[]);
    let frames = &saved["project"]["sheets"][1]["frames"];

    assert_eq!(
        frames[0]["style"],
        json!({ "borderRgb": "#FFE7B9", "borderWidthUm": 0, "opacityPercent": 100 })
    );
    assert_eq!(
        frames[1]["style"],
        json!({ "borderRgb": "#FFE7B9", "borderWidthUm": 2800, "opacityPercent": 65 })
    );
}

#[test]
fn features_the_current_program_lacks_are_dropped_and_noted() {
    let mut fixture = LegacyFixture::new().image("img_foto", "Foto.jpg", false, "");
    let mut sepia = frame(
        "b0000000-0000-4000-8000-000000000001",
        10.0,
        10.0,
        500.0,
        500.0,
    );
    sepia["image_id"] = json!("img_foto");
    sepia["filter"] = json!("sepia");
    sepia["adjustments"]["brightness"] = json!(0.2);
    sepia["transform"]["scale"] = json!(5.0);
    let mut black_and_white = frame(
        "b0000000-0000-4000-8000-000000000002",
        600.0,
        10.0,
        500.0,
        500.0,
    );
    black_and_white["image_id"] = json!("img_foto");
    black_and_white["filter"] = json!("pb");
    fixture.sheets[1]["frames"] = json!([sepia, black_and_white]);

    let (saved, notes) = convert(&fixture, &["Foto.jpg"]);
    let frames = &saved["project"]["sheets"][1]["frames"];

    assert_eq!(frames[0]["photo"]["transform"]["userZoom"], 4.0);
    assert!(
        frames[0]["photo"]["transform"]
            .get("blackAndWhite")
            .is_none()
    );
    assert_eq!(frames[1]["photo"]["transform"]["blackAndWhite"], true);
    assert!(notes.contains(&LegacyConversionNote::PhotoFilterDropped {
        sheet_number: 2,
        filter: "sepia".into()
    }));
    assert!(notes.contains(&LegacyConversionNote::PhotoAdjustmentsDropped { sheet_number: 2 }));
    assert!(notes.contains(&LegacyConversionNote::PhotoZoomLimited { sheet_number: 2 }));
}

#[test]
fn favorite_layouts_and_locked_layouts_are_kept() {
    let mut fixture = LegacyFixture::new();
    fixture.sheets[1]["frames"] = json!([
        frame(
            "b0000000-0000-4000-8000-000000000001",
            100.0,
            100.0,
            2000.0,
            3000.0
        ),
        frame(
            "b0000000-0000-4000-8000-000000000002",
            3000.0,
            100.0,
            2000.0,
            3000.0
        ),
    ]);
    fixture.sheets[1]["locked_layout_id"] = json!("v220clean");

    let (saved, notes) = convert(&fixture, &[]);
    let project = &saved["project"];

    assert!(notes.is_empty(), "{notes:?}");
    let favorites = project["favoriteLayouts"].as_array().unwrap();
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0]["layout"]["origin"], "custom");
    assert_eq!(
        favorites[0]["layout"]["positions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let locked = &project["sheets"][1];
    assert_eq!(locked["layoutLocked"], true);
    assert_eq!(
        locked["lastLayout"]["positions"].as_array().unwrap().len(),
        2
    );
}

#[test]
fn a_read_only_export_composes_photos_with_their_observed_dimensions() {
    let mut fixture = LegacyFixture::new().image("img_foto", "Foto.jpg", false, "");
    let mut photo_frame = frame(
        "b0000000-0000-4000-8000-000000000001",
        100.0,
        100.0,
        1200.0,
        1200.0,
    );
    photo_frame["image_id"] = json!("img_foto");
    photo_frame["image_size"] = json!([3000, 2000]);
    fixture.sheets[1]["frames"] = json!([photo_frame]);
    let opened = fixture_file(&fixture);
    let loaded = opened
        .core
        .load_persisted_revision(LoadProjectRequest::new(project_location(&opened.path)))
        .unwrap();
    let photo_id = MediaId::try_from(
        loaded
            .project()
            .media()
            .iter()
            .find(|media| media.kind() == MediaKind::Photo)
            .unwrap()
            .id(),
    )
    .unwrap();
    let draw_rect = |frozen: myalbuns_core::FrozenProjectRendering| {
        frozen.render_snapshot().composition.sheets[1].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .draw_rect
            .clone()
    };

    let unobserved = draw_rect(loaded.freeze_rendering());
    let observed = draw_rect(
        loaded.freeze_rendering_with_photo_sources(&HashMap::from([(
            photo_id,
            PhotoSourceMetadata::new(
                3000,
                2000,
                ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
            )
            .unwrap(),
        )])),
    );

    // Without its dimensions a Photo is composed as a 1 × 1 source and would be
    // stretched into a square; the batch export observes every Photo first.
    assert_eq!(unobserved.width, unobserved.height);
    let aspect = observed.width as f64 / observed.height as f64;
    assert!((aspect - 1.5).abs() < 1e-3, "{observed:?}");
}

// ---------------------------------------------------------------------------
// Real files, when their local copies are given

/// Opens every `.myalbuns` of the old program in `MYALBUNS_LEGACY_SAMPLES`
/// (local copies of client files, never committed) and saves a converted
/// copy beside it, for comparison with the old program.
#[test]
fn local_copies_of_real_old_projects_convert() {
    let Ok(directory) = std::env::var("MYALBUNS_LEGACY_SAMPLES") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let core = project_core(root.path());
    for entry in fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_none_or(|extension| extension != "myalbuns")
        {
            continue;
        }
        let mut project = core
            .open_editable(OpenProjectRequest::new(project_location(&path)))
            .unwrap_or_else(|error| panic!("{} did not open: {error:?}", path.display()));
        println!(
            "{}: {} lâminas, {} mídias, notas {:?}",
            path.display(),
            project.project().sheets().len(),
            project.project().media().len(),
            project.legacy_conversion_notes()
        );
        let converted = path.with_extension("convertido.json");
        let _ = fs::remove_file(&converted);
        project
            .save_as(SaveAsProjectRequest::new(
                project.revision(),
                project_location(&converted),
                SaveAsAuthorization::CreateOnly,
            ))
            .unwrap_or_else(|error| panic!("{} did not save: {error:?}", path.display()));
    }
}
