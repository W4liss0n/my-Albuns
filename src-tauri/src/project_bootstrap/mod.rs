mod configuration;
mod host;
mod protocol;
mod supervisor;

pub(crate) use configuration::{
    InitialBackground, InitialBackgroundContent, InitialDocumentConfiguration, InitialFrameBorder,
    InitialOverlay, InitialOverlayContent, InitialProjectConfiguration,
    InitialProjectCreationConfiguration, InitialStructureConfiguration, InitialVisualDefaults,
    ProjectConfigurationValidation, to_core_initial_project, validate_configuration,
};
#[cfg(test)]
pub(crate) use configuration::{InitialDisplayUnit, InitialSheetFormat};
#[cfg(any(debug_assertions, test))]
pub(crate) use host::bootstrap_host_project;
pub(crate) use host::{BootstrappedHostProject, HostBootstrap, bootstrap_host_project_or_pending};
pub(crate) use protocol::{
    BootstrapIntent, BootstrapRequest, CreateWriteAuthorization, FailureCode, FailureStage,
    HostTerminal, SaveExternalCopyRequest, TargetAuthority, TerminalValidationError,
    ValidatedTerminal, read_bootstrap_request, read_save_external_copy_request, validate_terminal,
    write_host_terminal,
};
#[cfg(debug_assertions)]
pub(crate) use supervisor::new_open_request;
pub(crate) use supervisor::{
    BootstrapFailure, BootstrapFailureKind, BootstrapOutcome, PendingExternalCopyProcess,
    ProjectHostBootstrap,
};
