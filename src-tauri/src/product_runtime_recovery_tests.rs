use std::{fs, path::Path};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, ImportPhoto, InitialProject, OpenProjectRequest,
    PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
    RelinkMedia,
};
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheBasicColorProfile, CacheFingerprint,
};
use myalbuns_paths::{AppPaths, OperationPathContext};

use super::{HostProjectRecoveryDecision, HostProjectRecoveryResolution, initialize_project_host};
use crate::{
    cache_engine::{CacheSourceBinding, RecoveredCacheArtifact},
    project_recovery::{RecoveryCoordinator, RecoveryStore},
};

fn location(path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), paths.freeze())
}

enum RecoveredPhoto {
    Saved,
    Imported,
    Relinked,
}

fn recover_cached_photo(scenario: RecoveredPhoto) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Album.myalbuns");
    let original = root.path().join("Retrato.jpg");
    fs::write(&original, b"linked Original").unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(
            original.clone(),
            PhotoSourceMetadata::new(
                800,
                1200,
                ["#102030".into(), "#405060".into(), "#708090".into()],
            )
            .unwrap(),
        ))
        .unwrap();
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: imported.projection.state.album.sheets[0].id.clone(),
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    if !matches!(scenario, RecoveredPhoto::Imported) {
        project.save(2).unwrap();
    }
    let saved = fs::read(&path).unwrap();
    let replacement = root.path().join("Paisagem.jpg");
    let (width, height, effective_path) = if matches!(scenario, RecoveredPhoto::Relinked) {
        fs::write(&replacement, b"relinked Original").unwrap();
        project
            .relink_media(RelinkMedia::new(imported.media_id, replacement.clone()))
            .unwrap();
        (1200, 800, &replacement)
    } else {
        (800, 1200, &original)
    };
    project.apply(ProjectIntent::SetDpi { dpi: 360 }).unwrap();
    let store = RecoveryStore::new(AppPaths::from_roots(
        &root.path().join("roaming"),
        &root.path().join("local"),
    ));
    store
        .publish(
            project.identity_authority(),
            &project.recovery_checkpoint().unwrap(),
        )
        .unwrap();
    drop(project);
    // Recovery must remain proportional even when only verified Cache is available.
    fs::remove_file(&original).unwrap();
    if replacement.exists() {
        fs::remove_file(&replacement).unwrap();
    }
    let reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let cached_artifact = |source: &Path, width_px, height_px, generation: &str| {
        RecoveredCacheArtifact::new(
            CacheArtifact {
                media_id: imported.media_id.to_string(),
                generation_id: generation.into(),
                width_px,
                height_px,
                preview_bytes: 1,
                format: CacheArtifactFormat::Jpeg,
                exif_orientation: Some(6),
                source_page_count: None,
                basic_color_profile: CacheBasicColorProfile::Srgb,
                fingerprint: CacheFingerprint::sha256_full_file(1, "a".repeat(64)).unwrap(),
            },
            CacheSourceBinding::for_path(source),
        )
    };
    let mut artifacts = Vec::new();
    if matches!(scenario, RecoveredPhoto::Relinked) {
        artifacts.push(cached_artifact(&original, 800, 1200, "g-old-binding"));
    }
    artifacts.push(cached_artifact(
        effective_path,
        width,
        height,
        "g-effective-binding",
    ));
    let host = initialize_project_host(
        reopened,
        RecoveryCoordinator::new(store),
        &artifacts,
        |host| {
            assert!(matches!(
                host.resolve_recovery(HostProjectRecoveryDecision::ReopenAndRecover)?,
                HostProjectRecoveryResolution::Recovered(_)
            ));
            Ok(true)
        },
    )
    .unwrap()
    .unwrap();
    let recovered = host.projection().unwrap();
    let photo = recovered.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap();
    let ratio = photo.draw_rect.width as f64 / photo.draw_rect.height as f64;
    assert!(
        (ratio - width as f64 / height as f64).abs() < 0.0001,
        "recovering must preserve the effective Photo's proportions: draw ratio {ratio}"
    );
    assert_eq!(recovered.state.album.media[0].source_width_px, Some(width));
    assert_eq!(
        recovered.state.album.media[0].source_height_px,
        Some(height)
    );
    assert_eq!(
        &host.authorized_media_catalog().unwrap().bindings[0].logical_path,
        effective_path
    );
    assert_eq!(recovered.state.document.dpi, 360);
    assert!(recovered.state.dirty);
    assert!(!recovered.state.can_undo);
    assert!(!recovered.state.can_redo);
    assert_eq!(fs::read(&path).unwrap(), saved);
}

#[test]
fn recovered_session_keeps_cached_photo_proportions() {
    recover_cached_photo(RecoveredPhoto::Saved);
}

#[test]
fn recovered_import_uses_cache_from_the_restored_catalog() {
    recover_cached_photo(RecoveredPhoto::Imported);
}

#[test]
fn recovered_relink_uses_cache_from_the_restored_binding() {
    recover_cached_photo(RecoveredPhoto::Relinked);
}
