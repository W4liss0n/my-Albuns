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
pub(crate) use host::{BootstrappedHostProject, bootstrap_host_project};
pub(crate) use protocol::{
    BootstrapIntent, BootstrapRequest, CreateWriteAuthorization, FailureCode, FailureStage,
    HostTerminal, TargetAuthority, TerminalValidationError, ValidatedTerminal,
    read_bootstrap_request, validate_terminal, write_host_terminal,
};
#[cfg(debug_assertions)]
pub(crate) use supervisor::new_open_request;
pub(crate) use supervisor::{BootstrapFailure, BootstrapFailureKind, ProjectHostBootstrap};
