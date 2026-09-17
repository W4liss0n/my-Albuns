use myalbuns_core::*;
use myalbuns_paths::OperationPathContext;

fn create(
    root: &std::path::Path,
    count: i64,
    first: EndSheetFormat,
    last: EndSheetFormat,
) -> EditableProject {
    let path = root.join("Album.myalbuns");
    let mut paths = OperationPathContext::new();
    paths.capture(&path).unwrap();
    ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(path, paths.freeze()),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                3_000,
                3_000,
                count,
                first,
                last,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap()
}

#[test]
fn projected_structure_agrees_with_every_command_and_reorder_position() {
    for count in [2, 3, 5] {
        for first in [EndSheetFormat::Double, EndSheetFormat::SinglePage] {
            for last in [EndSheetFormat::Double, EndSheetFormat::SinglePage] {
                let root = tempfile::tempdir().unwrap();
                let mut project = create(root.path(), count, first, last);
                let sheets = project.projection().state.album.sheets;
                for (index, sheet) in sheets.iter().enumerate() {
                    let id = sheet.id.clone();
                    let facts = &sheet.structure;
                    let a = &facts.availability;
                    let mut commands = vec![
                        (
                            ProjectIntent::AddSheet {
                                anchor_sheet_id: id.clone(),
                                position: SheetInsertionPosition::Before,
                            },
                            a.can_add_before,
                        ),
                        (
                            ProjectIntent::AddSheet {
                                anchor_sheet_id: id.clone(),
                                position: SheetInsertionPosition::After,
                            },
                            a.can_add_after,
                        ),
                        (
                            ProjectIntent::DuplicateSheet {
                                sheet_id: id.clone(),
                            },
                            a.can_duplicate,
                        ),
                        (
                            ProjectIntent::DeleteSheet {
                                sheet_id: id.clone(),
                            },
                            a.can_delete,
                        ),
                        (
                            ProjectIntent::ConvertEdgeSheet {
                                sheet_id: id.clone(),
                            },
                            a.can_convert_edge,
                        ),
                    ];
                    for target_index in 0..=sheets.len() {
                        commands.push((
                            ProjectIntent::ReorderSheet {
                                sheet_id: id.clone(),
                                target_index,
                            },
                            target_index != index
                                && target_index >= facts.minimum_reorder_index
                                && target_index <= facts.maximum_reorder_index,
                        ));
                    }
                    for (intent, expected) in commands {
                        let before = project.projection();
                        let result = project.apply(intent.clone());
                        assert_eq!(
                            result.is_ok(),
                            expected,
                            "{count}, {first:?}, {last:?}, {intent:?}"
                        );
                        if expected {
                            let undone = project.undo().unwrap();
                            assert_eq!(undone.state.album, before.state.album);
                            assert_eq!(undone.composition, before.composition);
                        } else {
                            assert_eq!(project.projection(), before, "rejection must be atomic");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn projected_conversion_losses_follow_the_actual_transform_in_both_entrypoints() {
    for index in [0, 2] {
        for mode in [
            "background",
            "overlay",
            "emptyOverlay",
            "bothSides",
            "keptSide",
            "default",
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut project = create(
                root.path(),
                3,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            );
            let sheet_id = project.projection().state.album.sheets[index].id.clone();
            let lost_side = if index == 0 { "left" } else { "right" };
            let kept_side = if index == 0 { "right" } else { "left" };
            let scope = if mode == "bothSides" {
                "bothSides"
            } else if mode == "keptSide" {
                kept_side
            } else {
                lost_side
            };
            if mode == "overlay" {
                let original = root.path().join("Overlay.png");
                std::fs::write(&original, b"linked original").unwrap();
                let imported = project
                    .import_media(
                        MediaKind::Decorative,
                        vec![ImportMedia::new(
                            original,
                            PhotoSourceMetadata::new(
                                600,
                                300,
                                std::array::from_fn(|_| "#FFFFFF".into()),
                            )
                            .unwrap(),
                        )],
                    )
                    .unwrap();
                let media_id = imported.projection.state.album.media[0].id;
                project.apply(serde_json::from_value(serde_json::json!({
                    "kind":"applyDecorative", "sheetId":sheet_id, "scope":scope, "role":"overlay", "mediaId":media_id
                })).unwrap()).unwrap();
            } else if mode != "default" {
                let change = if mode == "emptyOverlay" {
                    serde_json::json!({"kind":"remove", "role":"overlay"})
                } else {
                    serde_json::json!({"kind":"backgroundColor", "rgb":"#123456"})
                };
                project.apply(serde_json::from_value(serde_json::json!({
                    "kind":"editSheetVisual", "sheetId":sheet_id, "scope":scope, "change":change
                })).unwrap()).unwrap();
            }
            let before = project.projection();
            let loss = &before.state.album.sheets[index].edge_conversion_loss;
            assert_eq!(
                loss.is_some(),
                matches!(mode, "background" | "overlay"),
                "{index} {mode}"
            );
            if let Some(loss) = loss {
                assert_eq!(loss.sheet_id, sheet_id);
                assert_eq!(loss.sheet_number, index + 1);
                assert_eq!(serde_json::to_value(&loss.side).unwrap(), lost_side);
            }
            let information = AlbumInformation {
                display_unit: DisplayUnit::Mm,
                sheet_width_um: 600_000,
                sheet_height_um: 300_000,
                dpi: 300,
                bleed_um: 3_000,
                safety_um: 3_000,
                first_sheet: if index == 0 {
                    EndSheetFormat::SinglePage
                } else {
                    EndSheetFormat::Double
                },
                last_sheet: if index == 2 {
                    EndSheetFormat::SinglePage
                } else {
                    EndSheetFormat::Double
                },
            };
            let validation = project.validate_album_information(&information);
            assert_eq!(
                validation.impact.unwrap().conversion_losses,
                loss.iter().cloned().collect::<Vec<_>>()
            );
            assert_eq!(
                project.projection(),
                before,
                "query does not mutate or create History"
            );
            let direct = project
                .apply(ProjectIntent::ConvertEdgeSheet {
                    sheet_id: sheet_id.clone(),
                })
                .unwrap();
            assert!(
                direct.state.album.sheets[index]
                    .edge_conversion_loss
                    .is_none(),
                "expansion has no losses"
            );
            project.undo().unwrap();
            let configured = project
                .apply(ProjectIntent::SetAlbumInformation {
                    information,
                    expected_dimension_key: None,
                })
                .unwrap();
            assert_eq!(configured.state.album, direct.state.album);
            assert_eq!(configured.composition, direct.composition);
            project.undo().unwrap();
            assert_eq!(project.projection().state.album, before.state.album);
        }
    }
}
