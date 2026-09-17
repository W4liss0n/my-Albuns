use std::future::Future;

use myalbuns_core::EditorProjection;
use tauri::AppHandle;

use crate::{
    image_processing::prepare_changed_images,
    ipc_contract::ImageProcessingProgress,
    media_runtime::MediaBinding,
    project_host::ProjectHost,
    project_ui_operations::{self, ProjectUiOperation},
};

pub(crate) async fn execute_in_app<T>(
    app: &AppHandle,
    host: &ProjectHost,
    mutate: impl FnOnce() -> Result<T, String>,
    publish: impl FnMut(ImageProcessingProgress),
) -> Result<(T, EditorProjection), String> {
    execute(
        project_ui_operations::begin(app)?,
        host,
        mutate,
        |previous| async move { prepare_changed_images(app, &previous, publish).await },
    )
    .await
}

/// Keeps UI recovery waiting through mutation, image preparation and the authoritative reread.
pub(crate) async fn execute<T, F: Future<Output = Result<(), String>>>(
    operation: ProjectUiOperation,
    host: &ProjectHost,
    mutate: impl FnOnce() -> Result<T, String>,
    prepare: impl FnOnce(Vec<MediaBinding>) -> F,
) -> Result<(T, EditorProjection), String> {
    let previous = host.authorized_media_catalog()?.bindings;
    let result = mutate()?;
    prepare(previous).await?;
    let projection = host.projection()?;
    drop(operation);
    Ok((result, projection))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_ui_operations::ProjectUiOperations;
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectIntent,
        ProjectLocation,
    };
    use myalbuns_paths::OperationPathContext;
    use std::{
        cell::Cell,
        task::{Context, Poll, Waker},
    };

    fn fixture() -> (tempfile::TempDir, ProjectHost) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Commands.myalbuns");
        let mut paths = OperationPathContext::new();
        paths.capture(&path).unwrap();
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(path, paths.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        (root, ProjectHost::new(project))
    }

    #[test]
    fn holds_recovery_until_preparation_and_returns_the_final_projection_for_all_commands() {
        tauri::async_runtime::block_on(async {
            let (_root, host) = fixture();
            let operations = ProjectUiOperations::default();
            for command in ["apply", "undo", "redo"] {
                let (release, prepared) = tokio::sync::oneshot::channel();
                let previous = host.authorized_media_catalog().unwrap().bindings;
                let mut running = Box::pin(execute(
                    operations.begin().unwrap(),
                    &host,
                    || match command {
                        "apply" => host
                            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                            .map(|value| value.projection),
                        "undo" => host.undo(),
                        _ => host.redo(),
                    },
                    |captured| async move {
                        assert_eq!(captured, previous);
                        prepared.await.unwrap();
                        Ok(())
                    },
                ));
                assert!(matches!(
                    running
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
                let recovery = operations.recover().unwrap();
                assert!(!recovery.is_drained());
                assert!(operations.begin().is_err());
                release.send(()).unwrap();
                let (original, completed) = running.await.unwrap();
                assert_eq!(completed, host.projection().unwrap());
                assert_eq!(
                    completed.state.document.dpi,
                    if command == "undo" { 300 } else { 360 }
                );
                assert_eq!(completed, original);
                assert!(recovery.is_drained());
                drop(recovery);
                assert!(operations.is_idle());
            }
        });
    }

    #[test]
    fn failures_release_the_operation_and_rejected_mutations_never_prepare_images() {
        tauri::async_runtime::block_on(async {
            let (_root, host) = fixture();
            let operations = ProjectUiOperations::default();
            let prepared = Cell::new(false);
            let result = execute(
                operations.begin().unwrap(),
                &host,
                || Err::<(), _>("rejected".into()),
                |_| async {
                    prepared.set(true);
                    Ok(())
                },
            )
            .await;
            assert_eq!(result.unwrap_err(), "rejected");
            assert!(!prepared.get());
            assert!(operations.is_idle());
            let result = execute(
                operations.begin().unwrap(),
                &host,
                || host.apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 }),
                |_| async { Err("preparation failed".into()) },
            )
            .await;
            assert_eq!(result.unwrap_err(), "preparation failed");
            assert_eq!(host.projection().unwrap().state.document.dpi, 360);
            assert!(operations.is_idle());
        });
    }

    #[test]
    fn rereads_source_observations_published_during_preparation() {
        tauri::async_runtime::block_on(async {
            let (root, host) = fixture();
            let photo = root.path().join("Photo.png");
            image::RgbImage::from_pixel(48, 32, image::Rgb([20, 80, 160]))
                .save(&photo)
                .unwrap();
            host.import_photos(vec![photo], |_| {}).unwrap();
            let operations = ProjectUiOperations::default();
            let host = &host;
            let (mutated, completed) = execute(
                operations.begin().unwrap(),
                host,
                || host.apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 }),
                |previous| async move {
                    assert_eq!(previous.len(), 1);
                    host.observe_photo_source(
                        &previous[0],
                        myalbuns_core::PhotoSourceMetadata::new(
                            96,
                            64,
                            std::array::from_fn(|_| "#FFFFFF".into()),
                        )
                        .unwrap(),
                    )
                },
            )
            .await
            .unwrap();
            assert_eq!(
                mutated.projection.state.album.media[0].source_width_px,
                Some(48)
            );
            assert_eq!(completed.state.album.media[0].source_width_px, Some(96));
            assert_eq!(completed.state.revision, mutated.projection.state.revision);
            assert!(operations.is_idle());
        });
    }

    #[test]
    fn captures_bindings_before_removal_and_before_each_history_command() {
        tauri::async_runtime::block_on(async {
            let (root, host) = fixture();
            let photo = root.path().join("Photo.png");
            image::RgbImage::from_pixel(48, 32, image::Rgb([20, 80, 160]))
                .save(&photo)
                .unwrap();
            host.import_photos(vec![photo], |_| {}).unwrap();
            let imported = host.authorized_media_catalog().unwrap().bindings;
            let operations = ProjectUiOperations::default();
            let host = &host;
            let imported = &imported;
            for (command, previous_count, final_count) in
                [("remove", 1, 0), ("undo", 0, 1), ("redo", 1, 0)]
            {
                let (_, completed) = execute(
                    operations.begin().unwrap(),
                    host,
                    || match command {
                        "remove" => host
                            .apply_with_outcome(ProjectIntent::RemoveMedia {
                                media_ids: vec![imported[0].media_id.parse().unwrap()],
                                mode: myalbuns_core::MediaRemovalMode::RemoveAll,
                            })
                            .map(|value| value.projection),
                        "undo" => host.undo(),
                        _ => host.redo(),
                    },
                    |previous| async move {
                        assert_eq!(previous.len(), previous_count);
                        if previous_count == 1 {
                            assert_eq!(&previous, imported);
                        }
                        assert_eq!(
                            host.authorized_media_catalog().unwrap().bindings.len(),
                            final_count
                        );
                        Ok(())
                    },
                )
                .await
                .unwrap();
                assert_eq!(completed.state.album.media.len(), final_count);
            }
        });
    }
}
