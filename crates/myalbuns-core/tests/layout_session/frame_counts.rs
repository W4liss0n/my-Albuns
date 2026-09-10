use myalbuns_core::{
    CoreError, FrameOrientation, LayoutFrameRequest, LayoutSelection, ProjectIntent,
};

fn request(frame_count: usize) -> Option<LayoutFrameRequest> {
    Some(LayoutFrameRequest {
        frame_count,
        orientation: FrameOrientation::Horizontal,
    })
}

#[test]
fn reducing_after_expansion_and_unlock_preserves_photos_and_is_one_undoable_edit() {
    for (count, lock) in [(2, false), (3, false), (2, true)] {
        let root = tempfile::tempdir().unwrap();
        let mut project = super::visual_corpus::fixture_project(root.path(), 4);
        let sheet = project.projection().state.album.sheets[0].id.clone();
        let expanded = project
            .query_layouts_with_frame_request(&sheet, request(6))
            .unwrap();
        project
            .apply(ProjectIntent::LockLayout {
                selection: LayoutSelection {
                    query_id: expanded.query_id,
                    candidate_index: 0,
                },
            })
            .unwrap();
        project
            .apply(ProjectIntent::UnlockLayout {
                sheet_id: sheet.clone(),
            })
            .unwrap();
        let before = project.projection();
        assert_eq!(before.state.album.sheets[0].frames.len(), 6);
        let filled: Vec<_> = before.state.album.sheets[0]
            .frames
            .iter()
            .filter(|frame| frame.photo.is_some())
            .cloned()
            .collect();
        assert_eq!(filled.len(), 2);

        let query = project
            .query_layouts_with_frame_request(&sheet, request(count))
            .expect("surplus placeholders must not prevent a smaller requested Layout");
        assert!(
            query.listing.candidates.iter().all(|candidate| candidate
                .layout
                .definition
                .positions
                .len()
                == count)
        );
        let selection = LayoutSelection {
            query_id: query.query_id,
            candidate_index: 0,
        };
        let preview = project.preview_layout(&selection).unwrap();
        assert_eq!(preview.len(), count);
        assert_eq!(
            project.projection(),
            before,
            "query and hover do not remove placeholders"
        );
        let after = project
            .apply(if lock {
                ProjectIntent::LockLayout { selection }
            } else {
                ProjectIntent::ApplyLayout { selection }
            })
            .unwrap();
        let frames = &after.state.album.sheets[0].frames;
        assert_eq!(frames.len(), count);
        assert_eq!(after.composition.sheets[0].frames, preview);
        assert_eq!(after.state.revision, before.state.revision + 1);
        assert_eq!(after.state.album.sheets[0].layout_locked, lock);
        assert_eq!(after.media_usage, before.media_usage);
        for original in &filled {
            let retained = frames.iter().find(|frame| frame.id == original.id).unwrap();
            assert_eq!(retained.photo, original.photo);
            assert_eq!(retained.style, original.style);
        }
        let retained_ids: Vec<_> = frames.iter().map(|frame| &frame.id).collect();
        let expected_ids: Vec<_> = before.state.album.sheets[0]
            .frames
            .iter()
            .filter(|frame| {
                frame.photo.is_some()
                    || (count == 3 && frame.id == before.state.album.sheets[0].frames[1].id)
            })
            .map(|frame| &frame.id)
            .collect();
        assert_eq!(
            retained_ids, expected_ids,
            "retained Frames keep their original order"
        );
        let undone = project.undo().unwrap();
        assert_eq!(undone.state.album, before.state.album);
        assert_eq!(undone.composition, before.composition);
        assert_eq!(project.redo().unwrap(), after);
    }
}

#[test]
fn count_requests_cannot_remove_photos_or_bypass_a_locked_layout() {
    let root = tempfile::tempdir().unwrap();
    let mut project = super::visual_corpus::fixture_project(root.path(), 4);
    let sheet = project.projection().state.album.sheets[0].id.clone();
    let before = project.projection();
    assert_eq!(
        project.query_layouts_with_frame_request(&sheet, request(1)),
        Err(CoreError::InvalidLayoutQuery)
    );
    assert_eq!(
        project.query_layouts_with_frame_request(&sheet, request(31)),
        Err(CoreError::InvalidLayoutQuery)
    );
    assert_eq!(project.projection(), before);
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let locked = project.projection();
    assert_eq!(
        project.query_layouts_with_frame_request(&sheet, request(2)),
        Err(CoreError::LayoutLocked)
    );
    assert_eq!(project.projection(), locked);
}

#[test]
fn filling_a_placeholder_invalidates_a_prepared_reduction() {
    let root = tempfile::tempdir().unwrap();
    let mut project = super::visual_corpus::fixture_project(root.path(), 4);
    let before = project.projection();
    let sheet = before.state.album.sheets[0].id.clone();
    let query = project
        .query_layouts_with_frame_request(&sheet, request(2))
        .unwrap();
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: sheet,
            media_id: before.state.album.media[0].id,
            mode: myalbuns_core::PhotoPlacementMode::Normal,
        })
        .unwrap();
    let filled = project.projection();
    assert_eq!(
        filled.state.album.sheets[0]
            .frames
            .iter()
            .filter(|frame| frame.photo.is_some())
            .count(),
        3
    );
    assert_eq!(
        project.apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            }
        }),
        Err(CoreError::StaleLayoutPreview)
    );
    assert_eq!(project.projection(), filled);
}
