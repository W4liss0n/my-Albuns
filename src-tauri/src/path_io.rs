use std::{path::PathBuf, thread::ThreadId};

use myalbuns_paths::{AppPathsError, OperationPathContext, RootBindingPlan};

pub(crate) async fn capture_root_bindings(
    paths: Vec<PathBuf>,
) -> Result<RootBindingPlan, AppPathsError> {
    capture_root_bindings_with_thread(paths)
        .await
        .map(|(bindings, _)| bindings)
}

async fn capture_root_bindings_with_thread(
    paths: Vec<PathBuf>,
) -> Result<(RootBindingPlan, ThreadId), AppPathsError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut owner = OperationPathContext::new();
        for path in paths {
            owner.capture(&path)?;
        }
        Ok((owner.freeze(), std::thread::current().id()))
    })
    .await
    .map_err(|_| AppPathsError::OperationPathIoFailure)?
}

#[cfg(test)]
mod tests {
    use std::thread;

    #[test]
    fn path_binding_capture_runs_on_the_blocking_pool() {
        let root = tempfile::tempdir().expect("temporary path-I/O fixture");
        let path = root.path().join("Foto.jpg");
        std::fs::write(&path, b"photo").expect("the path-I/O fixture is writable");
        let caller_thread = thread::current().id();

        let (bindings, io_thread) =
            tauri::async_runtime::block_on(super::capture_root_bindings_with_thread(vec![
                path.clone(),
            ]))
            .expect("the native binding is captured");

        assert!(bindings.covers(&path));
        assert_ne!(
            io_thread, caller_thread,
            "potentially remote path I/O must not run on the caller/UI thread"
        );
    }
}
