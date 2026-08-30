use std::{env, path::PathBuf};

use myalbuns_desktop_lib::ipc_contract::{
    ApplicationSettings, CacheClearAllOutcome, CacheFreeResult, CacheProcessorState,
    CacheProcessorWarning, CacheServiceCommandError, CacheServiceStatus, CancelDisposition,
    ExportCommandError, ExportEvent, ExportResult, FrontendLogEvent, ImportPhotoResult,
    LinkedMediaChanged, MediaPanelSettings, MediaPanelTabSettings, MediaPreferenceKind,
    MediaPreview, MediaPreviewCommandError, MediaPreviewDemand, MediaPreviewState,
    MediaSortDirection, MediaThumbnailSizes, MediaUsageFilter, ProjectCloseChoice,
    ProjectCloseRequestOutcome, ProjectCloseResolution, ProjectDialogAction,
    ProjectDialogActionEvent, ProjectDialogPresentation, ProjectDialogState,
    ProjectRecoveryDecision, SaveAsProjectCommandError, SaveAsProjectOutcome, SaveAsProjectResult,
    SaveProjectCommandError, SaveProjectOutcome, SaveProjectResult, SettingsPreferenceChange,
    WorkspacePanelKind, WorkspacePanelPreference, WorkspacePanelPreferences,
    WorkspacePreferenceChange, WorkspacePreferences,
};
use ts_rs::{Config, TS};

fn main() {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("src/platform/generated"));
    let config = Config::new()
        .with_out_dir(output_dir)
        .with_large_int("number");

    CancelDisposition::export_all(&config)
        .expect("cancel disposition bindings should be generated");
    CacheClearAllOutcome::export_all(&config)
        .expect("Cache clear outcome bindings should be generated");
    CacheFreeResult::export_all(&config).expect("Cache free result bindings should be generated");
    CacheProcessorState::export_all(&config)
        .expect("Cache processor state bindings should be generated");
    CacheProcessorWarning::export_all(&config)
        .expect("Cache processor warning bindings should be generated");
    CacheServiceCommandError::export_all(&config)
        .expect("Cache service error bindings should be generated");
    CacheServiceStatus::export_all(&config)
        .expect("Cache service status bindings should be generated");
    ExportCommandError::export_all(&config).expect("export error bindings should be generated");
    ExportEvent::export_all(&config).expect("export event bindings should be generated");
    ExportResult::export_all(&config).expect("export result bindings should be generated");
    FrontendLogEvent::export_all(&config).expect("frontend log bindings should be generated");
    LinkedMediaChanged::export_all(&config)
        .expect("linked media change bindings should be generated");
    ApplicationSettings::export_all(&config)
        .expect("application settings bindings should be generated");
    MediaPanelSettings::export_all(&config)
        .expect("media panel settings bindings should be generated");
    MediaPanelTabSettings::export_all(&config)
        .expect("media panel tab settings bindings should be generated");
    ImportPhotoResult::export_all(&config).expect("Photo import bindings should be generated");
    MediaPreview::export_all(&config).expect("media preview bindings should be generated");
    MediaPreviewDemand::export_all(&config)
        .expect("media preview demand bindings should be generated");
    MediaPreviewState::export_all(&config)
        .expect("media preview state bindings should be generated");
    MediaPreviewCommandError::export_all(&config)
        .expect("media preview error bindings should be generated");
    MediaPreferenceKind::export_all(&config)
        .expect("media preference kind bindings should be generated");
    MediaSortDirection::export_all(&config)
        .expect("media sort direction bindings should be generated");
    MediaThumbnailSizes::export_all(&config)
        .expect("media thumbnail size bindings should be generated");
    MediaUsageFilter::export_all(&config).expect("media usage filter bindings should be generated");
    ProjectDialogAction::export_all(&config)
        .expect("Project dialog action bindings should be generated");
    ProjectDialogActionEvent::export_all(&config)
        .expect("owned Project dialog action bindings should be generated");
    ProjectDialogPresentation::export_all(&config)
        .expect("owned Project dialog presentation bindings should be generated");
    ProjectDialogState::export_all(&config)
        .expect("Project dialog state bindings should be generated");
    ProjectCloseChoice::export_all(&config)
        .expect("Project close choice bindings should be generated");
    ProjectCloseRequestOutcome::export_all(&config)
        .expect("Project close request outcome bindings should be generated");
    ProjectCloseResolution::export_all(&config)
        .expect("Project close resolution bindings should be generated");
    ProjectRecoveryDecision::export_all(&config)
        .expect("Project Recovery decision bindings should be generated");
    SaveProjectCommandError::export_all(&config)
        .expect("Save Project error bindings should be generated");
    SaveProjectOutcome::export_all(&config)
        .expect("Save Project outcome bindings should be generated");
    SaveProjectResult::export_all(&config)
        .expect("Save Project result bindings should be generated");
    SettingsPreferenceChange::export_all(&config)
        .expect("settings preference change bindings should be generated");
    WorkspacePanelKind::export_all(&config)
        .expect("workspace panel kind bindings should be generated");
    WorkspacePanelPreference::export_all(&config)
        .expect("workspace panel preference bindings should be generated");
    WorkspacePanelPreferences::export_all(&config)
        .expect("workspace panel preferences bindings should be generated");
    WorkspacePreferenceChange::export_all(&config)
        .expect("workspace preference change bindings should be generated");
    WorkspacePreferences::export_all(&config)
        .expect("workspace preference bindings should be generated");
    SaveAsProjectCommandError::export_all(&config)
        .expect("Save As Project error bindings should be generated");
    SaveAsProjectOutcome::export_all(&config)
        .expect("Save As Project outcome bindings should be generated");
    SaveAsProjectResult::export_all(&config)
        .expect("Save As Project result bindings should be generated");
}
