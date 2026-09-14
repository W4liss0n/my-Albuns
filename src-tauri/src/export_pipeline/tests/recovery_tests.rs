use super::*;

#[derive(Default)]
struct InterruptedRender {
    requests: Vec<Vec<usize>>,
    fail: bool,
}
impl ImagingTransport for InterruptedRender {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        _operation: ImagingOperation,
        _attempt: u8,
        _control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let ImagingCommand::RenderAlbum(request) = command else {
            panic!("album request")
        };
        request.validate().unwrap();
        self.requests.push(
            request
                .outputs
                .iter()
                .map(|output| output.units[0].index)
                .collect(),
        );
        let mut receipts = Vec::new();
        let fail_at = usize::from(request.outputs.len() > 1);
        let mut failure = None;
        for (index, output) in request.outputs.iter().enumerate() {
            let path = request
                .root_bindings
                .resolve(output.prepared_path.as_path())
                .unwrap();
            assert!(
                !path.exists(),
                "unfinished bytes must be removed before restarting their encoder"
            );
            if self.fail && index == fail_at {
                std::fs::write(&path, b"unfinished").unwrap();
                failure = Some(myalbuns_imaging_protocol::ImagingFailure {
                    code: ImagingFailureCode::OutputStorageFull,
                    media_id: None,
                    path_code: None,
                });
                break;
            }
            let bytes = format!("completed-{}", output.units[0].index).into_bytes();
            std::fs::write(&path, &bytes).unwrap();
            receipts.push(RenderCompletion {
                width_px: 1,
                height_px: 1,
                dpi: request.snapshot.dpi,
                source_count: request.sources.len(),
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&bytes)),
            });
        }
        let completion = myalbuns_imaging_protocol::AlbumRenderCompletion { outputs: receipts };
        let response = if let Some(failure) = failure {
            ImagingResponse::AlbumStorageFull {
                request_id: request.request_id.clone(),
                completion,
                failure,
            }
        } else {
            ImagingResponse::AlbumCompleted {
                request_id: request.request_id.clone(),
                completion,
            }
        };
        Box::pin(async move { Ok(response) })
    }
}

#[test]
fn storage_recovery_restarts_only_unfinished_files_and_preserves_the_snapshot() {
    use myalbuns_core::{ExportFormat, ExportMode};
    for format in [
        ExportFormat::Jpeg { quality: 100 },
        ExportFormat::Png,
        ExportFormat::Pdf,
    ] {
        let root = tempfile::tempdir().unwrap();
        let plan = skip_export_plan(root.path(), ExportMode::Sheet, format.clone());
        let folder = plan.preparation_directory().to_owned();
        let mut paths = OperationPathContext::new();
        for path in plan.required_paths() {
            paths.capture(&path).unwrap();
        }
        let mut transport = InterruptedRender {
            fail: true,
            ..Default::default()
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut failure = runtime
            .block_on(super::super::execute_album(
                &mut transport,
                plan,
                &paths.freeze(),
                &ExportExecutionControl::default(),
                &|_| {},
                &context("skip-existing"),
            ))
            .unwrap_err();
        assert!(failure.is_storage_full());
        assert!(folder.exists());
        #[cfg(windows)]
        assert!(std::fs::write(root.path().join("original.jpg"), b"changed while paused").is_err());
        let first = folder.join(format!("Album_001.{}", format.extension()));
        let prior = (format != ExportFormat::Pdf).then(|| std::fs::read(&first).unwrap());
        let repeated = failure.recovery.take().unwrap();
        failure = runtime
            .block_on(super::super::resume_album(
                &mut transport,
                repeated,
                &ExportExecutionControl::default(),
                &|_| {},
                &context("skip-existing"),
            ))
            .unwrap_err();
        assert!(
            failure.is_storage_full(),
            "each explicit retry can pause again without losing completed outputs"
        );
        transport.fail = false;
        runtime
            .block_on(super::super::resume_album(
                &mut transport,
                failure.recovery.take().unwrap(),
                &ExportExecutionControl::default(),
                &|_| {},
                &context("skip-existing"),
            ))
            .unwrap();
        assert_eq!(
            transport.requests,
            if format == ExportFormat::Pdf {
                vec![vec![1], vec![1], vec![1]]
            } else {
                vec![vec![1, 2], vec![2], vec![2]]
            }
        );
        if let Some(prior) = prior {
            assert_eq!(
                std::fs::read(
                    root.path()
                        .join(format!("Album_001.{}", format.extension()))
                )
                .unwrap(),
                prior
            );
        }
        assert!(!folder.exists());
        std::fs::write(
            root.path().join("original.jpg"),
            b"released after completion",
        )
        .unwrap();
    }
}

#[test]
fn discarding_a_paused_export_releases_originals_and_cleans_only_its_preparation() {
    let root = tempfile::tempdir().unwrap();
    let plan = skip_export_plan(
        root.path(),
        myalbuns_core::ExportMode::Sheet,
        myalbuns_core::ExportFormat::Png,
    );
    let folder = plan.preparation_directory().to_owned();
    let mut paths = OperationPathContext::new();
    for path in plan.required_paths() {
        paths.capture(&path).unwrap();
    }
    std::fs::write(root.path().join("unrelated.png"), b"keep").unwrap();
    let mut transport = InterruptedRender {
        fail: true,
        ..Default::default()
    };
    let failure = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(super::super::execute_album(
            &mut transport,
            plan,
            &paths.freeze(),
            &ExportExecutionControl::default(),
            &|_| {},
            &context("skip-existing"),
        ))
        .unwrap_err();
    assert!(failure.recovery.is_some());
    drop(failure);
    assert!(!folder.exists());
    assert_eq!(
        std::fs::read(root.path().join("unrelated.png")).unwrap(),
        b"keep"
    );
    std::fs::write(root.path().join("original.jpg"), b"released").unwrap();
}

#[test]
fn modified_retained_output_is_rejected_before_any_render_or_publication() {
    let root = tempfile::tempdir().unwrap();
    let plan = skip_export_plan(
        root.path(),
        myalbuns_core::ExportMode::Sheet,
        myalbuns_core::ExportFormat::Png,
    );
    let folder = plan.preparation_directory().to_owned();
    let mut paths = OperationPathContext::new();
    for path in plan.required_paths() {
        paths.capture(&path).unwrap();
    }
    let mut transport = InterruptedRender {
        fail: true,
        ..Default::default()
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut failure = runtime
        .block_on(super::super::execute_album(
            &mut transport,
            plan,
            &paths.freeze(),
            &ExportExecutionControl::default(),
            &|_| {},
            &context("skip-existing"),
        ))
        .unwrap_err();
    std::fs::write(folder.join("Album_001.png"), b"corrupted").unwrap();
    let failure = runtime
        .block_on(super::super::resume_album(
            &mut transport,
            failure.recovery.take().unwrap(),
            &ExportExecutionControl::default(),
            &|_| {},
            &context("skip-existing"),
        ))
        .unwrap_err();
    assert_eq!(failure.stage, ExportFailureStage::VerifyPreparation);
    assert_eq!(transport.requests.len(), 1);
    assert!(!root.path().join("Album_001.png").exists());
}
