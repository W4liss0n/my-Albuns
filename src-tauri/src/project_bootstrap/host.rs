use std::thread::ThreadId;

use myalbuns_core::{
    DocumentFailure, EditableProject, OpenProjectError, OpenProjectRequest, PathFailure,
    ProjectCore, ProjectLocation,
};
use myalbuns_paths::AppPaths;

use super::{BootstrapIntent, BootstrapRequest, FailureCode, FailureStage, HostTerminal};

#[derive(Debug)]
pub(crate) struct OpenedHostProject {
    request: BootstrapRequest,
    project: EditableProject,
}

impl OpenedHostProject {
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

pub(crate) fn open_host_project(
    request: BootstrapRequest,
    app_paths: &AppPaths,
) -> Result<OpenedHostProject, HostTerminal> {
    open_host_project_with_thread(request, app_paths).map(|(opened, _)| opened)
}

fn open_host_project_with_thread(
    request: BootstrapRequest,
    app_paths: &AppPaths,
) -> Result<(OpenedHostProject, ThreadId), HostTerminal> {
    if request.protocol_version != super::protocol::PROTOCOL_VERSION
        || request.attempt_id.is_empty()
        || request.launch_nonce.is_empty()
        || request.intent != BootstrapIntent::OpenExisting
        || request.authority.root_bindings.validate().is_err()
        || !request
            .authority
            .root_bindings
            .covers(request.authority.logical_target.as_path())
    {
        return Err(HostTerminal::failed(
            &request,
            FailureStage::Resolve,
            FailureCode::InvalidRequest,
        ));
    }

    let project_path = request.authority.logical_target.clone().into_path_buf();
    let root_bindings = request.authority.root_bindings.clone();
    let identity_lease_root = app_paths.project_identity_leases_dir();
    let worker = std::thread::spawn(move || {
        let worker_thread = std::thread::current().id();
        let project = ProjectCore::new()
            .with_identity_lease_root(identity_lease_root)
            .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                project_path,
                root_bindings,
            )))
            .map_err(map_open_error)?;
        Ok::<_, FailureCode>((project, worker_thread))
    });

    match worker.join() {
        Ok(Ok((project, worker_thread))) => {
            Ok((OpenedHostProject { request, project }, worker_thread))
        }
        Ok(Err(code)) => Err(HostTerminal::failed(&request, FailureStage::Open, code)),
        Err(_) => Err(HostTerminal::failed(
            &request,
            FailureStage::Initialize,
            FailureCode::IoFailure,
        )),
    }
}

fn map_open_error(error: OpenProjectError) -> FailureCode {
    match error {
        OpenProjectError::Path(error) => match error {
            PathFailure::NotFound => FailureCode::NotFound,
            PathFailure::Unavailable => FailureCode::Unavailable,
            PathFailure::AccessDenied => FailureCode::AccessDenied,
            PathFailure::InvalidPath => FailureCode::InvalidPath,
            PathFailure::UnexpectedObjectType => FailureCode::UnexpectedObjectType,
            PathFailure::Conflict => FailureCode::Conflict,
            PathFailure::IoFailure => FailureCode::IoFailure,
        },
        OpenProjectError::Document(error) => match error {
            DocumentFailure::InvalidDocumentType => FailureCode::InvalidDocumentType,
            DocumentFailure::UnsupportedFutureSchema { .. } => FailureCode::UnsupportedFutureSchema,
            DocumentFailure::UnsupportedLegacySchema { .. } => FailureCode::UnsupportedLegacySchema,
            DocumentFailure::InvalidProjectDocument => FailureCode::InvalidProjectDocument,
            DocumentFailure::InvalidProjectState => FailureCode::InvalidProjectState,
        },
        OpenProjectError::ProjectInUse => FailureCode::ProjectInUse,
        OpenProjectError::ExternalCopyRequiresInteractiveResolution => {
            FailureCode::ExternalCopyRequiresInteractiveResolution
        }
        OpenProjectError::IdentityIndeterminate => FailureCode::IdentityIndeterminate,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        thread,
    };

    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, NativePathDto, OperationPathContext};

    use super::*;

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
            let paths = AppPaths::from_roots(root.path(), root.path(), root.path());
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
            let core = ProjectCore::new()
                .with_identity_lease_root(self.paths.project_identity_leases_dir());
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
            open_host_project_with_thread(fixture.request(), &fixture.paths)
                .expect("the Host opens the v1 document");

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
    fn a_second_host_receives_project_in_use_without_disturbing_the_owner() {
        let fixture = Fixture::new();
        fixture.create_project();
        let owner = open_host_project(fixture.request(), &fixture.paths)
            .expect("the first Host owns the Project");

        let failure = open_host_project(fixture.request(), &fixture.paths)
            .expect_err("a second editable Host is rejected");

        assert_eq!(
            failure,
            HostTerminal::Failed {
                attempt_id: "attempt-open".into(),
                launch_nonce: "nonce-open".into(),
                host_pid: std::process::id(),
                stage: FailureStage::Open,
                code: FailureCode::ProjectInUse,
            }
        );
        assert_eq!(owner.project().revision(), 0);
        drop(owner);
        assert!(open_host_project(fixture.request(), &fixture.paths).is_ok());
    }

    #[test]
    fn invalid_documents_fail_structurally_without_leaving_a_session() {
        let fixture = Fixture::new();
        std::fs::write(&fixture.project_path, b"not a project")
            .expect("the invalid fixture is writable");

        let failure = open_host_project(fixture.request(), &fixture.paths)
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
            let opened = open_host_project(
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

        let opened = open_host_project(fixture.request(), &fixture.paths)
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
            open_host_project(missing.request(), &missing.paths),
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
            open_host_project(directory.request(), &directory.paths),
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

        let failure = open_host_project(
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
