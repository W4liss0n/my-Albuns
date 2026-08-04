use std::{fs, path::Path};

use myalbuns_core::{
    ActiveSides, Background, BackgroundContent, CreateAuthorization, CreateProjectError,
    CreateProjectRequest, DisplayUnit, DocumentFailure, FrameBorder, InitialProject,
    LoadProjectError, LoadProjectRequest, LoadedProjectRevision, OpenProjectError,
    OpenProjectRequest, Overlay, OverlayContent, ProjectCore, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

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
                "\"schemaVersion\": 2",
            ),
            DocumentFailure::UnsupportedFutureSchema { version: 2 },
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
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

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
fn create_only_never_replaces_an_object_that_already_occupies_the_destination() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("conflito.myalbuns");
    let original = b"conteudo preexistente";
    fs::write(&project_path, original).expect("the destination exists");
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

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
}

#[test]
fn replace_confirmed_replaces_an_unprotected_file_but_not_an_open_project() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("substituivel.myalbuns");
    fs::write(&project_path, b"arquivo regular antigo").expect("ordinary file exists");
    let lease_root = directory.path().join("leases");
    let core = ProjectCore::new().with_identity_lease_root(lease_root);

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
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

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
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

    let original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the original acquires its Identity");
    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&copy_path)))
            .expect_err("a copied file cannot share an editable Identity"),
        OpenProjectError::ExternalCopyRequiresInteractiveResolution
    );
    drop(original);
    core.open_editable(OpenProjectRequest::new(project_location(&copy_path)))
        .expect("the classification did not leave a residual physical lock");
}

#[test]
fn read_only_load_accepts_the_same_physical_project_while_it_is_open_for_editing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("original.myalbuns");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let original_bytes = fs::read(&project_path).expect("original bytes");
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

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
    let core = ProjectCore::new().with_identity_lease_root(directory.path().join("leases"));

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
fn read_only_load_fails_closed_when_active_identity_evidence_is_invalid() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let project_path = directory.path().join("original.myalbuns");
    let lease_root = directory.path().join("leases");
    fs::write(&project_path, NEUTRAL_PROJECT_V1.as_bytes()).expect("original fixture");
    let core = ProjectCore::new().with_identity_lease_root(lease_root.clone());

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
