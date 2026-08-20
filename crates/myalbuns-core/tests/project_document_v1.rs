use std::{fs, path::Path};

use myalbuns_core::{
    ActiveSides, AlbumInformation, AlbumInformationImpact, Background, BackgroundContent,
    CoreError, CreateAuthorization, CreateProjectError, CreateProjectRequest, DisplayUnit,
    DocumentFailure, EndSheetFormat, FrameBorder, InitialBackground, InitialBackgroundContent,
    InitialFrameBorder, InitialOverlay, InitialOverlayContent, InitialProject,
    InitialProjectConfiguration, InitialProjectPersonalization, LoadProjectError,
    LoadProjectRequest, LoadedProjectRevision, OpenProjectError, OpenProjectRequest, Overlay,
    OverlayContent, PathFailure, ProjectConfigurationValidationError as ValidationError,
    ProjectCore, ProjectIntent, ProjectLocation, ProjectedActiveSides, ProjectedDisplayUnit, Rgb,
    SaveProjectError, SaveProjectOutcome, SheetRole,
};
use myalbuns_paths::{OperationPathContext, ProjectTransitionBarrier, project_data_namespace};

const NEUTRAL_PROJECT_V1: &str = r##"{
  "documentType": "myalbuns.project",
  "schemaVersion": 1,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 0,
  "project": {
    "document": {
      "displayUnit": "mm",
      "sheetWidthUm": 600000,
      "sheetHeightUm": 300000,
      "dpi": 300,
      "bleedUm": 3000,
      "safetyUm": 3000
    },
    "visualDefaults": {
      "background": {
        "scope": "bothSides",
        "both": { "kind": "color", "rgb": "#FFFFFF" }
      },
      "overlay": { "scope": "bothSides", "both": null },
      "frameBorder": { "kind": "none" }
    },
    "media": [],
    "sheets": [
      {
        "id": "00000000-0000-4000-8000-000000000001",
        "activeSides": "both"
      },
      {
        "id": "00000000-0000-4000-8000-000000000002",
        "activeSides": "both"
      }
    ]
  }
}"##;

const PER_SIDE_PROJECT_V1: &str = r##"{
  "documentType": "myalbuns.project",
  "schemaVersion": 1,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 37,
  "project": {
    "document": {
      "displayUnit": "mm",
      "sheetWidthUm": 600000,
      "sheetHeightUm": 300000,
      "dpi": 300,
      "bleedUm": 3000,
      "safetyUm": 3000
    },
    "visualDefaults": {
      "background": {
        "scope": "perSide",
        "left": {
          "kind": "media",
          "mediaId": "00000000-0000-4000-8000-000000000010"
        },
        "right": {
          "kind": "media",
          "mediaId": "00000000-0000-4000-8000-000000000011"
        }
      },
      "overlay": {
        "scope": "perSide",
        "left": null,
        "right": {
          "kind": "media",
          "mediaId": "00000000-0000-4000-8000-000000000011"
        }
      },
      "frameBorder": { "kind": "none" }
    },
    "media": [
      {
        "id": "00000000-0000-4000-8000-000000000010",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [67, 58, 92, 70, 111, 116, 111, 115, 92, 99, 97, 112, 97, 55296, 46, 112, 110, 103]
        }
      },
      {
        "id": "00000000-0000-4000-8000-000000000011",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [92, 92, 115, 101, 114, 118, 105, 100, 111, 114, 92, 65, 108, 98, 117, 110, 115, 92, 111, 118, 101, 114, 108, 97, 121, 46, 112, 110, 103]
        }
      },
      {
        "id": "00000000-0000-4000-8000-000000000012",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [90, 58, 92, 100, 101, 99, 111, 114, 97, 116, 105, 118, 111, 115, 92, 109, 97, 112, 101, 97, 100, 111, 46, 112, 110, 103]
        }
      },
      {
        "id": "00000000-0000-4000-8000-000000000013",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [92, 92, 63, 92, 67, 58, 92, 70, 111, 116, 111, 115, 92, 118, 101, 114, 98, 97, 116, 105, 109, 46, 112, 110, 103]
        }
      },
      {
        "id": "00000000-0000-4000-8000-000000000014",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [92, 92, 63, 92, 85, 78, 67, 92, 115, 101, 114, 118, 105, 100, 111, 114, 92, 65, 108, 98, 117, 110, 115, 92, 118, 101, 114, 98, 97, 116, 105, 109, 46, 112, 110, 103]
        }
      }
    ],
    "sheets": [
      {
        "id": "00000000-0000-4000-8000-000000000001",
        "activeSides": "right"
      },
      {
        "id": "00000000-0000-4000-8000-000000000002",
        "activeSides": "both"
      },
      {
        "id": "00000000-0000-4000-8000-000000000003",
        "activeSides": "left"
      }
    ]
  }
}"##;

#[test]
fn loads_the_neutral_v1_document_without_rewriting_it_or_creating_an_editable_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Álbum neutro.myalbuns");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("fixture is written");
    let original = fs::read(&project_path).expect("fixture can be read");

    let loaded = ProjectCore::new()
        .load_persisted_revision(load_request(&project_path))
        .expect("the public v1 document loads");

    assert_eq!(
        loaded.project_id().to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(loaded.revision(), 0);
    assert_eq!(loaded.project().document().display_unit(), DisplayUnit::Mm);
    assert_eq!(loaded.project().document().sheet_width_um(), 600_000);
    assert_eq!(loaded.project().document().sheet_height_um(), 300_000);
    assert_eq!(loaded.project().document().dpi(), 300);
    assert_eq!(loaded.project().document().bleed_um(), 3_000);
    assert_eq!(loaded.project().document().safety_um(), 3_000);
    assert!(loaded.project().media().is_empty());
    assert_eq!(loaded.project().sheets().len(), 2);
    assert_eq!(
        loaded.project().sheets()[0].active_sides(),
        ActiveSides::Both
    );
    assert_eq!(
        loaded.project().sheets()[1].active_sides(),
        ActiveSides::Both
    );
    assert_eq!(
        fs::read(&project_path).expect("fixture remains readable"),
        original,
        "read-only loading must never rewrite the origin"
    );
}

#[cfg(windows)]
#[test]
fn loads_a_complete_per_side_document_and_preserves_native_windows_path_units() {
    use std::os::windows::ffi::OsStrExt;

    let loaded = load_bytes(PER_SIDE_PROJECT_V1.as_bytes())
        .expect("the complete per-side v1 document loads");
    let project = loaded.project();

    assert_eq!(loaded.revision(), 37);
    assert_eq!(project.media().len(), 5);
    assert_eq!(
        project.media()[0].id().to_string(),
        "00000000-0000-4000-8000-000000000010"
    );
    assert_eq!(
        project.media()[1].id().to_string(),
        "00000000-0000-4000-8000-000000000011"
    );
    assert_eq!(
        project.media()[0]
            .path()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        [
            67, 58, 92, 70, 111, 116, 111, 115, 92, 99, 97, 112, 97, 55_296, 46, 112, 110, 103,
        ],
        "an unpaired UTF-16 unit must survive without lossy Unicode conversion"
    );
    assert_eq!(
        project.media()[1]
            .path()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        [
            92, 92, 115, 101, 114, 118, 105, 100, 111, 114, 92, 65, 108, 98, 117, 110, 115, 92,
            111, 118, 101, 114, 108, 97, 121, 46, 112, 110, 103,
        ]
    );
    for (index, expected_units) in [
        vec![
            90, 58, 92, 100, 101, 99, 111, 114, 97, 116, 105, 118, 111, 115, 92, 109, 97, 112, 101,
            97, 100, 111, 46, 112, 110, 103,
        ],
        vec![
            92, 92, 63, 92, 67, 58, 92, 70, 111, 116, 111, 115, 92, 118, 101, 114, 98, 97, 116,
            105, 109, 46, 112, 110, 103,
        ],
        vec![
            92, 92, 63, 92, 85, 78, 67, 92, 115, 101, 114, 118, 105, 100, 111, 114, 92, 65, 108,
            98, 117, 110, 115, 92, 118, 101, 114, 98, 97, 116, 105, 109, 46, 112, 110, 103,
        ],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            project.media()[index + 2]
                .path()
                .as_os_str()
                .encode_wide()
                .collect::<Vec<_>>(),
            expected_units
        );
    }

    let visual_defaults = project.visual_defaults();
    match visual_defaults.background() {
        Background::PerSide { left, right } => {
            assert_background_media(left, "00000000-0000-4000-8000-000000000010");
            assert_background_media(right, "00000000-0000-4000-8000-000000000011");
        }
        other => panic!("expected a per-side Background, received {other:?}"),
    }
    match visual_defaults.overlay() {
        Overlay::PerSide { left, right } => {
            assert!(left.is_none());
            match right {
                Some(OverlayContent::Media { media_id }) => {
                    assert_eq!(media_id.to_string(), "00000000-0000-4000-8000-000000000011")
                }
                other => panic!("expected the right Overlay media, received {other:?}"),
            }
        }
        other => panic!("expected a per-side Overlay, received {other:?}"),
    }
    assert!(matches!(visual_defaults.frame_border(), FrameBorder::None));
    assert_eq!(project.sheets().len(), 3);
    assert_eq!(project.sheets()[0].active_sides(), ActiveSides::Right);
    assert_eq!(project.sheets()[1].active_sides(), ActiveSides::Both);
    assert_eq!(project.sheets()[2].active_sides(), ActiveSides::Left);
}

#[test]
fn classifies_document_type_and_schema_failures_with_public_typed_errors() {
    let cases = [
        (
            "documentType ausente",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "  \"documentType\": \"myalbuns.project\",\n",
                "",
            ),
            DocumentFailure::InvalidDocumentType,
        ),
        (
            "documentType incorreto",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"documentType\": \"myalbuns.project\"",
                "\"documentType\": \"other.document\"",
            ),
            DocumentFailure::InvalidDocumentType,
        ),
        (
            "schema futuro",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"schemaVersion\": 1",
                "\"schemaVersion\": 3",
            ),
            DocumentFailure::UnsupportedFutureSchema { version: 3 },
        ),
        (
            "schema legado",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"schemaVersion\": 1",
                "\"schemaVersion\": 0",
            ),
            DocumentFailure::UnsupportedLegacySchema { version: 0 },
        ),
    ];

    for (case, bytes, expected) in cases {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(expected),
            "{case}"
        );
    }
}

#[test]
fn rejects_unknown_and_duplicate_fields_as_a_closed_v1_document() {
    let unknown_field = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "  \"project\": {\n    \"document\": {",
        "  \"project\": {\n    \"unexpected\": true,\n    \"document\": {",
    );
    let duplicate_field = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "  \"revision\": 0,\n",
        "  \"revision\": 0,\n  \"revision\": 0,\n",
    );

    for (case, bytes) in [
        ("campo desconhecido", unknown_field),
        ("campo duplicado", duplicate_field),
    ] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
            "{case}"
        );
    }
}

#[test]
fn rejects_missing_wrong_typed_and_duplicated_fields_at_every_document_layer() {
    let cases = [
        (
            "campo obrigatório ausente",
            replace_literal_once(NEUTRAL_PROJECT_V1, "    \"media\": [],\n", ""),
        ),
        (
            "tipo primitivo incorreto",
            replace_literal_once(NEUTRAL_PROJECT_V1, "\"revision\": 0", "\"revision\": \"0\""),
        ),
        (
            "documentType duplicado",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "  \"documentType\": \"myalbuns.project\",\n",
                "  \"documentType\": \"myalbuns.project\",\n  \"documentType\": \"myalbuns.project\",\n",
            ),
        ),
        (
            "campo aninhado duplicado",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "      \"dpi\": 300,\n",
                "      \"dpi\": 300,\n      \"dpi\": 300,\n",
            ),
        ),
    ];

    for (case, bytes) in cases {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
            "{case}"
        );
    }
}

#[test]
fn rejects_noncanonical_identities_and_fields_from_the_wrong_union_branch() {
    let uppercase_uuid = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "550e8400-e29b-41d4-a716-446655440000",
        "550E8400-E29B-41D4-A716-446655440000",
    );
    let non_v4_uuid = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "550e8400-e29b-41d4-a716-446655440000",
        "550e8400-e29b-11d4-a716-446655440000",
    );
    let opposite_union_field = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"scope\": \"perSide\",\n        \"left\": {",
        "\"scope\": \"perSide\",\n        \"both\": { \"kind\": \"color\", \"rgb\": \"#FFFFFF\" },\n        \"left\": {",
    );
    let unsupported_media_kind = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000010\",\n        \"kind\": \"decorative\"",
        "\"id\": \"00000000-0000-4000-8000-000000000010\",\n        \"kind\": \"photo\"",
    );

    for (case, bytes) in [
        ("UUID com caixa não canônica", uppercase_uuid),
        ("UUID que não é v4", non_v4_uuid),
        ("campo do ramo oposto da união", opposite_union_field),
        ("tipo de mídia não suportado", unsupported_media_kind),
    ] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
            "{case}"
        );
    }
}

#[test]
fn rejects_a_bom_and_invalid_utf8_as_invalid_project_documents() {
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend_from_slice(NEUTRAL_PROJECT_V1.as_bytes());
    let mut invalid_utf8 = NEUTRAL_PROJECT_V1.as_bytes().to_vec();
    invalid_utf8.push(0xFF);

    for (case, bytes) in [("BOM", with_bom), ("UTF-8 inválido", invalid_utf8)] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
            "{case}"
        );
    }
}

#[test]
fn separates_malformed_primitives_from_invalid_project_state() {
    let cases = [
        (
            "revisão acima do inteiro seguro",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"revision\": 0",
                "\"revision\": 9007199254740992",
            ),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
        ),
        (
            "DPI zero",
            replace_literal_once(NEUTRAL_PROJECT_V1, "\"dpi\": 300", "\"dpi\": 0"),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
        ),
        (
            "DPI acima do máximo",
            replace_literal_once(NEUTRAL_PROJECT_V1, "\"dpi\": 300", "\"dpi\": 1201"),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
        ),
        (
            "largura ímpar",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"sheetWidthUm\": 600000",
                "\"sheetWidthUm\": 600001",
            ),
            LoadProjectError::Document(DocumentFailure::InvalidProjectState),
        ),
        (
            "eixo raster acima do limite estrutural",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"sheetWidthUm\": 600000",
                "\"sheetWidthUm\": 6000000",
            ),
            LoadProjectError::Document(DocumentFailure::InvalidProjectState),
        ),
        (
            "recuos eliminam a área segura",
            replace_literal_once(
                NEUTRAL_PROJECT_V1,
                "\"bleedUm\": 3000",
                "\"bleedUm\": 149000",
            ),
            LoadProjectError::Document(DocumentFailure::InvalidProjectState),
        ),
        (
            "cor não canônica",
            replace_literal_once(NEUTRAL_PROJECT_V1, "#FFFFFF", "#ffffff"),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
        ),
    ];

    for (case, bytes, expected) in cases {
        assert_eq!(load_error(&bytes, case), expected, "{case}");
    }
}

#[test]
fn rejects_broken_references_duplicate_identities_and_invalid_sheet_roles() {
    let missing_media = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"left\": null,\n        \"right\": {\n          \"kind\": \"media\",\n          \"mediaId\": \"00000000-0000-4000-8000-000000000011\"",
        "\"left\": null,\n        \"right\": {\n          \"kind\": \"media\",\n          \"mediaId\": \"00000000-0000-4000-8000-000000000099\"",
    );
    let duplicate_media_id = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000011\"",
        "\"id\": \"00000000-0000-4000-8000-000000000010\"",
    );
    let invalid_first_sheet = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000001\",\n        \"activeSides\": \"both\"",
        "\"id\": \"00000000-0000-4000-8000-000000000001\",\n        \"activeSides\": \"left\"",
    );

    for (case, bytes) in [
        ("referência de mídia ausente", missing_media),
        ("ID de mídia duplicado", duplicate_media_id),
        ("primeira Lâmina com lado inválido", invalid_first_sheet),
    ] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectState),
            "{case}"
        );
    }
}

#[test]
fn rejects_duplicate_paths_and_the_remaining_invalid_album_shapes() {
    let duplicate_path = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "[92, 92, 115, 101, 114, 118, 105, 100, 111, 114, 92, 65, 108, 98, 117, 110, 115, 92, 111, 118, 101, 114, 108, 97, 121, 46, 112, 110, 103]",
        "[67, 58, 92, 70, 111, 116, 111, 115, 92, 99, 97, 112, 97, 55296, 46, 112, 110, 103]",
    );
    let one_sheet = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        ",\n      {\n        \"id\": \"00000000-0000-4000-8000-000000000002\",\n        \"activeSides\": \"both\"\n      }",
        "",
    );
    let duplicate_sheet_id = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000002\",\n        \"activeSides\": \"both\"",
        "\"id\": \"00000000-0000-4000-8000-000000000001\",\n        \"activeSides\": \"both\"",
    );
    let invalid_internal_sheet = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000002\",\n        \"activeSides\": \"both\"",
        "\"id\": \"00000000-0000-4000-8000-000000000002\",\n        \"activeSides\": \"left\"",
    );
    let invalid_last_sheet = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"id\": \"00000000-0000-4000-8000-000000000003\",\n        \"activeSides\": \"left\"",
        "\"id\": \"00000000-0000-4000-8000-000000000003\",\n        \"activeSides\": \"right\"",
    );

    for (case, bytes) in [
        ("pathname duplicado", duplicate_path),
        ("menos de duas Lâminas", one_sheet),
        ("ID de Lâmina duplicado", duplicate_sheet_id),
        ("Lâmina interna com lado único", invalid_internal_sheet),
        ("última Lâmina com lado direito", invalid_last_sheet),
    ] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectState),
            "{case}"
        );
    }
}

#[test]
fn accepts_a_valid_solid_frame_border_and_rejects_zero_width() {
    let valid = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "{ \"kind\": \"none\" }",
        "{ \"kind\": \"solid\", \"rgb\": \"#AABBCC\", \"widthUm\": 1000 }",
    );
    let loaded = load_bytes(&valid).expect("a canonical positive solid border is valid");
    match loaded.project().visual_defaults().frame_border() {
        FrameBorder::Solid { rgb, width_um } => {
            assert_eq!(rgb.channels(), [0xAA, 0xBB, 0xCC]);
            assert_eq!(*width_um, 1_000);
        }
        other => panic!("expected a solid Frame border, received {other:?}"),
    }

    let zero_width =
        replace_literal_once(&String::from_utf8(valid).expect("valid UTF-8"), "1000", "0");
    assert_eq!(
        load_error(&zero_width, "borda sólida com largura zero"),
        LoadProjectError::Document(DocumentFailure::InvalidProjectDocument)
    );
}

#[cfg(windows)]
#[test]
fn classifies_invalid_native_path_shape_and_syntax() {
    let unknown_encoding = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"encoding\": \"windowsUtf16\",\n          \"units\": [67",
        "\"encoding\": \"utf8\",\n          \"units\": [67",
    );
    let relative_path = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "[67, 58, 92, 70, 111, 116, 111, 115, 92, 99, 97, 112, 97, 55296, 46, 112, 110, 103]",
        "[114, 101, 108, 97, 116, 105, 118, 111, 46, 112, 110, 103]",
    );
    let extra_path_field = replace_literal_once(
        PER_SIDE_PROJECT_V1,
        "\"encoding\": \"windowsUtf16\",\n          \"units\": [67",
        "\"encoding\": \"windowsUtf16\",\n          \"extra\": true,\n          \"units\": [67",
    );
    let unit_out_of_range = replace_literal_once(PER_SIDE_PROJECT_V1, "55296", "65536");

    assert_eq!(
        load_error(&unknown_encoding, "encoding desconhecido"),
        LoadProjectError::Document(DocumentFailure::InvalidProjectDocument)
    );
    assert_eq!(
        load_error(&relative_path, "pathname relativo"),
        LoadProjectError::Path(myalbuns_core::PathFailure::InvalidPath)
    );
    for (case, bytes) in [
        ("campo extra no pathname", extra_path_field),
        ("unidade UTF-16 fora do intervalo", unit_out_of_range),
    ] {
        assert_eq!(
            load_error(&bytes, case),
            LoadProjectError::Document(DocumentFailure::InvalidProjectDocument),
            "{case}"
        );
    }
}

#[test]
fn read_only_loading_succeeds_without_retaining_an_editable_lock() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("somente-leitura.myalbuns");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("fixture is written");
    let mut permissions = fs::metadata(&project_path)
        .expect("fixture metadata")
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&project_path, permissions).expect("fixture becomes read-only");

    let loaded = ProjectCore::new()
        .load_persisted_revision(load_request(&project_path))
        .expect("read-only loading does not require an editable handle");
    let loaded_again = ProjectCore::new()
        .load_persisted_revision(load_request(&project_path))
        .expect("the immutable value retains no opening lock");
    assert_eq!(loaded.project_id(), loaded_again.project_id());

    let mut permissions = fs::metadata(&project_path)
        .expect("fixture metadata remains available")
        .permissions();
    // This product and fixture are Windows-only; clearing the read-only attribute is
    // required so TempDir can remove the file after the assertion.
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&project_path, permissions).expect("temporary fixture can be cleaned up");
}

#[test]
fn creates_a_neutral_v1_project_and_reopens_it_as_a_clean_editable_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Álbum criado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the neutral Project is published and opened");

    assert_eq!(created.revision(), 0);
    assert_eq!(created.saved_revision(), 0);
    assert!(!created.has_unsaved_changes());
    assert!(!created.can_undo());
    assert!(!created.can_redo());
    assert_eq!(created.project().sheets().len(), 2);
    assert!(created.project().media().is_empty());
    assert_eq!(created.project().document().display_unit(), DisplayUnit::Mm);
    assert_eq!(created.project().document().sheet_width_um(), 600_000);
    assert_eq!(created.project().document().sheet_height_um(), 300_000);
    assert_eq!(created.project().document().dpi(), 300);
    assert_eq!(created.project().document().bleed_um(), 3_000);
    assert_eq!(created.project().document().safety_um(), 3_000);
    match created.project().visual_defaults().background() {
        Background::BothSides {
            both: BackgroundContent::Color { rgb },
        } => assert_eq!(rgb.channels(), [255, 255, 255]),
        other => panic!("expected the neutral white Background, received {other:?}"),
    }
    assert!(matches!(
        created.project().visual_defaults().overlay(),
        Overlay::BothSides { both: None }
    ));
    assert!(matches!(
        created.project().visual_defaults().frame_border(),
        FrameBorder::None
    ));
    let bytes = fs::read(&project_path).expect("the Project was published");
    assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert!(std::str::from_utf8(&bytes).is_ok());

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&project_path)))
            .expect_err("the physical Project remains exclusively editable"),
        OpenProjectError::ProjectInUse
    );
    let created_id = created.project_id();
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("dropping the owner releases both locks");
    assert_eq!(reopened.project_id(), created_id);
    assert_eq!(reopened.revision(), 0);
    assert_eq!(reopened.saved_revision(), 0);
    assert!(!reopened.has_unsaved_changes());
}

#[test]
fn configures_identity_lease_and_registry_roots_explicitly() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory
        .path()
        .join("Projeto com raízes explícitas.myalbuns");
    let lease_root = directory.path().join("ownership").join("leases");
    let registry_root = directory.path().join("authority").join("records");
    let formerly_derived_root = lease_root
        .parent()
        .expect("the fixture lease root has a parent")
        .join("ProjectIdentities");
    let core =
        ProjectCore::new().with_identity_storage_roots(lease_root.clone(), registry_root.clone());

    let project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the Project is created with explicit identity storage roots");
    let identity_record = registry_root.join(format!(
        "{}.json",
        project_data_namespace(&project.project_id().hyphenated().to_string())
    ));

    assert!(lease_root.is_dir(), "the lease root owns the live lease");
    assert!(
        identity_record.is_file(),
        "the explicit registry root owns the durable record"
    );
    assert!(
        !formerly_derived_root.exists(),
        "the lease root must not imply a second persistent storage location"
    );
}

#[test]
fn configured_project_round_trips_physical_settings_and_album_structure() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("configurado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let configuration = InitialProjectConfiguration::new(
        DisplayUnit::Cm,
        508_000,
        254_000,
        240,
        1_270,
        2_540,
        5,
        EndSheetFormat::SinglePage,
        EndSheetFormat::Double,
    );

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::configured(configuration),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the configured Project is created");

    assert_eq!(created.project().document().display_unit(), DisplayUnit::Cm);
    assert_eq!(created.project().document().sheet_width_um(), 508_000);
    assert_eq!(created.project().document().sheet_height_um(), 254_000);
    assert_eq!(created.project().document().dpi(), 240);
    assert_eq!(created.project().document().bleed_um(), 1_270);
    assert_eq!(created.project().document().safety_um(), 2_540);
    assert_eq!(created.project().sheets().len(), 5);
    assert_eq!(
        created.project().sheets()[0].active_sides(),
        ActiveSides::Right
    );
    assert!(
        created.project().sheets()[1..4]
            .iter()
            .all(|sheet| sheet.active_sides() == ActiveSides::Both)
    );
    assert_eq!(
        created.project().sheets()[4].active_sides(),
        ActiveSides::Both
    );
    let projection = created.projection();
    assert_eq!(
        projection.state.document.display_unit,
        ProjectedDisplayUnit::Cm
    );
    assert_eq!(projection.state.document.sheet_width_um, 508_000);
    assert_eq!(projection.state.document.sheet_height_um, 254_000);
    assert_eq!(projection.state.document.dpi, 240);
    assert_eq!(projection.state.document.bleed_um, 1_270);
    assert_eq!(projection.state.document.safety_um, 2_540);
    assert_eq!(
        projection.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );
    assert_eq!(
        projection.composition.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );
    assert_eq!(projection.composition.sheets[0].width_um, 254_000);
    let projected_json = serde_json::to_value(&projection).expect("projection serializes");
    assert_eq!(projected_json["state"]["document"]["displayUnit"], "cm");
    assert_eq!(
        projected_json["state"]["album"]["sheets"][0]["activeSides"],
        "right"
    );
    assert_eq!(
        projected_json["state"]["album"]["sheets"]
            .as_array()
            .expect("the projected Album has an ordered sheet list")
            .iter()
            .map(|sheet| sheet["pageNumbers"].clone())
            .collect::<Vec<_>>(),
        vec![
            serde_json::json!([1]),
            serde_json::json!([2, 3]),
            serde_json::json!([4, 5]),
            serde_json::json!([6, 7]),
            serde_json::json!([8, 9]),
        ]
    );
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the configured Project reopens");
    assert_eq!(
        reopened.project().document().display_unit(),
        DisplayUnit::Cm
    );
    assert_eq!(reopened.project().document().sheet_width_um(), 508_000);
    assert_eq!(reopened.project().document().sheet_height_um(), 254_000);
    assert_eq!(reopened.project().document().dpi(), 240);
    assert_eq!(reopened.project().document().bleed_um(), 1_270);
    assert_eq!(reopened.project().document().safety_um(), 2_540);
    assert_eq!(
        reopened
            .project()
            .sheets()
            .iter()
            .map(|sheet| sheet.active_sides())
            .collect::<Vec<_>>(),
        vec![
            ActiveSides::Right,
            ActiveSides::Both,
            ActiveSides::Both,
            ActiveSides::Both,
            ActiveSides::Both,
        ]
    );
}

#[cfg(windows)]
#[test]
fn creates_and_reopens_a_project_with_both_sides_visual_defaults() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("personalizado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let background_path = std::path::PathBuf::from(r"C:\Decorativos\background.png");
    let overlay_path = std::path::PathBuf::from(r"C:\Decorativos\overlay.png");
    let personalization = InitialProjectPersonalization::new(
        InitialBackground::BothSides {
            both: InitialBackgroundContent::Media {
                path: background_path.clone(),
            },
        },
        InitialOverlay::BothSides {
            both: Some(InitialOverlayContent::Media {
                path: overlay_path.clone(),
            }),
        },
        InitialFrameBorder::Solid {
            rgb: Rgb::parse_canonical("#AABBCC").expect("canonical color"),
            width_um: 1_000,
        },
    );

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral().with_personalization(personalization),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the personalized Project is created");

    assert_eq!(
        created
            .project()
            .media()
            .iter()
            .map(|media| media.path())
            .collect::<Vec<_>>(),
        vec![background_path.as_path(), overlay_path.as_path()]
    );
    let background_id = created.project().media()[0].id();
    let overlay_id = created.project().media()[1].id();
    assert!(matches!(
        created.project().visual_defaults().background(),
        Background::BothSides {
            both: BackgroundContent::Media { media_id }
        } if *media_id == background_id
    ));
    assert!(matches!(
        created.project().visual_defaults().overlay(),
        Overlay::BothSides {
            both: Some(OverlayContent::Media { media_id })
        } if *media_id == overlay_id
    ));
    assert!(matches!(
        created.project().visual_defaults().frame_border(),
        FrameBorder::Solid { rgb, width_um }
            if rgb.channels() == [0xAA, 0xBB, 0xCC] && *width_um == 1_000
    ));
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the personalized Project reopens");
    assert_eq!(reopened.project().media()[0].path(), background_path);
    assert_eq!(reopened.project().media()[1].path(), overlay_path);
    assert!(matches!(
        reopened.project().visual_defaults().background(),
        Background::BothSides {
            both: BackgroundContent::Media { media_id }
        } if *media_id == background_id
    ));
    assert!(matches!(
        reopened.project().visual_defaults().overlay(),
        Overlay::BothSides {
            both: Some(OverlayContent::Media { media_id })
        } if *media_id == overlay_id
    ));
}

#[cfg(windows)]
#[test]
fn per_side_personalization_preserves_reference_order_and_deduplicates_native_paths() {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
        path::PathBuf,
    };

    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("por-lado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let background_path = PathBuf::from(OsString::from_wide(&[
        67, 58, 92, 68, 101, 99, 111, 114, 97, 116, 105, 118, 111, 115, 92, 99, 97, 112, 97,
        0xD800, 46, 112, 110, 103,
    ]));
    let overlay_path = PathBuf::from(r"\\servidor\Albuns\overlay.png");
    let personalization = InitialProjectPersonalization::new(
        InitialBackground::PerSide {
            left: InitialBackgroundContent::Media {
                path: background_path.clone(),
            },
            right: InitialBackgroundContent::Color {
                rgb: Rgb::parse_canonical("#102030").expect("canonical color"),
            },
        },
        InitialOverlay::PerSide {
            left: Some(InitialOverlayContent::Media {
                path: overlay_path.clone(),
            }),
            right: Some(InitialOverlayContent::Media {
                path: background_path.clone(),
            }),
        },
        InitialFrameBorder::None,
    );

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral().with_personalization(personalization),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the per-side Project is created");

    assert_eq!(created.project().media().len(), 2);
    assert_eq!(created.project().media()[0].path(), background_path);
    assert_eq!(created.project().media()[1].path(), overlay_path);
    let background_id = created.project().media()[0].id();
    let overlay_id = created.project().media()[1].id();
    assert_ne!(background_id, overlay_id);
    match created.project().visual_defaults().background() {
        Background::PerSide { left, right } => {
            assert!(matches!(
                left,
                BackgroundContent::Media { media_id } if *media_id == background_id
            ));
            assert!(matches!(
                right,
                BackgroundContent::Color { rgb } if rgb.channels() == [0x10, 0x20, 0x30]
            ));
        }
        other => panic!("expected a per-side Background, received {other:?}"),
    }
    match created.project().visual_defaults().overlay() {
        Overlay::PerSide { left, right } => {
            assert!(matches!(
                left,
                Some(OverlayContent::Media { media_id }) if *media_id == overlay_id
            ));
            assert!(matches!(
                right,
                Some(OverlayContent::Media { media_id }) if *media_id == background_id
            ));
        }
        other => panic!("expected a per-side Overlay, received {other:?}"),
    }
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the per-side Project reopens");
    assert_eq!(reopened.project().media().len(), 2);
    assert_eq!(
        reopened.project().media()[0]
            .path()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        background_path
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>()
    );
    assert_eq!(reopened.project().media()[1].path(), overlay_path);
}

#[test]
fn invalid_initial_personalization_has_no_file_or_identity_lease_effects() {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

    let directory = tempfile::tempdir().expect("temporary directory");
    let valid_color = Rgb::parse_canonical("#AABBCC").expect("canonical color");
    let cases = [
        (
            "relative-media-path",
            InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: std::path::PathBuf::from("relative.png"),
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            ),
        ),
        (
            "zero-border-width",
            InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Color { rgb: valid_color },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::Solid {
                    rgb: valid_color,
                    width_um: 0,
                },
            ),
        ),
        (
            "negative-border-width",
            InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Color { rgb: valid_color },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::Solid {
                    rgb: valid_color,
                    width_um: -1,
                },
            ),
        ),
        (
            "border-width-above-safe-integer",
            InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Color { rgb: valid_color },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::Solid {
                    rgb: valid_color,
                    width_um: MAX_SAFE_INTEGER + 1,
                },
            ),
        ),
    ];

    for (case, personalization) in cases {
        let project_path = directory.path().join(format!("{case}.myalbuns"));
        let storage_root = directory.path().join(format!("identity-{case}"));
        let lease_root = storage_root.join("leases");
        let error = project_core_with_identity_storage(&storage_root)
            .create_editable(CreateProjectRequest::new(
                project_location(&project_path),
                InitialProject::neutral().with_personalization(personalization),
                CreateAuthorization::CreateOnly,
            ))
            .expect_err(case);

        assert_eq!(error, CreateProjectError::InvalidInitialProject, "{case}");
        assert!(!project_path.exists(), "{case}: no Project is published");
        assert!(
            !lease_root.exists(),
            "{case}: no identity lease root is created"
        );
    }
}

#[test]
fn rgb_accepts_only_the_canonical_persisted_color_form() {
    assert_eq!(
        Rgb::parse_canonical("#A1B2C3").map(Rgb::channels),
        Some([0xA1, 0xB2, 0xC3])
    );
    for invalid in ["A1B2C3", "#a1b2c3", "#A1B2C", "#GG0000", "#A1B2C3FF"] {
        assert!(Rgb::parse_canonical(invalid).is_none(), "{invalid}");
    }
}

#[test]
fn configured_project_maps_every_end_sheet_combination_and_keeps_internal_sheets_double() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let cases = [
        (
            EndSheetFormat::Double,
            EndSheetFormat::Double,
            ActiveSides::Both,
            ActiveSides::Both,
        ),
        (
            EndSheetFormat::Double,
            EndSheetFormat::SinglePage,
            ActiveSides::Both,
            ActiveSides::Left,
        ),
        (
            EndSheetFormat::SinglePage,
            EndSheetFormat::Double,
            ActiveSides::Right,
            ActiveSides::Both,
        ),
        (
            EndSheetFormat::SinglePage,
            EndSheetFormat::SinglePage,
            ActiveSides::Right,
            ActiveSides::Left,
        ),
    ];

    for (index, (first_format, last_format, first_sides, last_sides)) in
        cases.into_iter().enumerate()
    {
        let project_path = directory
            .path()
            .join(format!("extremidades-{index}.myalbuns"));
        let project =
            project_core_with_identity_storage(&directory.path().join(format!("identity-{index}")))
                .create_editable(CreateProjectRequest::new(
                    project_location(&project_path),
                    InitialProject::configured(InitialProjectConfiguration::new(
                        DisplayUnit::Mm,
                        600_000,
                        300_000,
                        300,
                        3_000,
                        3_000,
                        4,
                        first_format,
                        last_format,
                    )),
                    CreateAuthorization::CreateOnly,
                ))
                .expect("the end-sheet combination is valid");

        assert_eq!(
            project
                .project()
                .sheets()
                .iter()
                .map(|sheet| sheet.active_sides())
                .collect::<Vec<_>>(),
            vec![
                first_sides,
                ActiveSides::Both,
                ActiveSides::Both,
                last_sides
            ]
        );
        let unique_ids = project
            .project()
            .sheets()
            .iter()
            .map(|sheet| sheet.id())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            unique_ids.len(),
            4,
            "the core owns sheet identity generation"
        );
    }
}

#[test]
fn display_unit_does_not_change_authoritative_physical_values() {
    let directory = tempfile::tempdir().expect("temporary directory");

    for (index, display_unit) in [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::In]
        .into_iter()
        .enumerate()
    {
        let project_path = directory.path().join(format!("unidade-{index}.myalbuns"));
        let project =
            project_core_with_identity_storage(&directory.path().join(format!("identity-{index}")))
                .create_editable(CreateProjectRequest::new(
                    project_location(&project_path),
                    InitialProject::configured(InitialProjectConfiguration::new(
                        display_unit,
                        508_000,
                        254_000,
                        300,
                        1_270,
                        2_540,
                        2,
                        EndSheetFormat::Double,
                        EndSheetFormat::Double,
                    )),
                    CreateAuthorization::CreateOnly,
                ))
                .expect("displayUnit does not reinterpret micrometers");

        assert_eq!(project.project().document().display_unit(), display_unit);
        assert_eq!(project.project().document().sheet_width_um(), 508_000);
        assert_eq!(project.project().document().sheet_height_um(), 254_000);
        assert_eq!(project.project().document().bleed_um(), 1_270);
        assert_eq!(project.project().document().safety_um(), 2_540);
    }
}

#[test]
fn bleed_and_safety_accept_zero_independently() {
    let directory = tempfile::tempdir().expect("temporary directory");

    for (index, (bleed_um, safety_um)) in [(0, 3_000), (3_000, 0)].into_iter().enumerate() {
        let project_path = directory.path().join(format!("margens-{index}.myalbuns"));
        let project =
            project_core_with_identity_storage(&directory.path().join(format!("identity-{index}")))
                .create_editable(CreateProjectRequest::new(
                    project_location(&project_path),
                    InitialProject::configured(InitialProjectConfiguration::new(
                        DisplayUnit::Mm,
                        600_000,
                        300_000,
                        300,
                        bleed_um,
                        safety_um,
                        2,
                        EndSheetFormat::Double,
                        EndSheetFormat::Double,
                    )),
                    CreateAuthorization::CreateOnly,
                ))
                .expect("zero is valid for either technical area");

        assert_eq!(
            project.project().document().bleed_um(),
            u64::try_from(bleed_um).expect("valid bleed")
        );
        assert_eq!(
            project.project().document().safety_um(),
            u64::try_from(safety_um).expect("valid safety")
        );
    }
}

#[test]
fn invalid_initial_configuration_has_no_file_or_identity_lease_effects() {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

    let directory = tempfile::tempdir().expect("temporary directory");
    let cases = [
        (
            "negative-width",
            -1,
            300_000,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetWidthNotPositive,
        ),
        (
            "zero-width",
            0,
            300_000,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetWidthNotPositive,
        ),
        (
            "width-above-safe-integer",
            MAX_SAFE_INTEGER + 1,
            300_000,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetWidthAboveSafeInteger,
        ),
        (
            "odd-width",
            600_001,
            300_000,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetWidthNotEven,
        ),
        (
            "sheet-raster-too-wide",
            1_400_000,
            300_000,
            1_200,
            3_000,
            3_000,
            2,
            ValidationError::SheetWidthRasterOutOfRange,
        ),
        (
            "page-raster-is-zero",
            12_700,
            25_400,
            1,
            0,
            0,
            2,
            ValidationError::SheetWidthRasterOutOfRange,
        ),
        (
            "negative-height",
            600_000,
            -1,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetHeightNotPositive,
        ),
        (
            "zero-height",
            600_000,
            0,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetHeightNotPositive,
        ),
        (
            "height-above-safe-integer",
            600_000,
            MAX_SAFE_INTEGER + 1,
            300,
            3_000,
            3_000,
            2,
            ValidationError::SheetHeightAboveSafeInteger,
        ),
        (
            "sheet-raster-too-tall",
            600_000,
            1_400_000,
            1_200,
            3_000,
            3_000,
            2,
            ValidationError::SheetHeightRasterOutOfRange,
        ),
        (
            "zero-dpi",
            600_000,
            300_000,
            0,
            3_000,
            3_000,
            2,
            ValidationError::DpiOutOfRange,
        ),
        (
            "dpi-above-v1",
            600_000,
            300_000,
            1_201,
            3_000,
            3_000,
            2,
            ValidationError::DpiOutOfRange,
        ),
        (
            "one-sheet",
            600_000,
            300_000,
            300,
            3_000,
            3_000,
            1,
            ValidationError::SheetCountTooSmall,
        ),
        (
            "negative-bleed",
            600_000,
            300_000,
            300,
            -1,
            3_000,
            2,
            ValidationError::BleedNegative,
        ),
        (
            "bleed-above-safe-integer",
            600_000,
            300_000,
            300,
            MAX_SAFE_INTEGER + 1,
            3_000,
            2,
            ValidationError::BleedAboveSafeInteger,
        ),
        (
            "bleed-eliminates-page",
            600_000,
            300_000,
            300,
            300_000,
            0,
            2,
            ValidationError::BleedEliminatesCutArea,
        ),
        (
            "bleed-eliminates-height",
            600_000,
            300_000,
            300,
            150_000,
            0,
            2,
            ValidationError::BleedEliminatesCutArea,
        ),
        (
            "negative-safety",
            600_000,
            300_000,
            300,
            3_000,
            -1,
            2,
            ValidationError::SafetyNegative,
        ),
        (
            "safety-above-safe-integer",
            600_000,
            300_000,
            300,
            3_000,
            MAX_SAFE_INTEGER + 1,
            2,
            ValidationError::SafetyAboveSafeInteger,
        ),
        (
            "safety-eliminates-page",
            600_000,
            300_000,
            300,
            3_000,
            297_000,
            2,
            ValidationError::SafetyEliminatesSafeArea,
        ),
        (
            "safety-eliminates-height",
            600_000,
            300_000,
            300,
            3_000,
            147_000,
            2,
            ValidationError::SafetyEliminatesSafeArea,
        ),
    ];

    for (
        index,
        (case, width_um, height_um, dpi, bleed_um, safety_um, sheet_count, expected_error),
    ) in cases.into_iter().enumerate()
    {
        let project_path = directory.path().join(format!("invalid-{index}.myalbuns"));
        let lease_root = directory.path().join(format!("leases-{index}"));
        let configuration = InitialProjectConfiguration::new(
            DisplayUnit::Mm,
            width_um,
            height_um,
            dpi,
            bleed_um,
            safety_um,
            sheet_count,
            EndSheetFormat::Double,
            EndSheetFormat::Double,
        );
        assert_eq!(
            configuration.validation_errors(),
            vec![expected_error],
            "{case}"
        );
        let error = ProjectCore::new()
            .with_identity_storage_roots(
                lease_root.clone(),
                directory.path().join(format!("identities-{case}")),
            )
            .create_editable(CreateProjectRequest::new(
                project_location(&project_path),
                InitialProject::configured(configuration),
                CreateAuthorization::CreateOnly,
            ))
            .expect_err(case);

        assert_eq!(error, CreateProjectError::InvalidInitialProject, "{case}");
        assert!(!project_path.exists(), "{case}: no Project is published");
        assert!(
            !lease_root.exists(),
            "{case}: no identity lease root is created"
        );
    }
}

#[test]
fn initial_configuration_reports_independent_field_errors_in_form_order() {
    let configuration = InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        -2,
        -1,
        0,
        -3,
        -4,
        1,
        EndSheetFormat::Double,
        EndSheetFormat::Double,
    );

    assert_eq!(
        configuration.validation_errors(),
        vec![
            ValidationError::SheetWidthNotPositive,
            ValidationError::SheetHeightNotPositive,
            ValidationError::DpiOutOfRange,
            ValidationError::SheetCountTooSmall,
            ValidationError::BleedNegative,
            ValidationError::SafetyNegative,
        ]
    );
}

#[test]
fn an_unreservable_sheet_count_is_rejected_without_inventing_a_functional_maximum() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("quantidade-irreservavel.myalbuns");
    let lease_root = directory.path().join("leases");
    let configuration = InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        600_000,
        300_000,
        300,
        3_000,
        3_000,
        i64::MAX,
        EndSheetFormat::Double,
        EndSheetFormat::Double,
    );
    assert!(configuration.validation_errors().is_empty());

    assert_eq!(
        ProjectCore::new()
            .with_identity_storage_roots(lease_root.clone(), directory.path().join("identities"),)
            .create_editable(CreateProjectRequest::new(
                project_location(&project_path),
                InitialProject::configured(configuration),
                CreateAuthorization::CreateOnly,
            ))
            .expect_err("the fallible reservation rejects an impossible capacity"),
        CreateProjectError::InvalidInitialProject
    );
    assert!(!project_path.exists());
    assert!(!lease_root.exists());
}

#[test]
fn horizontal_technical_areas_may_reach_one_micrometer_before_the_page_center() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("limiar-horizontal.myalbuns");
    let project = project_core_with_identity_storage(directory.path())
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600,
                600,
                1_200,
                100,
                199,
                2,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .expect("only the outer page edge receives technical areas");

    assert_eq!(project.project().document().sheet_width_um() / 2, 300);
    assert_eq!(project.project().document().bleed_um(), 100);
    assert_eq!(project.project().document().safety_um(), 199);
}

#[test]
fn structurally_valid_configuration_is_not_rejected_by_the_export_memory_guardrail() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("alta-resolucao.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Cm,
                600_000,
                300_000,
                1_200,
                3_000,
                3_000,
                2,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the document does not inherit a transient export memory limit");
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the structurally valid high-resolution document reopens");
    assert_eq!(reopened.project().document().dpi(), 1_200);
    assert_eq!(reopened.project().document().sheet_width_um(), 600_000);
    assert_eq!(reopened.project().document().sheet_height_um(), 300_000);
}

#[test]
fn an_open_v1_project_exposes_the_initial_editor_projection_without_demo_content() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Projeto produtivo.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");

    let projection = project.projection();

    assert_eq!(
        projection.state.project_id,
        project.project_id().to_string()
    );
    assert_eq!(projection.state.project_name, "Projeto produtivo");
    assert_eq!(projection.state.revision, 0);
    assert_eq!(projection.state.saved_revision, 0);
    assert!(!projection.state.dirty);
    assert!(!projection.state.can_undo);
    assert!(!projection.state.can_redo);
    assert!(projection.state.album.media.is_empty());
    assert!(projection.media_usage.is_empty());
    assert_eq!(projection.state.album.sheets.len(), 2);
    assert_eq!(projection.state.album.sheets[0].role, SheetRole::Initial);
    assert_eq!(projection.state.album.sheets[1].role, SheetRole::Final);
    assert!(
        projection
            .state
            .album
            .sheets
            .iter()
            .all(|sheet| sheet.frames.is_empty())
    );
    assert_eq!(projection.composition.sheets.len(), 2);
    assert!(project.render_snapshot().validate().is_ok());
}

#[test]
fn changing_dpi_updates_the_authoritative_projection_without_writing_the_project_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("DPI editável.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    let initial_projection = project.projection();
    let persisted_before = fs::read(&project_path).expect("the initial revision is persisted");

    let projection = project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("a valid DPI change is applied");

    assert_eq!(projection.state.document.dpi, 240);
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 0);
    assert!(projection.state.dirty);
    assert!(projection.state.can_undo);
    assert!(!projection.state.can_redo);
    assert_eq!(
        projection.state.document.display_unit,
        initial_projection.state.document.display_unit
    );
    assert_eq!(
        projection.state.document.sheet_width_um,
        initial_projection.state.document.sheet_width_um
    );
    assert_eq!(
        projection.state.document.sheet_height_um,
        initial_projection.state.document.sheet_height_um
    );
    assert_eq!(
        projection.state.document.bleed_um,
        initial_projection.state.document.bleed_um
    );
    assert_eq!(
        projection.state.document.safety_um,
        initial_projection.state.document.safety_um
    );
    assert_eq!(projection.state.album, initial_projection.state.album);
    assert_eq!(projection.composition, initial_projection.composition);
    assert_eq!(project.project().document().dpi(), 240);
    assert_eq!(
        fs::read(&project_path).expect("the persisted revision remains readable"),
        persisted_before,
        "a creative action must not write before Save"
    );
}

#[test]
fn changing_album_information_is_one_atomic_authoritative_revision() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Informações editáveis.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");

    let information = AlbumInformation {
        display_unit: DisplayUnit::Cm,
        sheet_width_um: 700_000,
        sheet_height_um: 350_000,
        dpi: 240,
        bleed_um: 4_000,
        safety_um: 6_000,
        first_sheet: EndSheetFormat::SinglePage,
        last_sheet: EndSheetFormat::SinglePage,
    };
    assert_eq!(
        project.validate_album_information(&information),
        myalbuns_core::AlbumInformationValidation {
            errors: vec![],
            impact: Some(AlbumInformationImpact {
                sheet_width_px: 6_614,
                page_width_px: 3_307,
                height_px: 3_307,
            }),
        }
    );

    let changed = project
        .apply(ProjectIntent::SetAlbumInformation { information })
        .expect("all Album information changes are valid");
    assert_eq!(changed.state.revision, 1);
    assert_eq!(
        changed.state.document.display_unit,
        ProjectedDisplayUnit::Cm
    );
    assert_eq!(changed.state.document.sheet_width_um, 700_000);
    assert_eq!(changed.state.document.sheet_height_um, 350_000);
    assert_eq!(changed.state.document.dpi, 240);
    assert_eq!(changed.state.document.bleed_um, 4_000);
    assert_eq!(changed.state.document.safety_um, 6_000);
    assert_eq!(
        changed.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );
    assert_eq!(
        changed.state.album.sheets[1].active_sides,
        ProjectedActiveSides::Left
    );

    let undone = project
        .undo()
        .expect("the complete form is one Undo action");
    assert_eq!(undone.state.revision, 0);
    assert_eq!(undone.state.document.sheet_width_um, 600_000);
    assert_eq!(undone.state.document.dpi, 300);
    assert_eq!(
        undone.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Both
    );
    assert_eq!(
        undone.state.album.sheets[1].active_sides,
        ProjectedActiveSides::Both
    );

    let redone = project
        .redo()
        .expect("the complete form is one Redo action");
    assert_eq!(redone.state.revision, 1);
    assert_eq!(redone.state.document.sheet_width_um, 700_000);
    assert_eq!(
        redone.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );

    assert_eq!(
        project
            .save(1)
            .expect("the complete Album information revision is persisted"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    drop(project);
    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the saved Album information reopens")
        .projection();
    assert_eq!(
        reopened.state.document.display_unit,
        ProjectedDisplayUnit::Cm
    );
    assert_eq!(reopened.state.document.sheet_width_um, 700_000);
    assert_eq!(reopened.state.document.sheet_height_um, 350_000);
    assert_eq!(reopened.state.document.dpi, 240);
    assert_eq!(reopened.state.document.bleed_um, 4_000);
    assert_eq!(reopened.state.document.safety_um, 6_000);
    assert_eq!(
        reopened.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );
    assert_eq!(
        reopened.state.album.sheets[1].active_sides,
        ProjectedActiveSides::Left
    );
}

#[test]
fn invalid_album_information_does_not_consume_history() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Informações inválidas.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    let initial = project.projection();
    let information = AlbumInformation {
        display_unit: DisplayUnit::Mm,
        sheet_width_um: 600_000,
        sheet_height_um: 300_000,
        dpi: 300,
        bleed_um: 160_000,
        safety_um: 3_000,
        first_sheet: EndSheetFormat::Double,
        last_sheet: EndSheetFormat::Double,
    };

    assert_eq!(
        project.validate_album_information(&information).errors,
        [ValidationError::BleedEliminatesCutArea]
    );
    assert_eq!(
        project
            .apply(ProjectIntent::SetAlbumInformation { information })
            .expect_err("invalid Album information is rejected"),
        CoreError::InvalidAlbumInformation(vec![ValidationError::BleedEliminatesCutArea,])
    );
    assert_eq!(project.projection(), initial);
}

#[test]
fn dimensional_change_requires_the_current_sheet_proportion() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Proporção protegida.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    let initial = project.projection();
    let information = AlbumInformation {
        display_unit: DisplayUnit::Mm,
        sheet_width_um: 700_000,
        sheet_height_um: 300_000,
        dpi: 300,
        bleed_um: 3_000,
        safety_um: 3_000,
        first_sheet: EndSheetFormat::Double,
        last_sheet: EndSheetFormat::Double,
    };

    assert_eq!(
        project.validate_album_information(&information).errors,
        [ValidationError::SheetDimensionsNotProportional]
    );
    assert_eq!(
        project
            .apply(ProjectIntent::SetAlbumInformation { information })
            .expect_err("an incompatible proportion is rejected"),
        CoreError::InvalidAlbumInformation(vec![ValidationError::SheetDimensionsNotProportional,])
    );
    assert_eq!(project.projection(), initial);
}

#[test]
fn saving_the_current_revision_returns_already_current_without_touching_the_project_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Projeto atual.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    fs::remove_file(&project_path)
        .expect("the fixture removes the pathname while the authoritative handle remains open");

    assert_eq!(
        project
            .save(0)
            .expect("an already-current Save performs no file I/O"),
        SaveProjectOutcome::AlreadyCurrent { revision: 0 }
    );
    assert_eq!(project.saved_revision(), 0);
    assert!(!project.has_unsaved_changes());
}

#[test]
fn a_stale_save_request_is_rejected_before_file_io_and_preserves_the_session() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Pedido obsoleto.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    fs::remove_file(&project_path)
        .expect("the fixture removes the pathname while the authoritative handle remains open");

    assert_eq!(
        project
            .save(0)
            .expect_err("the stale request is rejected before consulting the missing pathname"),
        SaveProjectError::StaleRevision {
            expected: 0,
            current: 1,
        }
    );
    let projection = project.projection();
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 0);
    assert!(projection.state.dirty);
    assert!(projection.state.can_undo);
}

#[test]
fn saving_the_visible_revision_publishes_it_and_preserves_session_history() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("RevisÃ£o confirmada.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");

    assert_eq!(
        project
            .save(1)
            .expect("the visible revision is published and verified"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    let saved = project.projection();
    assert_eq!(saved.state.revision, 1);
    assert_eq!(saved.state.saved_revision, 1);
    assert!(!saved.state.dirty);
    assert!(saved.state.can_undo);
    assert!(!saved.state.can_redo);

    let persisted = core
        .load_persisted_revision(load_request(&project_path))
        .expect("the active identity lease follows the newly published physical object");
    assert_eq!(persisted.revision(), 1);
    assert_eq!(persisted.project().document().dpi(), 240);

    let undone = project
        .undo()
        .expect("Save does not erase the creative history");
    assert_eq!(undone.state.revision, 0);
    assert_eq!(undone.state.saved_revision, 1);
    assert_eq!(undone.state.document.dpi, 300);
    assert!(undone.state.dirty);
    assert!(undone.state.can_redo);
}

#[cfg(windows)]
#[test]
fn saving_reuses_the_captured_mapped_drive_binding_for_its_temporary_sibling() {
    let directory = tempfile::tempdir().expect("temporary operational root");
    let logical_path = std::path::PathBuf::from(r"R:\Albuns\Projeto mapeado.myalbuns");
    fs::create_dir(directory.path().join("Albuns")).expect("the operational parent exists");
    let mut context = OperationPathContext::new();
    context
        .capture_with_binding(&logical_path, directory.path())
        .expect("the mapped drive binding is captured for the attempt");
    let location = ProjectLocation::new(logical_path, context.freeze());
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location.clone(),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the Project is created through the captured binding");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");

    assert_eq!(
        project
            .save(1)
            .expect("the operational temporary remains inside the captured binding"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    let persisted = core
        .load_persisted_revision(LoadProjectRequest::new(location))
        .expect("the saved revision loads through the same captured binding");
    assert_eq!(persisted.revision(), 1);
    assert_eq!(persisted.project().document().dpi(), 240);
}

#[test]
fn reopening_a_saved_revision_starts_a_clean_session_with_empty_history() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Reabertura confirmada.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    project
        .save(1)
        .expect("the visible revision is published and verified");
    drop(project);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the saved Project reopens in a new Session");
    let projection = reopened.projection();
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 1);
    assert_eq!(projection.state.document.dpi, 240);
    assert!(!projection.state.dirty);
    assert!(!projection.state.can_undo);
    assert!(!projection.state.can_redo);
}

#[test]
fn opening_an_editable_project_respects_its_stable_transition_barrier() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Barreira de abertura.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project is created");
    let project_id = project.project_id().hyphenated().to_string();
    drop(project);
    let _barrier = ProjectTransitionBarrier::try_acquire(&project_path, &project_id)
        .expect("the competing transition owns the stable sibling barrier");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&project_path)))
            .expect_err("an opener cannot cross an active publication handoff"),
        OpenProjectError::ProjectInUse
    );
}

#[test]
fn a_conclusive_save_failure_preserves_the_editable_session_for_retry() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Falha conclusiva.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    let barrier = ProjectTransitionBarrier::try_acquire(
        &project_path,
        &project.project_id().hyphenated().to_string(),
    )
    .expect("a competing transition acquires the stable sibling barrier");

    assert_eq!(
        project
            .save(1)
            .expect_err("the occupied barrier fails before publication"),
        SaveProjectError::Path(PathFailure::Conflict)
    );
    let unchanged = project.projection();
    assert_eq!(unchanged.state.revision, 1);
    assert_eq!(unchanged.state.saved_revision, 0);
    assert_eq!(unchanged.state.document.dpi, 240);
    assert!(unchanged.state.dirty);
    assert!(unchanged.state.can_undo);

    drop(barrier);
    assert_eq!(
        project
            .save(1)
            .expect("the preserved Session can retry after the conclusive failure"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
}

#[cfg(windows)]
#[test]
fn a_replace_failure_reacquires_the_exact_baseline_and_allows_retry() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Replace bloqueado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    let external_handle = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&project_path)
        .expect("the external reader intentionally denies delete sharing");

    assert!(matches!(project.save(1), Err(SaveProjectError::Path(_))));
    let unchanged = project.projection();
    assert_eq!(unchanged.state.revision, 1);
    assert_eq!(unchanged.state.saved_revision, 0);
    assert_eq!(unchanged.state.document.dpi, 240);
    assert!(unchanged.state.dirty);
    assert!(unchanged.state.can_undo);

    drop(external_handle);
    assert_eq!(
        project
            .save(1)
            .expect("the reacquired exact baseline remains retryable"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
}

#[test]
fn save_rejects_persisted_bytes_that_diverge_even_with_the_same_revision() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Baseline divergente.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    let mut external_bytes = fs::read(&project_path).expect("the baseline is readable");
    external_bytes.extend_from_slice(b" ");
    fs::write(&project_path, &external_bytes)
        .expect("an external writer changes bytes without changing the declared revision");

    assert_eq!(
        project
            .save(1)
            .expect_err("the numeric revision does not replace exact baseline evidence"),
        SaveProjectError::PersistedBaselineConflict
    );
    assert_eq!(
        fs::read(&project_path).expect("the divergent destination remains untouched"),
        external_bytes
    );
    let projection = project.projection();
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 0);
    assert_eq!(projection.state.document.dpi, 240);
    assert!(projection.state.dirty);
    assert!(projection.state.can_undo);
}

#[test]
fn save_rejects_a_different_physical_object_even_when_its_bytes_are_identical() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Objeto fisico divergente.myalbuns");
    let displaced_path = directory.path().join("baseline-deslocado.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible revision advances");
    let baseline_bytes = fs::read(&project_path).expect("the baseline is readable");
    fs::rename(&project_path, &displaced_path)
        .expect("the external actor moves the physical baseline object");
    fs::write(&project_path, &baseline_bytes)
        .expect("the external actor creates a byte-identical object at the pathname");

    assert_eq!(
        project
            .save(1)
            .expect_err("byte equality cannot replace physical baseline identity"),
        SaveProjectError::PersistedBaselineConflict
    );
    assert_eq!(
        fs::read(&project_path).expect("the byte-identical external object remains untouched"),
        baseline_bytes
    );
    let projection = project.projection();
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 0);
    assert!(projection.state.dirty);
    assert!(projection.state.can_undo);
}

#[test]
fn invalid_dpi_values_leave_the_session_history_and_project_file_unchanged() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let core = project_core_with_identity_storage(directory.path());
    let neutral_path = directory.path().join("DPI inválido.myalbuns");
    let mut neutral = core
        .create_editable(CreateProjectRequest::new(
            project_location(&neutral_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the neutral Project opens");
    let neutral_bytes = fs::read(&neutral_path).expect("the neutral revision is persisted");
    neutral
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the fixture creates one history entry");
    let neutral_projection = neutral
        .undo()
        .expect("the fixture exposes a Redo branch before invalid input");
    assert!(neutral_projection.state.can_redo);

    for dpi in [0, 1_201] {
        assert_eq!(
            neutral
                .apply(ProjectIntent::SetDpi { dpi })
                .expect_err("an out-of-range DPI is rejected"),
            CoreError::InvalidDpi(dpi)
        );
        assert_eq!(neutral.projection(), neutral_projection);
        assert_eq!(
            fs::read(&neutral_path).expect("the neutral revision remains readable"),
            neutral_bytes
        );
    }
    let redone = neutral
        .redo()
        .expect("invalid input must not discard the Redo branch");
    assert_eq!(redone.state.document.dpi, 240);
    assert_eq!(redone.state.revision, 1);
    neutral
        .undo()
        .expect("the fixture returns to the saved revision");
    let branched = neutral
        .apply(ProjectIntent::SetDpi { dpi: 600 })
        .expect("invalid input must not consume revision identifiers");
    assert_eq!(branched.state.revision, 2);

    let wide_path = directory.path().join("DPI raster inválido.myalbuns");
    let mut wide = core
        .create_editable(CreateProjectRequest::new(
            project_location(&wide_path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                2_000_000,
                300_000,
                300,
                3_000,
                3_000,
                2,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the wide Project is structurally valid at its initial DPI");
    let wide_projection = wide.projection();
    let wide_bytes = fs::read(&wide_path).expect("the wide revision is persisted");

    assert_eq!(
        wide.apply(ProjectIntent::SetDpi { dpi: 1_200 })
            .expect_err("the raster axis would exceed the structural limit"),
        CoreError::InvalidDpi(1_200)
    );
    assert_eq!(wide.projection(), wide_projection);
    assert_eq!(
        fs::read(&wide_path).expect("the wide revision remains readable"),
        wide_bytes
    );
}

#[test]
fn undo_redo_and_a_divergent_dpi_branch_restore_authoritative_revisions() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Histórico de DPI.myalbuns");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project opens");
    let persisted_before = fs::read(&project_path).expect("the initial revision is persisted");

    let changed = project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the first DPI change is applied");
    assert_eq!(changed.state.revision, 1);

    let undone = project.undo().expect("the DPI change can be undone");
    assert_eq!(undone.state.document.dpi, 300);
    assert_eq!(undone.state.revision, 0);
    assert_eq!(undone.state.saved_revision, 0);
    assert!(!undone.state.dirty);
    assert!(!undone.state.can_undo);
    assert!(undone.state.can_redo);

    let redone = project.redo().expect("the DPI change can be redone");
    assert_eq!(redone.state.document.dpi, 240);
    assert_eq!(redone.state.revision, 1);
    assert!(redone.state.dirty);
    assert!(redone.state.can_undo);
    assert!(!redone.state.can_redo);

    project
        .undo()
        .expect("the history returns to its saved root");
    let branched = project
        .apply(ProjectIntent::SetDpi { dpi: 600 })
        .expect("a divergent DPI change is applied");
    assert_eq!(branched.state.document.dpi, 600);
    assert_eq!(branched.state.revision, 2);
    assert_eq!(branched.state.saved_revision, 0);
    assert!(branched.state.dirty);
    assert!(branched.state.can_undo);
    assert!(!branched.state.can_redo);
    assert!(
        project.redo().is_none(),
        "the abandoned branch is discarded"
    );
    assert_eq!(
        fs::read(&project_path).expect("the persisted revision remains readable"),
        persisted_before
    );
}

#[test]
fn exhausting_the_safe_revision_range_rejects_dpi_before_mutation() {
    const MAX_SAFE_REVISION: u64 = 9_007_199_254_740_991;

    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("Revisão máxima.myalbuns");
    let bytes = replace_literal_once(
        NEUTRAL_PROJECT_V1,
        "\"revision\": 0",
        "\"revision\": 9007199254740991",
    );
    fs::write(&project_path, &bytes).expect("the maximal public revision is written");
    let core = project_core_with_identity_storage(directory.path());
    let mut project = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the maximal public revision opens");

    assert_eq!(
        project
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect_err("the revision range is exhausted before mutation"),
        CoreError::RevisionSpaceExhausted
    );
    let projection = project.projection();
    assert_eq!(projection.state.document.dpi, 300);
    assert_eq!(projection.state.revision, MAX_SAFE_REVISION);
    assert_eq!(projection.state.saved_revision, MAX_SAFE_REVISION);
    assert!(!projection.state.dirty);
    assert!(!projection.state.can_undo);
    assert!(!projection.state.can_redo);
    assert!(project.render_snapshot().validate().is_ok());
    assert_eq!(
        fs::read(&project_path).expect("the maximal revision remains readable"),
        bytes
    );
}

#[test]
fn create_only_never_replaces_an_object_that_already_occupies_the_destination() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("conflito.myalbuns");
    let original = b"conteudo preexistente";
    fs::write(&project_path, original).expect("the destination exists");
    let lease_root = directory.path().join("leases");
    let core = ProjectCore::new()
        .with_identity_storage_roots(lease_root.clone(), directory.path().join("identities"));

    assert_eq!(
        core.create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect_err("CreateOnly must preserve every existing object"),
        CreateProjectError::DestinationConflict
    );
    assert_eq!(
        fs::read(&project_path).expect("destination remains"),
        original
    );
    assert_eq!(
        fs::read_dir(lease_root)
            .expect("the private lease root exists")
            .count(),
        0,
        "a conclusively failed creation leaves no private identity artifacts"
    );
}

#[cfg(windows)]
#[test]
fn concurrent_create_only_attempts_publish_exactly_one_complete_project() {
    use std::sync::{Arc, Barrier};

    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("corrida.myalbuns");
    let lease_root = directory.path().join("leases");
    let registry_root = directory.path().join("identities");
    let barrier = Arc::new(Barrier::new(6));
    let workers = (0..6)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let project_path = project_path.clone();
            let lease_root = lease_root.clone();
            let registry_root = registry_root.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ProjectCore::new()
                    .with_identity_storage_roots(lease_root, registry_root)
                    .create_editable(CreateProjectRequest::new(
                        project_location(&project_path),
                        InitialProject::neutral(),
                        CreateAuthorization::CreateOnly,
                    ))
                    .map(|project| project.project_id())
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("the creation worker completes"))
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .all(|error| *error == CreateProjectError::DestinationConflict)
    );
    let loaded = ProjectCore::new()
        .load_persisted_revision(load_request(&project_path))
        .expect("the winning candidate is a complete v1 document");
    assert_eq!(loaded.revision(), 0);
    assert_eq!(loaded.project().sheets().len(), 2);
}

#[cfg(windows)]
#[test]
fn creation_rejects_unsafe_children_through_the_public_core_boundary() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let valid_path = directory.path().join("válido.myalbuns");
    let mut paths = OperationPathContext::new();
    paths
        .capture(&valid_path)
        .expect("the valid fixture root is captured");
    let bindings = paths.freeze();
    let unsafe_paths = [
        directory.path().join("..").join("escape.myalbuns"),
        directory.path().join("NUL.myalbuns"),
        directory.path().join("*.myalbuns"),
        directory.path().join("fluxo.myalbuns:alternativo"),
    ];

    for unsafe_path in unsafe_paths {
        let error = project_core_with_identity_storage(directory.path())
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(unsafe_path.clone(), bindings.clone()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect_err("an unsafe child must never reach publication");
        assert_eq!(
            error,
            CreateProjectError::Path(PathFailure::InvalidPath),
            "unexpected result for {unsafe_path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn creates_and_reopens_a_non_ascii_project_beyond_the_legacy_path_limit() {
    use std::os::windows::ffi::OsStrExt;

    let directory = tempfile::tempdir().expect("temporary directory");
    let mut parent = directory.path().to_path_buf();
    for index in 0..9 {
        parent.push(format!("segmento-não-ascii-{index:02}-complementar"));
    }
    std::fs::create_dir_all(&parent).expect("the long parent is materialized");
    let project_path = parent.join("Álbum de família.myalbuns");
    assert!(project_path.as_os_str().encode_wide().count() > 260);
    let core = project_core_with_identity_storage(directory.path());

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the long native pathname is created");
    let project_id = created.project_id();
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the created long-path document reopens");
    assert_eq!(reopened.project_id(), project_id);
    assert_eq!(reopened.revision(), 0);
}

#[cfg(windows)]
#[test]
fn creates_and_reopens_through_an_accepted_forward_slash_windows_path() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let native_path = directory.path().join("Álbum com barras.myalbuns");
    let forward_slash_path =
        std::path::PathBuf::from(native_path.to_string_lossy().replace('\\', "/"));
    let core = project_core_with_identity_storage(directory.path());

    let created = core
        .create_editable(CreateProjectRequest::new(
            project_location(&forward_slash_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the accepted disk spelling publishes through the Win32 boundary");
    let project_id = created.project_id();
    drop(created);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(
            &forward_slash_path,
        )))
        .expect("the published Project reopens through the same accepted spelling");
    assert_eq!(reopened.project_id(), project_id);
    assert_eq!(reopened.revision(), 0);
}

#[test]
fn replace_confirmed_replaces_an_unprotected_file_but_not_an_open_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("substituivel.myalbuns");
    fs::write(&project_path, b"arquivo regular antigo").expect("ordinary file exists");
    let lease_root = directory.path().join("leases");
    let core = ProjectCore::new()
        .with_identity_storage_roots(lease_root, directory.path().join("identities"));

    let first_project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::ReplaceConfirmed,
        ))
        .expect("confirmed replacement may replace an unprotected regular file");
    let first_id = first_project.project_id();
    let protected_bytes = fs::read(&project_path).expect("first Project is present");

    assert_eq!(
        core.create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::ReplaceConfirmed,
        ))
        .expect_err("replacement consent never breaks an editable Project lock"),
        CreateProjectError::ProjectInUse
    );
    assert_eq!(
        fs::read(&project_path).expect("protected Project remains present"),
        protected_bytes
    );

    drop(first_project);
    let replacement = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::ReplaceConfirmed,
        ))
        .expect("dropping the owner permits the confirmed replacement");
    assert_ne!(replacement.project_id(), first_id);
    assert_eq!(replacement.revision(), 0);
}

#[test]
fn opening_invalid_content_leaves_no_editable_ownership_behind() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("qualquer-extensao.bin");
    fs::write(&project_path, b"not a project").expect("invalid fixture is written");
    let core = project_core_with_identity_storage(directory.path());

    assert!(matches!(
        core.open_editable(OpenProjectRequest::new(project_location(&project_path))),
        Err(OpenProjectError::Document(_))
    ));
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes())
        .expect("the failed attempt retained no physical lock");
    let opened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("valid content opens regardless of its explicit extension");
    assert_eq!(opened.revision(), 0);
}

#[test]
fn a_second_physical_copy_with_the_same_project_identity_requires_interactive_resolution() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let original_path = directory.path().join("original.myalbuns");
    let copy_path = directory.path().join("copia.myalbuns");
    fs::write(&original_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    fs::write(&copy_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("copied fixture");
    let core = project_core_with_identity_storage(directory.path());

    let original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the original acquires its Identity");
    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&copy_path)))
            .expect_err("a copied file cannot share an editable Identity"),
        OpenProjectError::ExternalCopyRequiresInteractiveResolution
    );
    drop(original);
    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&copy_path)))
            .expect_err("durable Identity evidence still protects the closed original"),
        OpenProjectError::ExternalCopyRequiresInteractiveResolution
    );

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the last authorized physical instance can reopen after closure");
    assert_eq!(
        reopened.project_id().to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
}

#[test]
fn a_possible_move_stays_fail_closed_until_the_identity_promotion_ticket() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let original_path = directory.path().join("original.myalbuns");
    let moved_path = directory.path().join("movido.myalbuns");
    fs::write(&original_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let core = project_core_with_identity_storage(directory.path());

    let original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the first observation authorizes the original");
    drop(original);
    fs::rename(&original_path, &moved_path).expect("the Project is moved after closure");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&moved_path)))
            .expect_err("issue #44 cannot classify or promote a possible movement"),
        OpenProjectError::IdentityIndeterminate
    );
}

#[test]
fn read_only_load_accepts_the_same_physical_project_while_it_is_open_for_editing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("original.myalbuns");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let original_bytes = fs::read(&project_path).expect("original bytes");
    let core = project_core_with_identity_storage(directory.path());

    let original = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the original opens for editing");
    let loaded = core
        .load_persisted_revision(load_request(&project_path))
        .expect("read-only loading accepts the same physical target");

    assert_eq!(loaded.project_id(), original.project_id());
    assert_eq!(loaded.revision(), original.revision());
    assert_eq!(
        fs::read(&project_path).expect("the source remains readable"),
        original_bytes,
        "read-only loading never rewrites the Project file"
    );
}

#[test]
fn read_only_load_rejects_an_external_physical_copy_of_an_open_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let original_path = directory.path().join("original.myalbuns");
    let copy_path = directory.path().join("copia.myalbuns");
    fs::write(&original_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let core = project_core_with_identity_storage(directory.path());

    let _original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the original opens for editing");
    fs::copy(&original_path, &copy_path).expect("the external physical copy is created");
    let copied_bytes = fs::read(&copy_path).expect("copied bytes");

    assert_eq!(
        core.load_persisted_revision(load_request(&copy_path))
            .expect_err("the copy needs interactive identity resolution"),
        LoadProjectError::ExternalCopyRequiresInteractiveResolution
    );
    assert_eq!(
        fs::read(&copy_path).expect("the copy remains readable"),
        copied_bytes,
        "classification never rewrites the copied Project file"
    );
}

#[test]
fn read_only_load_uses_the_durable_registry_after_the_original_session_closes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let original_path = directory.path().join("original.myalbuns");
    let copy_path = directory.path().join("copia.myalbuns");
    fs::write(&original_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let core = project_core_with_identity_storage(directory.path());

    let original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the original publishes durable Identity evidence");
    drop(original);
    fs::copy(&original_path, &copy_path).expect("the external copy is created after closure");
    let original_bytes = fs::read(&original_path).expect("original bytes");
    let copied_bytes = fs::read(&copy_path).expect("copied bytes");

    assert_eq!(
        core.load_persisted_revision(load_request(&copy_path))
            .expect_err("the durable registry still identifies a closed original"),
        LoadProjectError::ExternalCopyRequiresInteractiveResolution
    );
    assert_eq!(
        fs::read(&original_path).expect("original remains readable"),
        original_bytes
    );
    assert_eq!(
        fs::read(&copy_path).expect("copy remains readable"),
        copied_bytes
    );
}

#[test]
fn concurrent_first_read_only_observations_serialize_duplicate_identities() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let first_path = directory.path().join("primeira.myalbuns");
    let second_path = directory.path().join("segunda.myalbuns");
    fs::write(&first_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("first fixture");
    fs::write(&second_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("second fixture");
    let core = project_core_with_identity_storage(directory.path());
    let start = std::sync::Arc::new(std::sync::Barrier::new(3));
    let attempts = [first_path, second_path]
        .into_iter()
        .map(|path| {
            let core = core.clone();
            let start = std::sync::Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                core.load_persisted_revision(load_request(&path))
            })
        })
        .collect::<Vec<_>>();

    start.wait();
    let outcomes = attempts
        .into_iter()
        .map(|attempt| {
            attempt
                .join()
                .expect("the read-only attempt does not panic")
        })
        .collect::<Vec<_>>();

    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome,
                    Err(LoadProjectError::ExternalCopyRequiresInteractiveResolution)
                )
            })
            .count(),
        1,
        "the losing copy re-evaluates durable evidence under the Identity lease"
    );
}

#[test]
fn read_only_load_fails_closed_when_active_identity_evidence_is_invalid() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("original.myalbuns");
    let lease_root = directory.path().join("leases");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let core = ProjectCore::new()
        .with_identity_storage_roots(lease_root.clone(), directory.path().join("identities"));

    let _original = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the original opens for editing");
    fs::write(
        lease_root.join("550e8400-e29b-41d4-a716-446655440000.target"),
        b"invalid-local-identity\n",
    )
    .expect("the active evidence is corrupted for the fixture");

    assert_eq!(
        core.load_persisted_revision(load_request(&project_path))
            .expect_err("invalid active evidence must never be treated as safe"),
        LoadProjectError::IdentityIndeterminate
    );
}

fn assert_background_media(content: &BackgroundContent, expected_media_id: &str) {
    match content {
        BackgroundContent::Media { media_id } => {
            assert_eq!(media_id.to_string(), expected_media_id)
        }
        other => panic!("expected Background media, received {other:?}"),
    }
}

fn load_bytes(bytes: &[u8]) -> Result<LoadedProjectRevision, LoadProjectError> {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("fixture.myalbuns");
    fs::write(&project_path, bytes).expect("literal fixture is written");
    ProjectCore::new().load_persisted_revision(load_request(&project_path))
}

fn load_error(bytes: &[u8], case: &str) -> LoadProjectError {
    match load_bytes(bytes) {
        Ok(_) => panic!("{case}: the invalid literal unexpectedly loaded"),
        Err(error) => error,
    }
}

fn replace_literal_once(source: &str, from: &str, to: &str) -> Vec<u8> {
    assert_eq!(
        source.matches(from).count(),
        1,
        "the literal mutation must have exactly one unambiguous target"
    );
    source.replacen(from, to, 1).into_bytes()
}

fn load_request(project_path: &Path) -> LoadProjectRequest {
    LoadProjectRequest::new(project_location(project_path))
}

fn project_location(project_path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths
        .capture(project_path)
        .expect("the fixture has a supported absolute path");
    ProjectLocation::new(project_path.to_path_buf(), paths.freeze())
}

fn project_core_with_identity_storage(storage_root: &Path) -> ProjectCore {
    ProjectCore::new()
        .with_identity_storage_roots(storage_root.join("leases"), storage_root.join("identities"))
}
