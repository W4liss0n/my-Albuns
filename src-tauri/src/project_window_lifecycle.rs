use std::{
    io,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

use myalbuns_logging::ProcessRole;
use tauri::{Manager, Window};

use crate::export_attempts::ExportAttempts;

pub(crate) const PROJECT_CLOSE_CONFIRMATION_EVENT: &str =
    "myalbuns://project-close-confirmation-requested";
static CLOSE_COMPLETION_STARTED: AtomicBool = AtomicBool::new(false);

fn global_entry_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

fn launch_clean_global_entry() -> io::Result<()> {
    let executable = std::env::current_exe()?;
    global_entry_command(&executable).spawn().map(|_| ())
}

pub(crate) fn request_window_export_cancellation(window: &Window) -> ExportAttempts {
    let attempts = (*window.state::<ExportAttempts>()).clone();
    let cancelled_attempts = attempts.begin_window_close(window.label());
    if cancelled_attempts > 0 {
        tracing::info!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            window_label = window.label(),
            cancelled_attempts,
            event = "window_export_attempts_cancelled",
        );
    }
    attempts
}

/// Ends a consumed Project Host and starts a fresh global entry point.
///
/// No Project path or creative state crosses this process boundary. The
/// caller must consume the EditableProject before reaching this function.
pub(crate) fn complete_project_close(window: &Window) {
    if CLOSE_COMPLETION_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    let attempts = request_window_export_cancellation(window);
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        attempts.wait_for_window_to_finish(window.label()).await;

        if let Err(error) = launch_clean_global_entry() {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                error = %error,
                event = "global_entry_relaunch_failed",
            );
        }

        if let Err(error) = window.destroy() {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                error = %error,
                event = "project_window_destroy_failed",
            );
        }
        window.app_handle().exit(0);
    });
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::global_entry_command;
    use crate::runtime_role::{RuntimeRole, parse_runtime_role};

    #[test]
    fn the_replacement_global_process_receives_no_project_or_host_state() {
        let executable = PathBuf::from(r"C:\Program Files\MyAlbuns\MyAlbuns.exe");
        let command = global_entry_command(&executable);
        let arguments = std::iter::once(command.get_program().to_owned())
            .chain(command.get_args().map(OsString::from))
            .collect::<Vec<_>>();

        assert_eq!(arguments, vec![executable.into_os_string()]);
        assert_eq!(
            parse_runtime_role(arguments),
            RuntimeRole::Global {
                direct_project: None
            }
        );
    }
}
