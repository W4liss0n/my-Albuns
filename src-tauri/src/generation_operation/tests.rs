use super::*;
use myalbuns_core::{CreateAuthorization, CreateProjectRequest, InitialProject, ProjectLocation};
use myalbuns_paths::OperationPathContext;
use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct Presentation {
    opened: Arc<AtomicUsize>,
    release: Arc<tokio::sync::Notify>,
    events: Arc<StdMutex<Vec<&'static str>>>,
}
struct Surface(Arc<StdMutex<Vec<&'static str>>>);
impl Drop for Surface {
    fn drop(&mut self) {
        self.0.lock().unwrap().push("closed");
    }
}
impl GenerationPresentation for Presentation {
    type Surface = Surface;
    async fn open(&self, _: GenerationProgress) -> Result<Surface, String> {
        self.opened.fetch_add(1, Ordering::Release);
        self.events.lock().unwrap().push("opened");
        self.release.notified().await;
        Ok(Surface(self.events.clone()))
    }
    fn progress(&self, _: GenerationProgress) {
        self.events.lock().unwrap().push("progress");
    }
    async fn finish(&self, _: &GenerationView) -> Result<(), String> {
        self.events.lock().unwrap().push("result-ready");
        Ok(())
    }
}

fn fixture() -> (tempfile::TempDir, GenerationRequest) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("Fotos");
    let destination = root.path().join("Projetos");
    std::fs::create_dir_all(source.join("001")).unwrap();
    std::fs::create_dir(&destination).unwrap();
    image::RgbImage::new(8, 6)
        .save(source.join("001/foto.png"))
        .unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let model_path = root.path().join("Modelo.myalbuns");
    let mut paths = OperationPathContext::new();
    paths.capture(&model_path).unwrap();
    let model = core
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(model_path, paths.freeze()),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let request = GenerationRequest::Prepare {
        options: GenerationOptions {
            source_folder: source.to_string_lossy().into(),
            destination_folder: destination.to_string_lossy().into(),
        },
        template: Box::new(model.freeze_template().unwrap()),
        core,
    };
    (root, request)
}

#[tokio::test]
async fn verification_waits_for_the_progress_surface_before_touching_the_source() {
    let (root, request) = fixture();
    let source = root.path().join("Fotos");
    let destination = root.path().join("Projetos");
    let presentation = Presentation::default();
    let runner = Arc::new(Mutex::new(None));
    let attempt = execute_generation(
        request,
        runner.clone(),
        Arc::new(AtomicBool::new(false)),
        presentation.clone(),
    );
    tokio::pin!(attempt);
    tokio::select! {
        result = &mut attempt => panic!("Verification finished without first showing progress: {}", result.is_ok()),
        _ = async { while presentation.opened.load(Ordering::Acquire) == 0 { tokio::task::yield_now().await; } } => {}
    }
    assert!(
        runner.try_lock().is_err(),
        "The attempt owns its runner while the progress opens"
    );
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
    image::RgbImage::new(9, 7)
        .save(source.join("001/foto.png"))
        .unwrap();
    presentation.release.notify_one();
    let view = attempt.await.unwrap().unwrap();
    assert_eq!(view.phase, crate::ipc_contract::GenerationPhase::Finished);
    assert!(destination.join("001.myalbuns").is_file());
    assert_eq!(presentation.opened.load(Ordering::Acquire), 1);
    let events = presentation.events.lock().unwrap();
    assert_eq!(events.first(), Some(&"opened"));
    assert_eq!(&events[events.len() - 2..], &["result-ready", "closed"]);
}

#[tokio::test]
async fn conflicts_return_to_the_result_before_closing_progress_and_recheck_does_not_generate() {
    let (root, request) = fixture();
    let output = root.path().join("Projetos/001.myalbuns");
    std::fs::write(&output, b"existing file").unwrap();
    let runner = Arc::new(Mutex::new(None));
    let presentation = Presentation::default();
    presentation.release.notify_one();
    let view = execute_generation(
        request,
        runner.clone(),
        Arc::new(AtomicBool::new(false)),
        presentation.clone(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(view.phase, crate::ipc_contract::GenerationPhase::Prepared);
    assert!(!view.can_continue);
    assert!(view.items[0].conflict);
    assert_eq!(std::fs::read(&output).unwrap(), b"existing file");
    assert_eq!(
        *presentation.events.lock().unwrap(),
        ["opened", "result-ready", "closed"]
    );

    presentation.release.notify_one();
    let cancelled = execute_generation(
        GenerationRequest::Recheck,
        runner.clone(),
        Arc::new(AtomicBool::new(true)),
        presentation.clone(),
    )
    .await
    .unwrap();
    assert!(cancelled.is_none());
    assert_eq!(runner.lock().await.as_ref().unwrap().view().id, view.id);
    assert_eq!(std::fs::read(&output).unwrap(), b"existing file");

    std::fs::remove_file(&output).unwrap();
    presentation.release.notify_one();
    let view = execute_generation(
        GenerationRequest::Recheck,
        runner.clone(),
        Arc::new(AtomicBool::new(false)),
        presentation.clone(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(view.can_continue);
    assert_eq!(view.phase, crate::ipc_contract::GenerationPhase::Prepared);
    assert!(
        !output.exists(),
        "A recheck still requires an explicit Continue"
    );

    presentation.release.notify_one();
    let view = execute_generation(
        GenerationRequest::Run,
        runner,
        Arc::new(AtomicBool::new(false)),
        presentation,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(view.phase, crate::ipc_contract::GenerationPhase::Finished);
    assert!(output.is_file());
}

#[tokio::test]
async fn cancellation_before_verification_releases_progress_without_writing() {
    let (root, request) = fixture();
    let presentation = Presentation::default();
    presentation.release.notify_one();
    let result = execute_generation(
        request,
        Arc::new(Mutex::new(None)),
        Arc::new(AtomicBool::new(true)),
        presentation.clone(),
    )
    .await;
    assert!(
        result.unwrap().is_none(),
        "Cancellation returns to the existing configuration"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("Projetos"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(*presentation.events.lock().unwrap(), ["opened", "closed"]);
}
