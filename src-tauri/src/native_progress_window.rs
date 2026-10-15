//! The progress of openings, creation and image processing as a native
//! window. It is drawn with Direct2D in software, so it appears before any
//! WebView2 browser starts and has no page that could miss its load.
//! See `docs/research/2026-10-13-progresso-de-abertura-em-pc-fraco.md`.
//!
//! The window repeats the shared progress dialog of the WebView pages: the
//! same title bar, margins, tokens and wording.

use std::io;
#[cfg(windows)]
use std::sync::{Mutex, PoisonError};

use crate::ipc_contract::{OpeningProgress, OpeningProjectProgress, OpeningProjectState};

/// What the window reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProgressSubject {
    Opening,
    Creating,
    ProcessingImages,
}

impl ProgressSubject {
    fn title(self) -> &'static str {
        match self {
            Self::Opening => "Abrindo projeto",
            Self::Creating => "Criando projeto",
            Self::ProcessingImages => "Processando imagens",
        }
    }
}

/// What the window shows. Mirrors `openingProgressView` of the WebView page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProgressView {
    /// No count yet: a moving bar under the status.
    Waiting { title: String, status: &'static str },
    Measured {
        title: String,
        status: Option<&'static str>,
        completed: u32,
        total: u32,
    },
    /// Several Projects opened together, one row each.
    Projects {
        title: String,
        rows: Vec<ProjectRowView>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectRowView {
    name: String,
    status: String,
    failed: bool,
    /// Completed and total photos while the Project prepares them.
    bar: Option<(u32, u32)>,
}

pub(crate) fn progress_view(
    subject: ProgressSubject,
    progress: Option<&OpeningProgress>,
) -> ProgressView {
    let fallback = || match subject {
        ProgressSubject::ProcessingImages => ProgressView::Measured {
            title: subject.title().to_owned(),
            status: None,
            completed: 0,
            total: 1,
        },
        ProgressSubject::Opening | ProgressSubject::Creating => ProgressView::Waiting {
            title: subject.title().to_owned(),
            status: "Preparando a Janela do projeto…",
        },
    };
    let Some(progress) = progress else {
        return fallback();
    };
    if progress.projects.len() > 1 {
        return ProgressView::Projects {
            title: format!("Abrindo {} projetos", progress.projects.len()),
            rows: progress.projects.iter().map(project_row).collect(),
        };
    }
    match progress.projects.first() {
        Some(project)
            if project.total_files > 0 && project.state != OpeningProjectState::Deciding =>
        {
            ProgressView::Measured {
                title: subject.title().to_owned(),
                status: Some("Preparando imagens"),
                completed: project.completed_files.min(project.total_files),
                total: project.total_files,
            }
        }
        _ => fallback(),
    }
}

fn project_row(project: &OpeningProjectProgress) -> ProjectRowView {
    let total = project.total_files;
    let completed = project.completed_files.min(total);
    let status = match project.state {
        OpeningProjectState::Starting => "Abrindo…".to_owned(),
        OpeningProjectState::Preparing => format!(
            "{completed} de {total} {}",
            if total == 1 { "foto" } else { "fotos" }
        ),
        OpeningProjectState::Deciding => "Aguardando sua escolha".to_owned(),
        OpeningProjectState::Ready => "Pronto".to_owned(),
        OpeningProjectState::Failed => "Não abriu".to_owned(),
        OpeningProjectState::Cancelled => "Cancelado".to_owned(),
    };
    ProjectRowView {
        name: project.name.clone(),
        status,
        failed: project.state == OpeningProjectState::Failed,
        bar: (project.state == OpeningProjectState::Preparing && total > 0)
            .then_some((completed, total)),
    }
}

/// The last line of a list taller than the screen, in place of the rows that
/// do not fit.
fn more_projects(hidden: usize) -> String {
    if hidden == 1 {
        "Mais 1 projeto".to_owned()
    } else {
        format!("Mais {hidden} projetos")
    }
}

/// `Math.round` of the page, for the percentage.
fn percentage(completed: u32, total: u32) -> u32 {
    let total = total.max(1);
    let completed = completed.min(total);
    ((f64::from(completed) / f64::from(total)) * 100.0 + 0.5).floor() as u32
}

/// Measures of the shared progress dialog, in device-independent pixels.
mod layout {
    use super::ProgressView;

    pub(super) const BORDER: f32 = 1.0;
    pub(super) const TITLEBAR: f32 = 36.0;
    pub(super) const MARGIN: f32 = 24.0;
    pub(super) const HEADER_GAP: f32 = 8.0;
    pub(super) const TITLE_SIZE: f32 = 13.0;
    pub(super) const TITLE_LINE: f32 = TITLE_SIZE * 1.35;
    pub(super) const STATUS_SIZE: f32 = 12.5;
    pub(super) const STATUS_LINE: f32 = STATUS_SIZE * 1.5;
    pub(super) const STACK_GAP: f32 = 9.0;
    pub(super) const TRACK: f32 = 5.0;
    pub(super) const META_SIZE: f32 = 11.0;
    pub(super) const META_LINE: f32 = META_SIZE * 1.5;
    pub(super) const ROW_NAME_SIZE: f32 = 12.5;
    pub(super) const ROW_STATUS_SIZE: f32 = 11.0;
    pub(super) const ROW_LINE: f32 = ROW_NAME_SIZE * 1.5;
    pub(super) const ROW_GAP: f32 = 10.0;
    pub(super) const ROW_COLUMN_GAP: f32 = 12.0;
    pub(super) const ROW_BAR: f32 = 88.0;
    const WIDTH: f32 = 380.0;
    const LIST_WIDTH: f32 = 480.0;

    pub(super) fn size(view: &ProgressView) -> (f32, f32) {
        let (width, body) = match view {
            ProgressView::Waiting { .. } => (
                WIDTH,
                STATUS_LINE + STACK_GAP + TRACK + STACK_GAP + META_LINE,
            ),
            ProgressView::Measured { status, .. } => (
                WIDTH,
                status.map_or(0.0, |_| STATUS_LINE + STACK_GAP) + TRACK + STACK_GAP + META_LINE,
            ),
            ProgressView::Projects { rows, .. } => {
                let count = rows.len().max(1) as f32;
                (LIST_WIDTH, count * ROW_LINE + (count - 1.0) * ROW_GAP)
            }
        };
        (width, (CHROME + body).ceil())
    }

    /// Everything around the body: border, title bar, margins and title.
    const CHROME: f32 = BORDER * 2.0 + TITLEBAR + MARGIN + TITLE_LINE + HEADER_GAP + MARGIN;

    /// The size of `view` in a window that cannot grow past `max_height`, the
    /// height of the screen's work area. A shorter list keeps room for one row.
    pub(super) fn size_within(view: &ProgressView, max_height: f32) -> (f32, f32) {
        let (width, height) = size(view);
        let smallest = (CHROME + ROW_LINE).ceil();
        (width, height.min(max_height.floor().max(smallest)))
    }

    /// How many rows of `count` are drawn in a window `height` tall. When some
    /// do not fit, the last line counts them instead of showing one more row.
    pub(super) fn rows_shown(height: f32, count: usize) -> usize {
        let body = height - CHROME + ROW_GAP;
        let lines = ((body + 0.01) / (ROW_LINE + ROW_GAP)).floor().max(1.0) as usize;
        if lines >= count { count } else { lines - 1 }
    }
}

/// The window created before WebView2 for a launch that opens files. The
/// first opening of that launch adopts it; anything that shows the Welcome
/// instead closes it.
#[cfg(windows)]
static EARLY_OPENING: Mutex<Option<ProgressWindow>> = Mutex::new(None);

/// Shows the opening progress right away. Call on the main thread before the
/// first WebView is built.
#[cfg(windows)]
pub(crate) fn show_early_opening() {
    let options = ProgressWindowOptions {
        owner: None,
        near: None,
        in_taskbar: true,
        view: progress_view(ProgressSubject::Opening, None),
    };
    match ProgressWindow::create(options) {
        Ok(window) => {
            window.show(true);
            *EARLY_OPENING.lock().unwrap_or_else(PoisonError::into_inner) = Some(window);
        }
        Err(error) => tracing::warn!(
            target: "myalbuns.desktop",
            error = %error,
            event = "early_opening_progress_unavailable",
        ),
    }
}

#[cfg(not(windows))]
pub(crate) fn show_early_opening() {}

#[cfg(windows)]
pub(crate) fn take_early_opening() -> Option<ProgressWindow> {
    EARLY_OPENING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}

#[cfg(not(windows))]
pub(crate) fn take_early_opening() -> Option<ProgressWindow> {
    None
}

/// The launch shows the Welcome or another window instead of an opening.
pub(crate) fn close_early_opening() {
    if let Some(window) = take_early_opening() {
        window.close();
    }
}

pub(crate) struct ProgressWindowOptions {
    /// The native owner: the window stays above it and has no taskbar entry
    /// of its own.
    pub(crate) owner: Option<isize>,
    /// The window whose monitor centers this one; the cursor's monitor
    /// otherwise.
    pub(crate) near: Option<isize>,
    /// Gives an unowned window its own taskbar entry.
    pub(crate) in_taskbar: bool,
    pub(crate) view: ProgressView,
}

/// Creates the window on the main thread, where the event loop pumps its
/// messages.
pub(crate) async fn create_on_main_thread(
    app: &tauri::AppHandle,
    options: ProgressWindowOptions,
) -> io::Result<ProgressWindow> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(ProgressWindow::create(options));
    })
    .map_err(io::Error::other)?;
    receiver
        .await
        .map_err(|_| io::Error::other("the progress window creation became unavailable"))?
}

#[cfg(windows)]
pub(crate) use platform::ProgressWindow;

#[cfg(not(windows))]
#[derive(Clone)]
pub(crate) struct ProgressWindow;

#[cfg(not(windows))]
impl ProgressWindow {
    pub(crate) fn create(_options: ProgressWindowOptions) -> io::Result<Self> {
        Err(io::Error::other(
            "the native progress window exists only on Windows",
        ))
    }
    pub(crate) fn set_view(&self, _view: ProgressView) {}
    pub(crate) fn show(&self, _activate: bool) {}
    pub(crate) fn hide(&self) {}
    pub(crate) fn set_topmost(&self, _topmost: bool) {}
    pub(crate) fn is_visible(&self) -> bool {
        false
    }
    pub(crate) fn close(&self) {}
}

#[cfg(windows)]
mod platform {
    use std::{
        cell::{Cell, RefCell},
        io,
        sync::{
            Arc, Mutex, Once, PoisonError,
            atomic::{AtomicBool, AtomicIsize, Ordering},
        },
        time::Instant,
    };

    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
            Graphics::{
                Direct2D::{
                    Common::{
                        D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F,
                        D2D1_PIXEL_FORMAT,
                    },
                    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
                    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1CreateFactory,
                    ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget, ID2D1SolidColorBrush,
                },
                DirectWrite::{
                    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_TRIMMING,
                    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
                    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
                },
                Dwm::{
                    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
                    DwmSetWindowAttribute,
                },
                Gdi::{
                    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect,
                    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromRect,
                    MonitorFromWindow, PAINTSTRUCT, ScreenToClient,
                },
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                Controls::MARGINS,
                HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
                WindowsAndMessaging::{
                    CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
                    DestroyWindow, GWLP_USERDATA, GetClientRect, GetCursorPos, GetSystemMetrics,
                    GetWindowLongPtrW, GetWindowRect, HCURSOR, HICON, HTCAPTION, HTCLIENT,
                    HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, IMAGE_ICON, KillTimer,
                    LR_DEFAULTCOLOR, LoadCursorW, LoadImageW, PostMessageW, RegisterClassExW,
                    RegisterWindowMessageW, SM_CXICON, SM_CXSMICON, SPI_GETCLIENTAREAANIMATION,
                    SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE,
                    SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetForegroundWindow, SetTimer,
                    SetWindowLongPtrW, SetWindowPos, ShowWindow, SystemParametersInfoW,
                    WINDOW_EX_STYLE, WM_APP, WM_CLOSE, WM_DPICHANGED, WM_ERASEBKGND, WM_NCACTIVATE,
                    WM_NCCALCSIZE, WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SIZE,
                    WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_EX_APPWINDOW, WS_POPUP,
                },
            },
        },
        core::{BOOL, Interface, PCWSTR, w},
    };

    use super::{
        ProgressView, ProgressWindowOptions, ProjectRowView, layout, more_projects, percentage,
    };

    const CLASS_NAME: PCWSTR = w!("MyAlbunsProgressWindow");
    const WM_PROGRESS_SYNC: u32 = WM_APP + 1;
    const WM_PROGRESS_SHOW: u32 = WM_APP + 2;
    const WM_PROGRESS_HIDE: u32 = WM_APP + 3;
    const WM_PROGRESS_TOPMOST: u32 = WM_APP + 4;
    const WM_PROGRESS_CLOSE: u32 = WM_APP + 5;
    const ANIMATION_TIMER: usize = 1;
    const ANIMATION_INTERVAL_MS: u32 = 16;
    /// `IDI_APPLICATION`, where tauri-build places the application icon.
    const APPLICATION_ICON: u16 = 32512;

    // Tokens of `src/ui/theme.css`.
    const SURFACE: u32 = 0xfbfaf8;
    const SURFACE_RAISED: u32 = 0xfffefc;
    const BORDER_COLOR: u32 = 0xe1ddd4;
    const BORDER_STRONG: u32 = 0xd4cfc4;
    const TEXT: u32 = 0x2c2924;
    const TEXT_SECONDARY: u32 = 0x655f56;
    const TEXT_MUTED: u32 = 0x6d665c;
    const DANGER_TEXT: u32 = 0x7f4a45;
    const ACCENT: u32 = 0x2f7fba;
    const TRACK_COLOR: u32 = 0xe9e5dc;

    /// What other threads change. The window reads it on its own thread.
    struct Shared {
        handle: AtomicIsize,
        view: Mutex<ProgressView>,
        sync_pending: AtomicBool,
        visible: AtomicBool,
        /// WM_NCCREATE took the window state; WM_NCDESTROY frees it.
        adopted: AtomicBool,
    }

    /// The progress window. Clones refer to the same window; every method can
    /// be called from any thread.
    #[derive(Clone)]
    pub(crate) struct ProgressWindow {
        shared: Arc<Shared>,
    }

    /// Owned by the window's thread through `GWLP_USERDATA`.
    struct WindowState {
        shared: Arc<Shared>,
        renderer: RefCell<Option<Renderer>>,
        animation_start: Cell<Instant>,
        taskbar_button_created: u32,
        render_warned: Cell<bool>,
    }

    impl ProgressWindow {
        /// Creates the hidden window. Call on the thread that pumps its
        /// messages: the main thread.
        pub(crate) fn create(options: ProgressWindowOptions) -> io::Result<Self> {
            register_class()?;
            let shared = Arc::new(Shared {
                handle: AtomicIsize::new(0),
                view: Mutex::new(options.view.clone()),
                sync_pending: AtomicBool::new(false),
                visible: AtomicBool::new(false),
                adopted: AtomicBool::new(false),
            });
            let monitor = unsafe {
                match options.near.map(handle_from) {
                    Some(near) => MonitorFromWindow(near, MONITOR_DEFAULTTONEAREST),
                    None => {
                        let mut cursor = POINT::default();
                        let _ = GetCursorPos(&mut cursor);
                        MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
                    }
                }
            };
            let mut dpi_x = 96;
            let mut dpi_y = 96;
            let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
            let area = work_area(monitor);
            let (width, height) = pixel_size(&options.view, dpi_x, &area);
            let state = Box::new(WindowState {
                shared: Arc::clone(&shared),
                renderer: RefCell::new(None),
                animation_start: Cell::new(Instant::now()),
                taskbar_button_created: unsafe {
                    RegisterWindowMessageW(w!("TaskbarButtonCreated"))
                },
                render_warned: Cell::new(false),
            });
            let extended_style = if options.owner.is_none() && options.in_taskbar {
                WS_EX_APPWINDOW
            } else {
                WINDOW_EX_STYLE::default()
            };
            let state_pointer = Box::into_raw(state);
            let created = unsafe {
                CreateWindowExW(
                    extended_style,
                    CLASS_NAME,
                    w!("MyAlbuns"),
                    WS_POPUP | WS_CAPTION,
                    area.left + (area.right - area.left - width) / 2,
                    area.top + (area.bottom - area.top - height) / 2,
                    width,
                    height,
                    options.owner.map(handle_from),
                    None,
                    Some(module_instance()),
                    Some(state_pointer.cast()),
                )
            };
            let handle = match created {
                Ok(handle) => handle,
                Err(error) => {
                    if !shared.adopted.load(Ordering::Acquire) {
                        drop(unsafe { Box::from_raw(state_pointer) });
                    }
                    return Err(io::Error::other(error));
                }
            };
            unsafe {
                // A one-pixel frame keeps the system shadow of a frameless window.
                let margins = MARGINS {
                    cxLeftWidth: 0,
                    cxRightWidth: 0,
                    cyTopHeight: 0,
                    cyBottomHeight: 1,
                };
                let _ = DwmExtendFrameIntoClientArea(handle, &margins);
                let corners = DWMWCP_ROUND;
                let _ = DwmSetWindowAttribute(
                    handle,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&raw const corners).cast(),
                    size_of_val(&corners) as u32,
                );
                // Applies the frameless client area to the created window.
                let _ = SetWindowPos(
                    handle,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOZORDER
                        | SWP_NOACTIVATE
                        | SWP_NOOWNERZORDER,
                );
            }
            let window = Self { shared };
            // The monitor's DPI was an estimate; the window knows its own.
            window.set_view(options.view);
            Ok(window)
        }

        pub(crate) fn set_view(&self, view: ProgressView) {
            *self
                .shared
                .view
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = view;
            if !self.shared.sync_pending.swap(true, Ordering::AcqRel) {
                self.post(WM_PROGRESS_SYNC, 0);
            }
        }

        pub(crate) fn show(&self, activate: bool) {
            self.shared.visible.store(true, Ordering::Release);
            self.post(WM_PROGRESS_SHOW, usize::from(activate));
        }

        pub(crate) fn hide(&self) {
            self.shared.visible.store(false, Ordering::Release);
            self.post(WM_PROGRESS_HIDE, 0);
        }

        pub(crate) fn set_topmost(&self, topmost: bool) {
            self.post(WM_PROGRESS_TOPMOST, usize::from(topmost));
        }

        pub(crate) fn is_visible(&self) -> bool {
            self.shared.visible.load(Ordering::Acquire)
        }

        pub(crate) fn close(&self) {
            self.shared.visible.store(false, Ordering::Release);
            self.post(WM_PROGRESS_CLOSE, 0);
        }

        fn post(&self, message: u32, parameter: usize) {
            let handle = self.shared.handle.load(Ordering::Acquire);
            if handle != 0 {
                let _ = unsafe {
                    PostMessageW(
                        Some(handle_from(handle)),
                        message,
                        WPARAM(parameter),
                        LPARAM(0),
                    )
                };
            }
        }
    }

    fn handle_from(value: isize) -> HWND {
        HWND(value as *mut core::ffi::c_void)
    }

    fn module_instance() -> HINSTANCE {
        unsafe { GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default()
    }

    fn register_class() -> io::Result<()> {
        static REGISTER: Once = Once::new();
        static REGISTERED: AtomicBool = AtomicBool::new(false);
        REGISTER.call_once(|| {
            let instance = module_instance();
            let icon = |size| unsafe {
                LoadImageW(
                    Some(instance),
                    PCWSTR(APPLICATION_ICON as usize as *const u16),
                    IMAGE_ICON,
                    GetSystemMetrics(size),
                    GetSystemMetrics(size),
                    LR_DEFAULTCOLOR,
                )
                .map(|handle| HICON(handle.0))
                .unwrap_or_default()
            };
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_procedure),
                hInstance: instance,
                hIcon: icon(SM_CXICON),
                hIconSm: icon(SM_CXSMICON),
                hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or(HCURSOR::default()),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassExW(&class) };
            REGISTERED.store(atom != 0, Ordering::Release);
        });
        if REGISTERED.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(io::Error::other("the progress window class is unavailable"))
        }
    }

    fn work_area(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> RECT {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            info.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 720,
            }
        }
    }

    fn pixel_size(view: &ProgressView, dpi: u32, area: &RECT) -> (i32, i32) {
        let scale = dpi as f32 / 96.0;
        let (width, height) = layout::size_within(view, (area.bottom - area.top) as f32 / scale);
        (
            (width * scale).ceil() as i32,
            (height * scale).ceil() as i32,
        )
    }

    fn window_state<'a>(handle: HWND) -> Option<&'a WindowState> {
        let pointer = unsafe { GetWindowLongPtrW(handle, GWLP_USERDATA) } as *const WindowState;
        unsafe { pointer.as_ref() }
    }

    fn current_view(state: &WindowState) -> ProgressView {
        state
            .shared
            .view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    unsafe extern "system" fn window_procedure(
        handle: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            let state = create.lpCreateParams.cast::<WindowState>();
            unsafe { SetWindowLongPtrW(handle, GWLP_USERDATA, state as isize) };
            if let Some(state) = unsafe { state.as_ref() } {
                state.shared.adopted.store(true, Ordering::Release);
                state
                    .shared
                    .handle
                    .store(handle.0 as isize, Ordering::Release);
            }
            return unsafe { DefWindowProcW(handle, message, wparam, lparam) };
        }
        let Some(state) = window_state(handle) else {
            return unsafe { DefWindowProcW(handle, message, wparam, lparam) };
        };
        match message {
            // The whole window is client area; the frame only keeps the shadow.
            WM_NCCALCSIZE => LRESULT(0),
            // Keeps the inactive frame from being painted over the content.
            WM_NCACTIVATE => unsafe { DefWindowProcW(handle, message, wparam, LPARAM(-1)) },
            WM_NCHITTEST => hit_test(handle, lparam),
            WM_ERASEBKGND => LRESULT(1),
            // Only the opening ends this window, never Alt+F4.
            WM_CLOSE => LRESULT(0),
            WM_PAINT => {
                paint(handle, state);
                LRESULT(0)
            }
            WM_SIZE => {
                if let Some(renderer) = state.renderer.borrow().as_ref() {
                    renderer.resize(handle);
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                state.renderer.replace(None);
                let suggested = unsafe { &*(lparam.0 as *const RECT) };
                let center_x = (suggested.left + suggested.right) / 2;
                let center_y = (suggested.top + suggested.bottom) / 2;
                let dpi = u32::from(wparam.0 as u16);
                let area =
                    work_area(unsafe { MonitorFromRect(suggested, MONITOR_DEFAULTTONEAREST) });
                let (width, height) = pixel_size(&current_view(state), dpi, &area);
                let _ = unsafe {
                    SetWindowPos(
                        handle,
                        None,
                        center_x - width / 2,
                        center_y - height / 2,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
                    )
                };
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == ANIMATION_TIMER => {
                let _ = unsafe { InvalidateRect(Some(handle), None, false) };
                LRESULT(0)
            }
            WM_PROGRESS_SYNC => {
                state.shared.sync_pending.store(false, Ordering::Release);
                synchronize(handle, state);
                LRESULT(0)
            }
            WM_PROGRESS_SHOW => {
                unsafe {
                    let _ = ShowWindow(
                        handle,
                        if wparam.0 != 0 {
                            SW_SHOWNORMAL
                        } else {
                            SW_SHOWNOACTIVATE
                        },
                    );
                    if wparam.0 != 0 {
                        let _ = SetForegroundWindow(handle);
                    }
                }
                LRESULT(0)
            }
            WM_PROGRESS_HIDE => {
                let _ = unsafe { ShowWindow(handle, SW_HIDE) };
                LRESULT(0)
            }
            WM_PROGRESS_TOPMOST => {
                let _ = unsafe {
                    SetWindowPos(
                        handle,
                        Some(if wparam.0 != 0 {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        }),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                    )
                };
                LRESULT(0)
            }
            WM_PROGRESS_CLOSE => {
                let _ = unsafe { DestroyWindow(handle) };
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let _ = unsafe { KillTimer(Some(handle), ANIMATION_TIMER) };
                unsafe { SetWindowLongPtrW(handle, GWLP_USERDATA, 0) };
                let state = unsafe { Box::from_raw(std::ptr::from_ref(state).cast_mut()) };
                state.shared.handle.store(0, Ordering::Release);
                state.shared.visible.store(false, Ordering::Release);
                drop(state);
                unsafe { DefWindowProcW(handle, message, wparam, lparam) }
            }
            _ if message == state.taskbar_button_created && message != 0 => {
                tracing::info!(target: "myalbuns.desktop", event = "opening_taskbar_button_created");
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(handle, message, wparam, lparam) },
        }
    }

    /// The title bar moves the window; the rest is inert.
    fn hit_test(handle: HWND, lparam: LPARAM) -> LRESULT {
        let mut point = POINT {
            x: i32::from(lparam.0 as i16),
            y: i32::from((lparam.0 >> 16) as i16),
        };
        let _ = unsafe { ScreenToClient(handle, &mut point) };
        let scale = unsafe { GetDpiForWindow(handle) } as f32 / 96.0;
        if (point.y as f32) < (layout::BORDER + layout::TITLEBAR) * scale {
            LRESULT(HTCAPTION as isize)
        } else {
            LRESULT(HTCLIENT as isize)
        }
    }

    /// Applies the latest view: size, animation and a repaint.
    fn synchronize(handle: HWND, state: &WindowState) {
        let view = current_view(state);
        let dpi = unsafe { GetDpiForWindow(handle) };
        let area = work_area(unsafe { MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST) });
        let (width, height) = pixel_size(&view, dpi, &area);
        let mut rect = RECT::default();
        let _ = unsafe { GetWindowRect(handle, &mut rect) };
        if rect.right - rect.left != width || rect.bottom - rect.top != height {
            // Content changes keep the window centered, as the WebView dialogs do.
            let _ = unsafe {
                SetWindowPos(
                    handle,
                    None,
                    area.left + (area.right - area.left - width) / 2,
                    area.top + (area.bottom - area.top - height) / 2,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
                )
            };
        }
        if matches!(view, ProgressView::Waiting { .. }) {
            unsafe { SetTimer(Some(handle), ANIMATION_TIMER, ANIMATION_INTERVAL_MS, None) };
        } else {
            let _ = unsafe { KillTimer(Some(handle), ANIMATION_TIMER) };
        }
        let _ = unsafe { InvalidateRect(Some(handle), None, false) };
    }

    fn paint(handle: HWND, state: &WindowState) {
        let mut paint = PAINTSTRUCT::default();
        unsafe { BeginPaint(handle, &mut paint) };
        if state.renderer.borrow().is_none() {
            match Renderer::new(handle) {
                Ok(renderer) => {
                    state.renderer.replace(Some(renderer));
                }
                Err(error) => {
                    if !state.render_warned.replace(true) {
                        tracing::warn!(
                            target: "myalbuns.desktop",
                            error = %error,
                            event = "native_progress_render_unavailable",
                        );
                    }
                }
            }
        }
        let view = current_view(state);
        let elapsed = state.animation_start.get().elapsed().as_secs_f32();
        let failed = state.renderer.borrow().as_ref().is_some_and(|renderer| {
            let dpi = unsafe { GetDpiForWindow(handle) } as f32;
            renderer.draw(dpi, &view, elapsed).is_err()
        });
        if failed {
            // A lost target is created again on the next paint.
            state.renderer.replace(None);
        }
        let _ = unsafe { EndPaint(handle, &paint) };
    }

    fn color(rgb: u32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: ((rgb >> 16) & 0xff) as f32 / 255.0,
            g: ((rgb >> 8) & 0xff) as f32 / 255.0,
            b: (rgb & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
        D2D_RECT_F {
            left,
            top,
            right,
            bottom,
        }
    }

    struct TextFormats {
        title: IDWriteTextFormat,
        status: IDWriteTextFormat,
        meta: IDWriteTextFormat,
        meta_end: IDWriteTextFormat,
        row_name: IDWriteTextFormat,
        row_status: IDWriteTextFormat,
    }

    pub(super) struct Renderer {
        target: ID2D1RenderTarget,
        window_target: Option<ID2D1HwndRenderTarget>,
        write: IDWriteFactory,
        formats: TextFormats,
        brush: ID2D1SolidColorBrush,
    }

    impl Renderer {
        fn new(handle: HWND) -> windows::core::Result<Self> {
            let factory: ID2D1Factory =
                unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
            let dpi = unsafe { GetDpiForWindow(handle) } as f32;
            let mut client = RECT::default();
            unsafe { GetClientRect(handle, &mut client)? };
            // Software rendering: the window must not depend on the GPU driver.
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: Default::default(),
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let window_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: handle,
                pixelSize: D2D_SIZE_U {
                    width: (client.right - client.left).max(1) as u32,
                    height: (client.bottom - client.top).max(1) as u32,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let window_target =
                unsafe { factory.CreateHwndRenderTarget(&properties, &window_properties)? };
            Self::with_target(window_target.cast()?, Some(window_target))
        }

        pub(super) fn with_target(
            target: ID2D1RenderTarget,
            window_target: Option<ID2D1HwndRenderTarget>,
        ) -> windows::core::Result<Self> {
            let write: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
            let formats = TextFormats::new(&write)?;
            let brush = unsafe { target.CreateSolidColorBrush(&color(TEXT), None)? };
            Ok(Self {
                target,
                window_target,
                write,
                formats,
                brush,
            })
        }

        fn resize(&self, handle: HWND) {
            let mut client = RECT::default();
            if let Some(target) = &self.window_target
                && unsafe { GetClientRect(handle, &mut client) }.is_ok()
            {
                let _ = unsafe {
                    target.Resize(&D2D_SIZE_U {
                        width: (client.right - client.left).max(1) as u32,
                        height: (client.bottom - client.top).max(1) as u32,
                    })
                };
            }
        }

        fn fill(&self, area: D2D_RECT_F, rgb: u32) {
            unsafe {
                self.brush.SetColor(&color(rgb));
                self.target.FillRectangle(&area, &self.brush);
            }
        }

        fn fill_rounded(&self, area: D2D_RECT_F, radius: f32, rgb: u32) {
            unsafe {
                self.brush.SetColor(&color(rgb));
                self.target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: area,
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &self.brush,
                );
            }
        }

        fn text(&self, value: &str, format: &IDWriteTextFormat, area: D2D_RECT_F, rgb: u32) {
            let value = value.encode_utf16().collect::<Vec<_>>();
            unsafe {
                self.brush.SetColor(&color(rgb));
                self.target.DrawText(
                    &value,
                    format,
                    &area,
                    &self.brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }

        fn text_width(&self, value: &str, format: &IDWriteTextFormat) -> f32 {
            let value = value.encode_utf16().collect::<Vec<_>>();
            let mut metrics = DWRITE_TEXT_METRICS::default();
            unsafe { self.write.CreateTextLayout(&value, format, 4096.0, 256.0) }
                .and_then(|text| unsafe { text.GetMetrics(&mut metrics) })
                .map_or(0.0, |()| metrics.widthIncludingTrailingWhitespace.ceil())
        }

        /// Track and indicator of the shared bar. `None` draws the moving bar.
        fn bar(&self, area: D2D_RECT_F, measured: Option<f32>, elapsed: f32) {
            let radius = layout::TRACK / 2.0;
            self.fill_rounded(area, radius, TRACK_COLOR);
            let width = area.right - area.left;
            match measured {
                Some(fraction) if fraction > 0.0 => {
                    let mut indicator = area;
                    indicator.right = area.left + width * fraction.min(1.0);
                    self.fill_rounded(indicator, radius, ACCENT);
                }
                Some(_) => {}
                None => {
                    // `ui-progress-indeterminate`: 30% of the track crossing
                    // from -100% to 340% of its own width.
                    let duration = if reduced_motion() { 3.0 } else { 1.5 };
                    let progress = ease((elapsed % duration) / duration);
                    let segment = width * 0.3;
                    let left = area.left + segment * (-1.0 + 4.4 * progress);
                    unsafe {
                        self.target
                            .PushAxisAlignedClip(&area, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                    }
                    self.fill_rounded(
                        rect(left, area.top, left + segment, area.bottom),
                        radius,
                        ACCENT,
                    );
                    unsafe { self.target.PopAxisAlignedClip() };
                }
            }
        }

        pub(super) fn draw(
            &self,
            dpi: f32,
            view: &ProgressView,
            elapsed: f32,
        ) -> windows::core::Result<()> {
            unsafe { self.target.SetDpi(dpi, dpi) };
            let D2D_SIZE_F { width, height } = unsafe { self.target.GetSize() };
            unsafe {
                self.target.BeginDraw();
                self.target.Clear(Some(&color(SURFACE_RAISED)));
            }
            let inner_left = layout::BORDER;
            let inner_right = width - layout::BORDER;
            // Title bar without text: the dialog's title names the flow below.
            self.fill(
                rect(
                    inner_left,
                    layout::BORDER,
                    inner_right,
                    layout::BORDER + layout::TITLEBAR,
                ),
                SURFACE,
            );
            self.fill(
                rect(
                    inner_left,
                    layout::BORDER + layout::TITLEBAR - 1.0,
                    inner_right,
                    layout::BORDER + layout::TITLEBAR,
                ),
                BORDER_COLOR,
            );
            let left = inner_left + layout::MARGIN;
            let right = inner_right - layout::MARGIN;
            let mut top = layout::BORDER + layout::TITLEBAR + layout::MARGIN;
            let title = match view {
                ProgressView::Waiting { title, .. }
                | ProgressView::Measured { title, .. }
                | ProgressView::Projects { title, .. } => title,
            };
            self.text(
                title,
                &self.formats.title,
                rect(left, top, right, top + layout::TITLE_LINE),
                TEXT,
            );
            top += layout::TITLE_LINE + layout::HEADER_GAP;
            match view {
                ProgressView::Waiting { status, .. } => {
                    self.text(
                        status,
                        &self.formats.status,
                        rect(left, top, right, top + layout::STATUS_LINE),
                        TEXT_SECONDARY,
                    );
                    top += layout::STATUS_LINE + layout::STACK_GAP;
                    self.bar(rect(left, top, right, top + layout::TRACK), None, elapsed);
                }
                ProgressView::Measured {
                    status,
                    completed,
                    total,
                    ..
                } => {
                    if let Some(status) = status {
                        self.text(
                            status,
                            &self.formats.status,
                            rect(left, top, right, top + layout::STATUS_LINE),
                            TEXT_SECONDARY,
                        );
                        top += layout::STATUS_LINE + layout::STACK_GAP;
                    }
                    let percent = percentage(*completed, *total);
                    self.bar(
                        rect(left, top, right, top + layout::TRACK),
                        Some(percent as f32 / 100.0),
                        elapsed,
                    );
                    top += layout::TRACK + layout::STACK_GAP;
                    let meta = rect(left, top, right, top + layout::META_LINE);
                    self.text(&format!("{percent}%"), &self.formats.meta, meta, TEXT_MUTED);
                    self.text(
                        &format!("{} de {total}", (*completed).min(*total)),
                        &self.formats.meta_end,
                        meta,
                        TEXT_MUTED,
                    );
                }
                ProgressView::Projects { rows, .. } => {
                    let shown = layout::rows_shown(height, rows.len());
                    for row in &rows[..shown] {
                        self.project_row(row, left, right, top);
                        top += layout::ROW_LINE + layout::ROW_GAP;
                    }
                    if shown < rows.len() {
                        self.text(
                            &more_projects(rows.len() - shown),
                            &self.formats.row_name,
                            rect(left, top, right, top + layout::ROW_LINE),
                            TEXT_MUTED,
                        );
                    }
                }
            }
            // The shell's border, inside the window.
            let edge = layout::BORDER;
            self.fill(rect(0.0, 0.0, width, edge), BORDER_STRONG);
            self.fill(rect(0.0, height - edge, width, height), BORDER_STRONG);
            self.fill(rect(0.0, 0.0, edge, height), BORDER_STRONG);
            self.fill(rect(width - edge, 0.0, width, height), BORDER_STRONG);
            unsafe { self.target.EndDraw(None, None) }
        }

        /// One row: name, status at its own width, then a fixed bar column so
        /// bars line up from row to row.
        fn project_row(&self, row: &ProjectRowView, left: f32, right: f32, top: f32) {
            let bottom = top + layout::ROW_LINE;
            let bar_left = right - layout::ROW_BAR;
            let status_width = self.text_width(&row.status, &self.formats.row_status);
            let status_left = bar_left - layout::ROW_COLUMN_GAP - status_width;
            self.text(
                &row.name,
                &self.formats.row_name,
                rect(
                    left,
                    top,
                    (status_left - layout::ROW_COLUMN_GAP).max(left),
                    bottom,
                ),
                TEXT,
            );
            self.text(
                &row.status,
                &self.formats.row_status,
                rect(status_left, top, bar_left - layout::ROW_COLUMN_GAP, bottom),
                if row.failed {
                    DANGER_TEXT
                } else {
                    TEXT_SECONDARY
                },
            );
            if let Some((completed, total)) = row.bar {
                let middle = top + layout::ROW_LINE / 2.0;
                self.bar(
                    rect(
                        bar_left,
                        middle - layout::TRACK / 2.0,
                        right,
                        middle + layout::TRACK / 2.0,
                    ),
                    Some(percentage(completed, total) as f32 / 100.0),
                    0.0,
                );
            }
        }
    }

    impl TextFormats {
        fn new(write: &IDWriteFactory) -> windows::core::Result<Self> {
            let interface = interface_family(write);
            let make = |family: PCWSTR,
                        weight: DWRITE_FONT_WEIGHT,
                        size: f32,
                        trailing: bool,
                        ellipsis: bool|
             -> windows::core::Result<IDWriteTextFormat> {
                let format = unsafe {
                    write.CreateTextFormat(
                        family,
                        None,
                        weight,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        size,
                        w!("pt-BR"),
                    )?
                };
                unsafe {
                    format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                    format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                    format.SetTextAlignment(if trailing {
                        DWRITE_TEXT_ALIGNMENT_TRAILING
                    } else {
                        DWRITE_TEXT_ALIGNMENT_LEADING
                    })?;
                    if ellipsis {
                        let sign = write.CreateEllipsisTrimmingSign(&format)?;
                        format.SetTrimming(
                            &DWRITE_TRIMMING {
                                granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                                delimiter: 0,
                                delimiterCount: 0,
                            },
                            &sign,
                        )?;
                    }
                }
                Ok(format)
            };
            let normal = DWRITE_FONT_WEIGHT_NORMAL;
            Ok(Self {
                title: make(
                    interface,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    layout::TITLE_SIZE,
                    false,
                    true,
                )?,
                status: make(interface, normal, layout::STATUS_SIZE, false, true)?,
                meta: make(w!("Consolas"), normal, layout::META_SIZE, false, false)?,
                meta_end: make(w!("Consolas"), normal, layout::META_SIZE, true, false)?,
                row_name: make(interface, normal, layout::ROW_NAME_SIZE, false, true)?,
                row_status: make(interface, normal, layout::ROW_STATUS_SIZE, true, false)?,
            })
        }
    }

    /// The interface font of `theme.css` that this Windows has.
    fn interface_family(write: &IDWriteFactory) -> PCWSTR {
        let mut collection: Option<IDWriteFontCollection> = None;
        let _ = unsafe { write.GetSystemFontCollection(&mut collection, false) };
        let available = |name: PCWSTR| {
            collection.as_ref().is_some_and(|collection| {
                let mut index = 0;
                let mut exists = BOOL(0);
                unsafe { collection.FindFamilyName(name, &mut index, &mut exists) }.is_ok()
                    && exists.as_bool()
            })
        };
        if available(w!("Segoe UI Variable Text")) {
            w!("Segoe UI Variable Text")
        } else {
            w!("Segoe UI")
        }
    }

    fn reduced_motion() -> bool {
        let mut animations = BOOL(1);
        unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&raw mut animations).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok()
            && !animations.as_bool()
    }

    /// `cubic-bezier(0.6, 0.05, 0.35, 0.95)` of the page's animation.
    fn ease(x: f32) -> f32 {
        let (x1, y1, x2, y2) = (0.6_f32, 0.05_f32, 0.35_f32, 0.95_f32);
        let curve = |t: f32, a: f32, b: f32| {
            3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t.powi(2) * b + t.powi(3)
        };
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        for _ in 0..24 {
            let middle = (low + high) / 2.0;
            if curve(middle, x1, x2) < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        curve((low + high) / 2.0, y1, y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(state: OpeningProjectState, completed: u32, total: u32) -> OpeningProjectProgress {
        OpeningProjectProgress {
            name: "SARAH XAVIER".into(),
            state,
            completed_files: completed,
            total_files: total,
        }
    }

    #[test]
    fn one_project_waits_until_its_photos_are_counted() {
        assert_eq!(
            progress_view(ProgressSubject::Opening, None),
            ProgressView::Waiting {
                title: "Abrindo projeto".into(),
                status: "Preparando a Janela do projeto…",
            }
        );
        let starting = OpeningProgress {
            projects: vec![project(OpeningProjectState::Starting, 0, 0)],
        };
        assert!(matches!(
            progress_view(ProgressSubject::Creating, Some(&starting)),
            ProgressView::Waiting { title, .. } if title == "Criando projeto"
        ));
        let preparing = OpeningProgress {
            projects: vec![project(OpeningProjectState::Preparing, 101, 300)],
        };
        assert_eq!(
            progress_view(ProgressSubject::Opening, Some(&preparing)),
            ProgressView::Measured {
                title: "Abrindo projeto".into(),
                status: Some("Preparando imagens"),
                completed: 101,
                total: 300,
            }
        );
    }

    #[test]
    fn a_pending_decision_does_not_show_the_photo_count() {
        let deciding = OpeningProgress {
            projects: vec![project(OpeningProjectState::Deciding, 10, 300)],
        };
        assert!(matches!(
            progress_view(ProgressSubject::Opening, Some(&deciding)),
            ProgressView::Waiting { .. }
        ));
    }

    #[test]
    fn projects_opened_together_are_listed_with_their_own_state() {
        let progress = OpeningProgress {
            projects: vec![
                project(OpeningProjectState::Preparing, 1, 1),
                project(OpeningProjectState::Preparing, 4, 30),
                project(OpeningProjectState::Failed, 0, 0),
            ],
        };
        let ProgressView::Projects { title, rows } =
            progress_view(ProgressSubject::Opening, Some(&progress))
        else {
            panic!("several Projects are listed");
        };
        assert_eq!(title, "Abrindo 3 projetos");
        assert_eq!(rows[0].status, "1 de 1 foto");
        assert_eq!(rows[1].status, "4 de 30 fotos");
        assert_eq!(rows[1].bar, Some((4, 30)));
        assert_eq!(rows[2].status, "Não abriu");
        assert!(rows[2].failed);
        assert_eq!(rows[2].bar, None);
    }

    #[test]
    fn image_processing_keeps_its_fixed_count() {
        assert_eq!(
            progress_view(ProgressSubject::ProcessingImages, None),
            ProgressView::Measured {
                title: "Processando imagens".into(),
                status: None,
                completed: 0,
                total: 1,
            }
        );
    }

    #[test]
    fn percentage_rounds_like_the_page() {
        assert_eq!(percentage(101, 300), 34);
        assert_eq!(percentage(1, 200), 1);
        assert_eq!(percentage(5, 0), 100);
        assert_eq!(percentage(0, 300), 0);
    }

    #[test]
    fn the_window_grows_with_the_list_and_matches_the_dialog_height() {
        let waiting = progress_view(ProgressSubject::Opening, None);
        assert_eq!(layout::size(&waiting), (380.0, 170.0));
        let list = |count| ProgressView::Projects {
            title: String::new(),
            rows: vec![
                ProjectRowView {
                    name: String::new(),
                    status: String::new(),
                    failed: false,
                    bar: None,
                };
                count
            ],
        };
        let (width, two) = layout::size(&list(2));
        assert_eq!(width, 480.0);
        assert!(layout::size(&list(3)).1 > two);
    }

    #[test]
    fn a_list_taller_than_the_screen_counts_the_rows_that_do_not_fit() {
        let list = |count| ProgressView::Projects {
            title: String::new(),
            rows: vec![
                ProjectRowView {
                    name: String::new(),
                    status: String::new(),
                    failed: false,
                    bar: None,
                };
                count
            ],
        };
        let (_, full) = layout::size(&list(3));
        assert_eq!(layout::size_within(&list(3), 2000.0).1, full);
        assert_eq!(layout::rows_shown(full, 3), 3);

        let (_, capped) = layout::size_within(&list(40), 552.0);
        assert_eq!(capped, 552.0);
        let shown = layout::rows_shown(capped, 40);
        assert!(shown > 0 && shown < 40);
        // The shown rows and the summary line fit in the window.
        let (_, needed) = layout::size(&list(shown + 1));
        assert!(needed <= capped);
        assert_eq!(
            more_projects(40 - shown),
            format!("Mais {} projetos", 40 - shown)
        );
        assert_eq!(more_projects(1), "Mais 1 projeto");
    }
}
