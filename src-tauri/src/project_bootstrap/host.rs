use std::thread::ThreadId;

use myalbuns_core::{
    CreateAuthorization, CreateProjectError, CreateProjectRequest, DocumentFailure,
    EditableProject, InitialProject, OpenProjectError, OpenProjectRequest, PathFailure,
    ProjectCore, ProjectLocation, SaveCopyAsError, SaveCopyAsRequest,
};
use myalbuns_paths::{AppPaths, ProcessInstanceId};

use super::{
    BootstrapIntent, BootstrapRequest, CreateWriteAuthorization, FailureCode, FailureStage,
    HostTerminal, InitialProjectCreationConfiguration, SaveExternalCopyRequest,
    to_core_initial_project,
};

#[derive(Debug)]
pub(crate) struct BootstrappedHostProject {
    request: BootstrapRequest,
    project: EditableProject,
}

pub(crate) struct PendingExternalCopyHost {
    request: BootstrapRequest,
    core: ProjectCore,
    source: myalbuns_core::ExternalCopySource,
}

pub(crate) enum HostBootstrap {
    Ready(BootstrappedHostProject),
    ExternalCopyNotWritable(PendingExternalCopyHost),
}

enum BootstrapWorkerError {
    Failed(FailureStage, FailureCode),
    FocusExisting {
        project_id: String,
        owner_process: ProcessInstanceId,
    },
    ExternalCopyNotWritable {
        core: ProjectCore,
        source: Box<myalbuns_core::ExternalCopySource>,
        worker_thread: ThreadId,
    },
}

impl PendingExternalCopyHost {
    pub(crate) fn terminal(&self) -> HostTerminal {
        HostTerminal::external_copy_not_writable(&self.request)
    }

    pub(crate) fn invalid_continuation_terminal(&self) -> HostTerminal {
        HostTerminal::failed(
            &self.request,
            FailureStage::Decode,
            FailureCode::InvalidRequest,
        )
    }

    pub(crate) fn save_copy_as(
        self,
        continuation: SaveExternalCopyRequest,
    ) -> Result<BootstrappedHostProject, HostTerminal> {
        if continuation.protocol_version != super::protocol::PROTOCOL_VERSION
            || continuation.attempt_id != self.request.attempt_id
            || continuation.launch_nonce != self.request.launch_nonce
            || !continuation.authority.validates_target_binding()
        {
            return Err(HostTerminal::failed(
                &self.request,
                FailureStage::Resolve,
                FailureCode::InvalidRequest,
            ));
        }
        let destination = ProjectLocation::new(
            continuation.authority.logical_target.into_path_buf(),
            continuation.authority.root_bindings,
        );
        let request = self.request;
        let worker = std::thread::spawn(move || {
            self.core.save_copy_as(SaveCopyAsRequest::new(
                self.source,
                destination,
                create_authorization(continuation.authorization),
            ))
        });
        match worker.join() {
            Ok(Ok(project)) => Ok(BootstrappedHostProject { request, project }),
            Ok(Err(error)) => Err(HostTerminal::failed(
                &request,
                FailureStage::SaveCopy,
                map_save_copy_error(error),
            )),
            Err(_) => Err(HostTerminal::failed(
                &request,
                FailureStage::SaveCopy,
                FailureCode::IoFailure,
            )),
        }
    }
}

impl BootstrappedHostProject {
    #[cfg(test)]
    pub(crate) fn project(&self) -> &EditableProject {
        &self.project
    }

    pub(crate) fn into_parts(self) -> (BootstrapRequest, EditableProject) {
        (self.request, self.project)
    }

    #[cfg(test)]
    pub(crate) fn ready_terminal(&self) -> HostTerminal {
        HostTerminal::ready(
            &self.request,
            self.project.project_id().hyphenated().to_string(),
            self.project.revision(),
        )
    }
}

#[cfg(any(debug_assertions, test))]
pub(crate) fn bootstrap_host_project(
    request: BootstrapRequest,
    app_paths: &AppPaths,
) -> Result<BootstrappedHostProject, HostTerminal> {
    match bootstrap_host_project_or_pending(request, app_paths)? {
        HostBootstrap::Ready(opened) => Ok(opened),
        HostBootstrap::ExternalCopyNotWritable(pending) => Err(pending.terminal()),
    }
}

pub(crate) fn bootstrap_host_project_or_pending(
    request: BootstrapRequest,
    app_paths: &AppPaths,
) -> Result<HostBootstrap, HostTerminal> {
    bootstrap_host_project_with_thread(request, app_paths).map(|(opened, _)| opened)
}

fn bootstrap_host_project_with_thread(
    request: BootstrapRequest,
    app_paths: &AppPaths,
) -> Result<(HostBootstrap, ThreadId), HostTerminal> {
    if request.protocol_version != super::protocol::PROTOCOL_VERSION
        || request.attempt_id.is_empty()
        || request.launch_nonce.is_empty()
        || !request.authority.validates_target_binding()
    {
        return Err(HostTerminal::failed(
            &request,
            FailureStage::Resolve,
            FailureCode::InvalidRequest,
        ));
    }

    let project_path = request.authority.logical_target.clone().into_path_buf();
    let root_bindings = request.authority.root_bindings.clone();
    let intent = request.intent.clone();
    let identity_lease_root = app_paths.project_identity_leases_dir();
    let identity_registry_root = app_paths.project_identities_dir();
    let worker = std::thread::spawn(move || {
        let worker_thread = std::thread::current().id();
        let core = ProjectCore::new()
            .with_identity_storage_roots(identity_lease_root, identity_registry_root);
        let location = ProjectLocation::new(project_path, root_bindings);
        let project = match intent {
            BootstrapIntent::OpenExisting => {
                match core.open_editable(OpenProjectRequest::new(location)) {
                    Ok(project) => project,
                    Err(OpenProjectError::FocusExisting {
                        project_id,
                        owner_process,
                    }) => {
                        return Err(BootstrapWorkerError::FocusExisting {
                            project_id: project_id.hyphenated().to_string(),
                            owner_process,
                        });
                    }
                    Err(OpenProjectError::ExternalCopyNotWritable(source)) => {
                        return Err(BootstrapWorkerError::ExternalCopyNotWritable {
                            core,
                            source,
                            worker_thread,
                        });
                    }
                    Err(error) => {
                        return Err(BootstrapWorkerError::Failed(
                            FailureStage::Open,
                            map_open_error(error),
                        ));
                    }
                }
            }
            BootstrapIntent::CreateNew {
                configuration,
                authorization,
            } => {
                let initial = initial_project(*configuration).map_err(|()| {
                    BootstrapWorkerError::Failed(
                        FailureStage::Create,
                        FailureCode::InvalidInitialProject,
                    )
                })?;
                core.create_editable(CreateProjectRequest::new(
                    location,
                    initial,
                    create_authorization(authorization),
                ))
                .map_err(|error| {
                    BootstrapWorkerError::Failed(FailureStage::Create, map_create_error(error))
                })?
            }
        };
        Ok::<_, BootstrapWorkerError>((project, worker_thread))
    });

    match worker.join() {
        Ok(Ok((project, worker_thread))) => Ok((
            HostBootstrap::Ready(BootstrappedHostProject { request, project }),
            worker_thread,
        )),
        Ok(Err(BootstrapWorkerError::Failed(stage, code))) => {
            Err(HostTerminal::failed(&request, stage, code))
        }
        Ok(Err(BootstrapWorkerError::FocusExisting {
            project_id,
            owner_process,
        })) => Err(HostTerminal::focus_existing(
            &request,
            project_id,
            owner_process,
        )),
        Ok(Err(BootstrapWorkerError::ExternalCopyNotWritable {
            core,
            source,
            worker_thread,
        })) => Ok((
            HostBootstrap::ExternalCopyNotWritable(PendingExternalCopyHost {
                request,
                core,
                source: *source,
            }),
            worker_thread,
        )),
        Err(_) => Err(HostTerminal::failed(
            &request,
            FailureStage::Initialize,
            FailureCode::IoFailure,
        )),
    }
}

fn initial_project(
    configuration: InitialProjectCreationConfiguration,
) -> Result<InitialProject, ()> {
    to_core_initial_project(configuration).ok_or(())
}

fn create_authorization(authorization: CreateWriteAuthorization) -> CreateAuthorization {
    match authorization {
        CreateWriteAuthorization::CreateOnly => CreateAuthorization::CreateOnly,
        CreateWriteAuthorization::ReplaceConfirmed => CreateAuthorization::ReplaceConfirmed,
    }
}

fn map_create_error(error: CreateProjectError) -> FailureCode {
    match error {
        CreateProjectError::InvalidInitialProject => FailureCode::InvalidInitialProject,
        CreateProjectError::Path(error) => map_path_error(error),
        CreateProjectError::DestinationConflict => FailureCode::DestinationConflict,
        CreateProjectError::ProjectInUse => FailureCode::ProjectInUse,
        CreateProjectError::IdentityIndeterminate => FailureCode::IdentityIndeterminate,
        CreateProjectError::CreateStateIndeterminate => FailureCode::CreateStateIndeterminate,
    }
}

fn map_open_error(error: OpenProjectError) -> FailureCode {
    match error {
        OpenProjectError::Path(error) => map_path_error(error),
        OpenProjectError::Document(error) => match error {
            DocumentFailure::InvalidDocumentType => FailureCode::InvalidDocumentType,
            DocumentFailure::UnsupportedFutureSchema { .. } => FailureCode::UnsupportedFutureSchema,
            DocumentFailure::UnsupportedLegacySchema { .. } => FailureCode::UnsupportedLegacySchema,
            DocumentFailure::InvalidProjectDocument => FailureCode::InvalidProjectDocument,
            DocumentFailure::InvalidProjectState => FailureCode::InvalidProjectState,
        },
        OpenProjectError::ProjectInUse => FailureCode::ProjectInUse,
        OpenProjectError::FocusExisting { .. } => FailureCode::ProjectInUse,
        OpenProjectError::ExternalCopyRequiresInteractiveResolution => {
            FailureCode::ExternalCopyRequiresInteractiveResolution
        }
        OpenProjectError::ExternalCopyNotWritable(_) => FailureCode::ExternalCopyNotWritable,
        OpenProjectError::IdentityIndeterminate => FailureCode::IdentityIndeterminate,
    }
}

fn map_save_copy_error(error: SaveCopyAsError) -> FailureCode {
    match error {
        SaveCopyAsError::Path(error) => map_path_error(error),
        SaveCopyAsError::DestinationConflict => FailureCode::DestinationConflict,
        SaveCopyAsError::ProjectInUse => FailureCode::ProjectInUse,
        SaveCopyAsError::IdentityIndeterminate => FailureCode::IdentityIndeterminate,
        SaveCopyAsError::SaveCopyStateIndeterminate => FailureCode::SaveCopyStateIndeterminate,
    }
}

fn map_path_error(error: PathFailure) -> FailureCode {
    match error {
        PathFailure::NotFound => FailureCode::NotFound,
        PathFailure::Unavailable => FailureCode::Unavailable,
        PathFailure::AccessDenied => FailureCode::AccessDenied,
        PathFailure::InvalidPath => FailureCode::InvalidPath,
        PathFailure::UnexpectedObjectType => FailureCode::UnexpectedObjectType,
        PathFailure::Conflict => FailureCode::Conflict,
        PathFailure::IoFailure => FailureCode::IoFailure,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        thread,
    };

    use myalbuns_core::{
        ActiveSides, CreateAuthorization, CreateProjectRequest, DisplayUnit, InitialProject,
        ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, NativePathDto, OperationPathContext};

    use super::super::configuration::{
        InitialBackground, InitialBackgroundContent, InitialDisplayUnit,
        InitialDocumentConfiguration, InitialFrameBorder, InitialOverlay,
        InitialProjectCreationConfiguration, InitialSheetFormat, InitialStructureConfiguration,
        InitialVisualDefaults,
    };

    use super::*;

    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "the Desktop test suite runs on Windows and clears FILE_ATTRIBUTE_READONLY on its fixture"
    )]
    fn make_writable(path: &Path) {
        let mut permissions = std::fs::metadata(path)
            .expect("the read-only fixture still has metadata")
            .permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).expect("the Windows fixture becomes writable");
    }

    fn configured_project() -> InitialProjectCreationConfiguration {
        InitialProjectCreationConfiguration {
            document: InitialDocumentConfiguration {
                display_unit: InitialDisplayUnit::Cm,
                sheet_width_um: 508_000,
                sheet_height_um: 254_000,
                dpi: 240,
                bleed_um: 4_000,
                safety_um: 7_500,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 3,
                first_sheet: InitialSheetFormat::SinglePage,
                last_sheet: InitialSheetFormat::Double,
            },
            visual_defaults: InitialVisualDefaults {
                background: InitialBackground::BothSides {
                    both: InitialBackgroundContent::Color {
                        rgb: "#FFFFFF".into(),
                    },
                },
                overlay: InitialOverlay::BothSides { both: None },
                frame_border: InitialFrameBorder::None,
            },
        }
    }

    struct Fixture {
        _root: tempfile::TempDir,
        paths: AppPaths,
        project_path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_relative_path(Path::new("Álbum de família.myalbuns"))
        }

        fn with_relative_path(relative_path: &Path) -> Self {
            let root = tempfile::tempdir().expect("temporary Host fixture");
            let paths = AppPaths::from_roots(root.path(), root.path());
            let project_path = root.path().join(relative_path);
            std::fs::create_dir_all(
                project_path
                    .parent()
                    .expect("the Project fixture has a parent"),
            )
            .expect("the Project fixture parent is materialized");
            Self {
                _root: root,
                paths,
                project_path,
            }
        }

        fn bindings(&self) -> myalbuns_paths::RootBindingPlan {
            let mut context = OperationPathContext::new();
            context
                .capture(&self.project_path)
                .expect("the fixture root is captured");
            context.freeze()
        }

        fn request(&self) -> BootstrapRequest {
            self.request_for(self.project_path.clone(), self.bindings())
        }

        fn create_request(&self, authorization: CreateWriteAuthorization) -> BootstrapRequest {
            BootstrapRequest {
                protocol_version: super::super::protocol::PROTOCOL_VERSION,
                attempt_id: "attempt-create".into(),
                launch_nonce: "nonce-create".into(),
                intent: BootstrapIntent::CreateNew {
                    configuration: Box::new(configured_project()),
                    authorization,
                },
                authority: super::super::TargetAuthority {
                    logical_target: NativePathDto::from(self.project_path.clone()),
                    root_bindings: self.bindings(),
                },
            }
        }

        fn request_for(
            &self,
            logical_target: PathBuf,
            root_bindings: myalbuns_paths::RootBindingPlan,
        ) -> BootstrapRequest {
            BootstrapRequest {
                protocol_version: super::super::protocol::PROTOCOL_VERSION,
                attempt_id: "attempt-open".into(),
                launch_nonce: "nonce-open".into(),
                intent: BootstrapIntent::OpenExisting,
                authority: super::super::TargetAuthority {
                    logical_target: NativePathDto::from(logical_target),
                    root_bindings,
                },
            }
        }

        fn create_project(&self) -> String {
            let core = ProjectCore::new().with_identity_storage_roots(
                self.paths.project_identity_leases_dir(),
                self.paths.project_identities_dir(),
            );
            let project = core
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(self.project_path.clone(), self.bindings()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .expect("the v1 project fixture is created");
            project.project_id().hyphenated().to_string()
        }
    }

    #[test]
    fn the_host_opens_one_productive_project_off_the_caller_thread() {
        let fixture = Fixture::new();
        let project_id = fixture.create_project();
        let caller_thread = thread::current().id();

        let (opened, worker_thread) =
            bootstrap_host_project_with_thread(fixture.request(), &fixture.paths)
                .expect("the Host opens the v1 document");
        let HostBootstrap::Ready(opened) = opened else {
            panic!("the writable fixture must be ready");
        };

        assert_ne!(worker_thread, caller_thread);
        assert_eq!(
            opened.project().project_id().hyphenated().to_string(),
            project_id
        );
        assert_eq!(opened.project().revision(), 0);
        assert_eq!(
            opened.ready_terminal(),
            HostTerminal::Ready {
                attempt_id: "attempt-open".into(),
                launch_nonce: "nonce-open".into(),
                host_pid: std::process::id(),
                project_id,
                revision: 0,
            }
        );
    }

    #[test]
    fn the_host_converts_the_wire_configuration_and_keeps_its_editable_session() {
        let fixture = Fixture::new();
        let caller_thread = thread::current().id();

        let (created, worker_thread) = bootstrap_host_project_with_thread(
            fixture.create_request(CreateWriteAuthorization::CreateOnly),
            &fixture.paths,
        )
        .expect("the Host creates the v1 document");
        let HostBootstrap::Ready(created) = created else {
            panic!("the valid creation must be ready");
        };

        assert_ne!(worker_thread, caller_thread);
        assert_eq!(created.project().revision(), 0);
        assert_eq!(created.project().saved_revision(), 0);
        let document = created.project().project();
        let settings = document.document();
        assert_eq!(settings.display_unit(), DisplayUnit::Cm);
        assert_eq!(settings.sheet_width_um(), 508_000);
        assert_eq!(settings.sheet_height_um(), 254_000);
        assert_eq!(settings.dpi(), 240);
        assert_eq!(settings.bleed_um(), 4_000);
        assert_eq!(settings.safety_um(), 7_500);
        assert_eq!(document.sheets().len(), 3);
        assert_eq!(document.sheets()[0].active_sides(), ActiveSides::Right);
        assert_eq!(document.sheets()[1].active_sides(), ActiveSides::Both);
        assert_eq!(document.sheets()[2].active_sides(), ActiveSides::Both);
        assert!(document.media().is_empty());
        assert!(fixture.project_path.is_file());
        assert!(matches!(
            ProjectCore::new()
                .with_identity_storage_roots(
                    fixture.paths.project_identity_leases_dir(),
                    fixture.paths.project_identities_dir(),
                )
                .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                    fixture.project_path.clone(),
                    fixture.bindings(),
                ))),
            Err(OpenProjectError::FocusExisting { .. })
        ));
        assert!(matches!(
            created.ready_terminal(),
            HostTerminal::Ready {
                attempt_id,
                launch_nonce,
                revision: 0,
                ..
            } if attempt_id == "attempt-create" && launch_nonce == "nonce-create"
        ));
    }

    #[test]
    fn host_creation_preserves_destination_conflict_and_project_in_use() {
        let conflict = Fixture::new();
        std::fs::write(&conflict.project_path, b"concorrente")
            .expect("the concurrent destination is materialized");
        assert!(matches!(
            bootstrap_host_project(
                conflict.create_request(CreateWriteAuthorization::CreateOnly),
                &conflict.paths,
            ),
            Err(HostTerminal::Failed {
                stage: FailureStage::Create,
                code: FailureCode::DestinationConflict,
                ..
            })
        ));

        let protected = Fixture::new();
        let owner = ProjectCore::new()
            .with_identity_storage_roots(
                protected.paths.project_identity_leases_dir(),
                protected.paths.project_identities_dir(),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(protected.project_path.clone(), protected.bindings()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the existing Project is protected");
        assert!(matches!(
            bootstrap_host_project(
                protected.create_request(CreateWriteAuthorization::ReplaceConfirmed),
                &protected.paths,
            ),
            Err(HostTerminal::Failed {
                stage: FailureStage::Create,
                code: FailureCode::ProjectInUse,
                ..
            })
        ));
        drop(owner);
    }

    #[test]
    fn a_second_host_receives_focus_existing_without_disturbing_the_owner() {
        let fixture = Fixture::new();
        fixture.create_project();
        let owner = bootstrap_host_project(fixture.request(), &fixture.paths)
            .expect("the first Host owns the Project");

        let failure = bootstrap_host_project(fixture.request(), &fixture.paths)
            .expect_err("a second editable Host is rejected");

        assert_eq!(
            failure,
            HostTerminal::FocusExisting {
                attempt_id: "attempt-open".into(),
                launch_nonce: "nonce-open".into(),
                host_pid: std::process::id(),
                project_id: owner.project().project_id().hyphenated().to_string(),
                owner_process: ProcessInstanceId::current()
                    .expect("the owning process instance is captured"),
            }
        );
        assert_eq!(owner.project().revision(), 0);
        drop(owner);
        assert!(bootstrap_host_project(fixture.request(), &fixture.paths).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn the_host_offers_and_completes_save_copy_as_for_a_read_only_external_copy() {
        let fixture = Fixture::new();
        let original_id = fixture.create_project();
        let read_only_path = fixture._root.path().join("Cópia somente leitura.myalbuns");
        let destination_path = fixture._root.path().join("Cópia editável.myalbuns");
        std::fs::copy(&fixture.project_path, &read_only_path)
            .expect("the external copy is created");
        let mut permissions = std::fs::metadata(&read_only_path)
            .expect("the external copy has metadata")
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&read_only_path, permissions)
            .expect("the external copy becomes read-only");
        let source_bytes = std::fs::read(&read_only_path).expect("source bytes are captured");
        let mut source_paths = OperationPathContext::new();
        source_paths
            .capture(&read_only_path)
            .expect("the source root is captured");
        let source = super::super::TargetAuthority {
            logical_target: NativePathDto::from(read_only_path.clone()),
            root_bindings: source_paths.freeze(),
        };
        let open_request =
            fixture.request_for(read_only_path.clone(), source.root_bindings.clone());

        let caller_thread = thread::current().id();
        let (classification, worker_thread) =
            bootstrap_host_project_with_thread(open_request, &fixture.paths)
                .expect("the Host classifies the read-only external copy");
        assert_ne!(worker_thread, caller_thread);
        let pending = match classification {
            HostBootstrap::ExternalCopyNotWritable(pending) => pending,
            HostBootstrap::Ready(_) => panic!("a read-only duplicate cannot become Ready"),
        };
        assert!(matches!(
            pending.terminal(),
            HostTerminal::ExternalCopyNotWritable { .. }
        ));

        let mut destination_paths = OperationPathContext::new();
        destination_paths
            .capture(&destination_path)
            .expect("the destination root is captured");
        let saved = pending
            .save_copy_as(SaveExternalCopyRequest {
                protocol_version: super::super::protocol::PROTOCOL_VERSION,
                attempt_id: "attempt-open".into(),
                launch_nonce: "nonce-open".into(),
                authority: super::super::TargetAuthority {
                    logical_target: NativePathDto::from(destination_path.clone()),
                    root_bindings: destination_paths.freeze(),
                },
                authorization: CreateWriteAuthorization::CreateOnly,
            })
            .expect("the same Host publishes the editable copy");

        assert_ne!(
            saved.project().project_id().hyphenated().to_string(),
            original_id
        );
        assert_eq!(saved.project().revision(), 0);
        assert_eq!(saved.project().saved_revision(), 0);
        assert!(!saved.project().can_undo());
        assert_eq!(
            std::fs::read(&read_only_path).expect("the source remains readable"),
            source_bytes
        );
        assert!(destination_path.is_file());
        make_writable(&read_only_path);
    }

    #[test]
    fn invalid_documents_fail_structurally_without_leaving_a_session() {
        let fixture = Fixture::new();
        std::fs::write(&fixture.project_path, b"not a project")
            .expect("the invalid fixture is writable");

        let failure = bootstrap_host_project(fixture.request(), &fixture.paths)
            .expect_err("invalid JSON cannot become Ready");

        assert!(matches!(
            failure,
            HostTerminal::Failed {
                stage: FailureStage::Open,
                code: FailureCode::InvalidProjectDocument,
                ..
            }
        ));
    }

    #[cfg(windows)]
    #[test]
    fn the_host_consumes_frozen_mapped_unc_and_verbatim_bindings_without_rediscovery() {
        let fixture = Fixture::new();
        let project_id = fixture.create_project();
        let logical_targets = [
            PathBuf::from(r"R:\Álbum de família.myalbuns"),
            PathBuf::from(r"\\host-que-não-existe\acervo\Álbum de família.myalbuns"),
            PathBuf::from(r"\\?\Q:\Álbum de família.myalbuns"),
            PathBuf::from(r"\\?\UNC\host-que-não-existe\acervo\Álbum de família.myalbuns"),
        ];

        for logical_target in logical_targets {
            let mut context = OperationPathContext::new();
            context
                .capture_with_binding(&logical_target, fixture._root.path())
                .expect("the owner captures the authoritative operational root");
            let opened = bootstrap_host_project(
                fixture.request_for(logical_target.clone(), context.freeze()),
                &fixture.paths,
            )
            .unwrap_or_else(|terminal| {
                panic!("the Host must use the frozen binding for {logical_target:?}: {terminal:?}")
            });

            assert_eq!(
                opened.project().project_id().hyphenated().to_string(),
                project_id
            );
            drop(opened);
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_host_opens_a_real_non_ascii_project_beyond_the_legacy_path_limit() {
        use std::os::windows::ffi::OsStrExt;

        let mut relative = PathBuf::new();
        for index in 0..9 {
            relative.push(format!("segmento-não-ascii-{index:02}-complementar"));
        }
        relative.push("Álbum de família.myalbuns");
        let fixture = Fixture::with_relative_path(&relative);
        assert!(
            fixture.project_path.as_os_str().encode_wide().count() > 260,
            "the real Project path must cross MAX_PATH"
        );
        let source = Fixture::new();
        let project_id = source.create_project();
        let document = std::fs::read(&source.project_path)
            .expect("the valid shallow Project fixture is readable");
        std::fs::write(&fixture.project_path, document)
            .expect("the valid v1 document is copied to the long pathname");

        let opened = bootstrap_host_project(fixture.request(), &fixture.paths)
            .expect("the Host opens the long native Project pathname");

        assert_eq!(
            opened.project().project_id().hyphenated().to_string(),
            project_id
        );
    }

    #[test]
    fn missing_and_wrong_type_targets_fail_with_distinct_terminal_codes() {
        let missing = Fixture::new();
        assert!(matches!(
            bootstrap_host_project(missing.request(), &missing.paths),
            Err(HostTerminal::Failed {
                stage: FailureStage::Open,
                code: FailureCode::NotFound,
                ..
            })
        ));

        let directory = Fixture::new();
        std::fs::create_dir(&directory.project_path)
            .expect("the wrong-type target is materialized as a directory");
        assert!(matches!(
            bootstrap_host_project(directory.request(), &directory.paths),
            Err(HostTerminal::Failed {
                stage: FailureStage::Open,
                code: FailureCode::UnexpectedObjectType,
                ..
            })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn a_disappeared_operational_root_is_unavailable_instead_of_missing() {
        let fixture = Fixture::new();
        let operational_root = fixture._root.path().join("origem-offline");
        std::fs::create_dir(&operational_root).expect("the source root starts available");
        let logical_target = PathBuf::from(r"R:\Álbum de família.myalbuns");
        let mut context = OperationPathContext::new();
        context
            .capture_with_binding(&logical_target, &operational_root)
            .expect("the owner freezes the available source root");
        let bindings = context.freeze();
        std::fs::remove_dir(&operational_root).expect("the source becomes unavailable");

        let failure = bootstrap_host_project(
            fixture.request_for(logical_target, bindings),
            &fixture.paths,
        )
        .expect_err("an unavailable source cannot become a missing Project");

        assert!(matches!(
            failure,
            HostTerminal::Failed {
                stage: FailureStage::Open,
                code: FailureCode::Unavailable,
                ..
            }
        ));
    }

    #[test]
    fn every_typed_path_failure_keeps_its_bootstrap_code() {
        let cases = [
            (PathFailure::NotFound, FailureCode::NotFound),
            (PathFailure::Unavailable, FailureCode::Unavailable),
            (PathFailure::AccessDenied, FailureCode::AccessDenied),
            (PathFailure::InvalidPath, FailureCode::InvalidPath),
            (
                PathFailure::UnexpectedObjectType,
                FailureCode::UnexpectedObjectType,
            ),
            (PathFailure::Conflict, FailureCode::Conflict),
            (PathFailure::IoFailure, FailureCode::IoFailure),
        ];

        for (path_failure, expected_code) in cases {
            assert_eq!(
                map_open_error(OpenProjectError::Path(path_failure)),
                expected_code
            );
        }
    }
}
