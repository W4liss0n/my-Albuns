//! Recreate failed presentation controls without replacing their native window or Host.
use std::{
    collections::{HashMap, VecDeque},
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl,
    webview::{PageLoadEvent, WebviewBuilder},
};
use webview2_com::{
    BrowserProcessExitedEventHandler, CoTaskMemPWSTR,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_REASON,
        ICoreWebView2Environment5, ICoreWebView2Environment7, ICoreWebView2Environment11,
        ICoreWebView2ProcessFailedEventArgs2,
    },
    ProcessFailedEventHandler, SourceChangedEventHandler,
};
use windows::core::{Interface, PWSTR};

const RECOVERY_TIMEOUT: Duration = Duration::from_secs(15);
const RETRY_WINDOW: Duration = Duration::from_secs(60);
const MAX_RECOVERIES: usize = 2;
static WINDOWS: OnceLock<Mutex<HashMap<String, RecoveryState>>> = OnceLock::new();
static NEXT_CONTROLLER: AtomicUsize = AtomicUsize::new(1);

#[derive(Default)]
struct RecoveryGuard {
    controller: usize,
    serial: u64,
    active: Option<u64>,
    attempts: VecDeque<Instant>,
}

impl RecoveryGuard {
    fn admit(&mut self, controller: usize, now: Instant) -> Option<u64> {
        if controller != self.controller || self.active.is_some() {
            return None;
        }
        while self
            .attempts
            .front()
            .is_some_and(|at| now.duration_since(*at) >= RETRY_WINDOW)
        {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= MAX_RECOVERIES {
            return None;
        }
        self.serial += 1;
        self.active = Some(self.serial);
        self.attempts.push_back(now);
        self.active
    }
}

struct RecoveryState {
    native_window: usize,
    window_generation: usize,
    guard: RecoveryGuard,
    url: tauri::Url,
    data_directory: PathBuf,
    browser_arguments: String,
    browser_exit: Option<Arc<AtomicBool>>,
}

fn windows() -> &'static Mutex<HashMap<String, RecoveryState>> {
    WINDOWS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn forget(label: &str) {
    if let Ok(mut states) = windows().lock() {
        states.remove(label);
    }
}

fn com_string(
    read: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>,
) -> windows::core::Result<String> {
    let mut value = PWSTR::null();
    let result = read(&mut value);
    let owned = CoTaskMemPWSTR::from(value);
    result?;
    Ok(owned.to_string())
}

pub(crate) fn install(
    native: &tauri::webview::PlatformWebview,
    app: AppHandle,
    label: String,
    native_window: usize,
    url: tauri::Url,
    arguments: String,
) -> windows::core::Result<()> {
    unsafe {
        let controller = native.controller();
        // COM allocation addresses can be reused immediately after Close.
        let controller_id = NEXT_CONTROLLER.fetch_add(1, Ordering::Relaxed);
        let core = controller.CoreWebView2()?;
        let environment = native.environment();
        let mut browser_pid = 0;
        core.BrowserProcessId(&mut browser_pid)?;
        let version = com_string(|out| environment.BrowserVersionString(out))?;
        let data_directory = com_string(|out| {
            environment
                .cast::<ICoreWebView2Environment7>()?
                .UserDataFolder(out)
        })?;
        let report_directory = com_string(|out| {
            environment
                .cast::<ICoreWebView2Environment11>()?
                .FailureReportFolderPath(out)
        })
        .unwrap_or_default();
        {
            let mut states = windows()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let state = states
                .entry(label.clone())
                .or_insert_with(|| RecoveryState {
                    native_window,
                    window_generation: controller_id,
                    guard: RecoveryGuard::default(),
                    url: url.clone(),
                    data_directory: PathBuf::from(&data_directory),
                    browser_arguments: arguments.clone(),
                    browser_exit: None,
                });
            if state.native_window != native_window {
                state.guard = RecoveryGuard::default();
                state.window_generation = controller_id;
            }
            state.native_window = native_window;
            state.guard.controller = controller_id;
            state.url = url;
            state.data_directory = PathBuf::from(data_directory);
            state.browser_arguments = arguments;
            state.browser_exit = None;
        }
        let source_label = label.clone();
        let mut token = 0;
        let browser_exited = Arc::new(AtomicBool::new(false));
        let exited = browser_exited.clone();
        environment.cast::<ICoreWebView2Environment5>()?.add_BrowserProcessExited(
            &BrowserProcessExitedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut exited_pid = 0;
                    args.BrowserProcessId(&mut exited_pid)?;
                    if exited_pid == browser_pid {
                        exited.store(true, Ordering::Release);
                        tracing::info!(target: "myalbuns.desktop", event = "webview_browser_released", browser_process_id = browser_pid);
                    }
                }
                Ok(())
            })), &mut token,
        )?;
        core.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |source, _| {
                if let Some(source) = source {
                    let url = com_string(|out| source.Source(out))?;
                    if let Ok(url) = tauri::Url::parse(&url)
                        && let Ok(mut states) = windows().lock()
                        && let Some(state) = states.get_mut(&source_label)
                        && state.guard.controller == controller_id
                    {
                        state.url = url;
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        tracing::info!(target: "myalbuns.desktop", event = "webview_diagnostics_ready",
            webview_label = label, browser_process_id = browser_pid,
            runtime_version = version, failure_report_directory = report_directory);
        core.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                args.ProcessFailedKind(&mut kind)?;
                let args = args.cast::<ICoreWebView2ProcessFailedEventArgs2>()?;
                let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
                let mut code = 0;
                args.Reason(&mut reason)?;
                args.ExitCode(&mut code)?;
                tracing::error!(target: "myalbuns.desktop", event = "webview_process_failed",
                webview_label = label, browser_process_id = browser_pid,
                failure_kind = kind.0, failure_reason = reason.0, exit_code = code,
                runtime_version = version, failure_report_directory = report_directory);
                // GPU and utility failures recover within WebView2. A failed browser or
                // main renderer requires replacing the presentation owned by this Host.
                if matches!(kind.0, 0 | 1 | 2) {
                    request_recovery(
                        app.clone(),
                        label.clone(),
                        native_window,
                        controller_id,
                        (kind.0 == 0).then(|| browser_exited.clone()),
                    );
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

fn request_recovery(
    app: AppHandle,
    label: String,
    native_window: usize,
    controller: usize,
    browser_exit: Option<Arc<AtomicBool>>,
) {
    let admission = windows().lock().ok().and_then(|mut states| {
        let state = states.get_mut(&label)?;
        if state.native_window != native_window
            || state.guard.controller != controller
            || state.guard.active.is_some()
        {
            return None;
        }
        if browser_exit.is_some() {
            state.browser_exit = browser_exit;
        }
        let attempt = state.guard.admit(controller, Instant::now());
        if attempt.is_none() {
            state.guard.active = Some(u64::MAX);
        }
        Some((attempt, state.window_generation))
    });
    let Some((admission, generation)) = admission else {
        return;
    };
    let Some(attempt) = admission else {
        tracing::error!(target: "myalbuns.desktop", event = "webview_recovery_exhausted", webview_label = label);
        tauri::async_runtime::spawn_blocking(move || show_failure(&app, &label, native_window));
        return;
    };
    // Never rebuild synchronously from the COM callback or hold the dispatcher
    // lock while waiting for a new controller's page-load callback.
    tauri::async_runtime::spawn_blocking(move || {
        let result = recover(&app, &label, native_window, generation, controller, attempt);
        let current = windows()
            .lock()
            .ok()
            .and_then(|mut states| {
                let state = states.get_mut(&label)?;
                if state.native_window != native_window
                    || state.window_generation != generation
                    || state.guard.active != Some(attempt)
                {
                    return None;
                }
                state.guard.active = None;
                Some(())
            })
            .is_some();
        if let Err(error) = result {
            tracing::error!(target: "myalbuns.desktop", event = "webview_recovery_failed", webview_label = label, error = %error);
            if current {
                show_failure(&app, &label, native_window);
            }
        }
    });
}

fn recover(
    app: &AppHandle,
    label: &str,
    native_window: usize,
    generation: usize,
    controller: usize,
    attempt: u64,
) -> io::Result<()> {
    let Some(window) = app.get_window(label) else {
        return Ok(());
    };
    if window.hwnd().map_err(io::Error::other)?.0 as usize != native_window {
        return Ok(());
    }
    // Drain before reserving Save As authority: an already accepted Save As
    // may still need that reservation after its native file picker returns.
    let _ui_recovery = if label == crate::product_runtime::PROJECT_WINDOW_LABEL {
        let retirement = app
            .state::<crate::project_ui_operations::ProjectUiOperations>()
            .recover()
            .map_err(io::Error::other)?;
        let deadline = Instant::now() + RECOVERY_TIMEOUT;
        while !retirement.is_drained() {
            app.state::<crate::export_attempts::ExportAttempts>()
                .cancel_window_for_recovery(label);
            if Instant::now() >= deadline {
                return Err(io::Error::other(
                    "A operação anterior ainda não terminou. Aguarde e tente novamente.",
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Some(retirement)
    } else {
        None
    };
    let _authority_reservation = if label == crate::product_runtime::PROJECT_WINDOW_LABEL {
        if let Some(authority) =
            app.try_state::<crate::project_webview_authority::ProjectWebviewAuthority>()
        {
            let deadline = Instant::now() + RECOVERY_TIMEOUT;
            Some(loop {
                if let Some(reservation) = authority.try_reserve_recovery() {
                    break reservation;
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::other(
                        "A transição da janela do Projeto não terminou.",
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            })
        } else {
            None
        }
    } else {
        None
    };
    let (mut url, directory, arguments, browser_exit) = {
        let states = windows()
            .lock()
            .map_err(|_| io::Error::other("WebView recovery state unavailable"))?;
        let Some(state) = states.get(label) else {
            return Ok(());
        };
        if state.native_window != native_window
            || state.window_generation != generation
            || state.guard.controller != controller
            || state.guard.active != Some(attempt)
        {
            return Ok(());
        }
        (
            crate::native_dialog_window::recovery_url(label, state.url.clone()),
            state.data_directory.clone(),
            state.browser_arguments.clone(),
            state.browser_exit.clone(),
        )
    };
    // ProcessFailed can precede release of the failed browser's child processes.
    // Keep its controller/environment alive until BrowserProcessExited arrives.
    if let Some(exited) = browser_exit {
        let deadline = Instant::now() + RECOVERY_TIMEOUT;
        while !exited.load(Ordering::Acquire) {
            if Instant::now() >= deadline {
                return Err(io::Error::other(
                    "O navegador anterior não terminou de encerrar.",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let still_current = windows().lock().ok().is_some_and(|states| {
        states.get(label).is_some_and(|state| {
            state.native_window == native_window
                && state.window_generation == generation
                && state.guard.controller == controller
                && state.guard.active == Some(attempt)
        })
    });
    if !still_current {
        return Ok(());
    }
    if label == crate::product_runtime::PROJECT_WINDOW_LABEL {
        // Only an unanswered close confirmation is cancelled. A close/save
        // already approved has finished during the drain above.
        let _ = app
            .state::<crate::project_host::ProjectHost>()
            .cancel_close();
        crate::project_dialog_window::retire_editor_dialog(app).map_err(io::Error::other)?;
    }
    if label == crate::project_dialog_window::PROJECT_DIALOG_LABEL {
        // Reused export dialogs keep their original URL while the Host advances
        // to progress/results. Bootstrap from today's presentation, not that URL.
        url = app
            .state::<crate::project_dialog_window::ProjectDialogPresentationStore>()
            .recovery_url(url)
            .map_err(io::Error::other)?;
    }
    let size = window.inner_size().map_err(io::Error::other)?;
    let _ = window.set_background_color(Some(tauri::window::Color(251, 250, 248, 255)));
    if let Some(webview) = app.get_webview(label) {
        webview.close().map_err(io::Error::other)?;
    }
    if app.get_window(label).is_none() {
        return Ok(());
    }
    tracing::info!(target: "myalbuns.desktop", event = "webview_recovery_started", webview_label = label, attempt);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let sender = Mutex::new(Some(sender));
    let callback_arguments = arguments.clone();
    let builder = WebviewBuilder::new(label, WebviewUrl::External(url))
        .data_directory(directory)
        .additional_browser_args(&arguments)
        .auto_resize()
        .focused(false)
        .devtools(false)
        .zoom_hotkeys_enabled(false)
        .background_color(tauri::window::Color(251, 250, 248, 255))
        .on_page_load(move |webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && let Some(sender) = sender.lock().ok().and_then(|mut pending| pending.take())
            {
                let result = crate::desktop_webview_policy::enforce_webview_with_arguments(
                    &webview,
                    callback_arguments.clone(),
                );
                let _ = sender.send(result);
            }
        });
    window
        .add_child(builder, PhysicalPosition::new(0, 0), size)
        .map_err(io::Error::other)?;
    receiver
        .recv_timeout(RECOVERY_TIMEOUT)
        .map_err(|_| io::Error::other("A interface não respondeu durante a recuperação."))??;
    if window.is_focused().unwrap_or(false)
        && let Some(webview) = app.get_webview(label)
    {
        let _ = webview.set_focus();
    }
    tracing::info!(target: "myalbuns.desktop", event = "webview_recovery_ready", webview_label = label, attempt);
    Ok(())
}

fn show_failure(app: &AppHandle, label: &str, native_window: usize) {
    let Some(window) = app.get_window(label) else {
        return;
    };
    if window
        .hwnd()
        .ok()
        .is_none_or(|hwnd| hwnd.0 as usize != native_window)
    {
        return;
    }
    let app = app.clone();
    let label = label.to_owned();
    let _ = window.run_on_main_thread(move || {
        use windows::{core::w, Win32::{Foundation::HWND, UI::WindowsAndMessaging::{IsWindow, MessageBoxW, MB_ICONERROR, MB_RETRYCANCEL, IDRETRY}}};
        let hwnd = HWND(native_window as *mut _);
        let retry = unsafe {
            IsWindow(Some(hwnd)).as_bool() &&
                MessageBoxW(Some(hwnd), w!("Não foi possível restaurar a interface. Tentar novamente? A sessão permanece no programa."), w!("Falha na interface"), MB_RETRYCANCEL | MB_ICONERROR) == IDRETRY
        };
        if retry {
            let controller = windows().lock().ok().and_then(|mut states| {
                let state = states.get_mut(&label)?;
                if state.native_window != native_window { return None; }
                state.guard.active = None;
                state.guard.attempts.clear();
                Some(state.guard.controller)
            });
            if let Some(controller) = controller { request_recovery(app, label, native_window, controller, None); }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_and_retired_controllers_cannot_start_concurrent_recovery() {
        let mut state = RecoveryGuard {
            controller: 12,
            ..Default::default()
        };
        let now = Instant::now();
        assert_eq!(state.admit(11, now), None);
        assert_eq!(state.admit(12, now), Some(1));
        assert_eq!(state.admit(12, now), None);
        state.controller = 13;
        state.active = None;
        assert_eq!(state.admit(12, now), None);
        assert_eq!(state.admit(13, now), Some(2));
    }
    #[test]
    fn repeated_failures_are_bounded_and_a_later_independent_failure_can_recover() {
        let mut state = RecoveryGuard {
            controller: 7,
            ..Default::default()
        };
        let now = Instant::now();
        for serial in 1..=2 {
            assert_eq!(state.admit(7, now), Some(serial));
            state.active = None;
        }
        assert_eq!(state.admit(7, now), None);
        assert_eq!(state.admit(7, now + RETRY_WINDOW), Some(3));
    }
}
