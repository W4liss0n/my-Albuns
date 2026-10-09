//! Remembers the size, position and maximized state of the last closed Project
//! window, so the next Project opens the same way.
//!
//! The bounds are the window's restored (not maximized) rectangle as Windows
//! reports it through `GetWindowPlacement`. Writing them back with
//! `SetWindowPlacement` keeps the coordinate space consistent and lets Windows
//! move a window that would fall outside the current monitors.

use std::{fs, io};

use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};

use crate::local_store_io::write_atomically;

const SCHEMA_VERSION: u16 = 1;
const MAX_EXTENT: i32 = 32_768;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectWindowPlacement {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    pub(crate) maximized: bool,
}

impl ProjectWindowPlacement {
    fn is_plausible(&self) -> bool {
        let width = i64::from(self.right) - i64::from(self.left);
        let height = i64::from(self.bottom) - i64::from(self.top);
        [self.left, self.top, self.right, self.bottom]
            .iter()
            .all(|coordinate| coordinate.abs() <= MAX_EXTENT)
            && (1..=i64::from(MAX_EXTENT)).contains(&width)
            && (1..=i64::from(MAX_EXTENT)).contains(&height)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWindowPlacementEnvelope {
    schema_version: u16,
    #[serde(flatten)]
    placement: ProjectWindowPlacement,
}

pub(crate) fn load(app_paths: &AppPaths) -> Option<ProjectWindowPlacement> {
    let bytes = fs::read(app_paths.project_window_placement_file()).ok()?;
    let envelope = serde_json::from_slice::<ProjectWindowPlacementEnvelope>(&bytes).ok()?;
    // Local UI state has no migrations: another version uses the default window.
    (envelope.schema_version == SCHEMA_VERSION && envelope.placement.is_plausible())
        .then_some(envelope.placement)
}

pub(crate) fn save(app_paths: &AppPaths, placement: ProjectWindowPlacement) -> io::Result<()> {
    if !placement.is_plausible() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Project window placement is out of range",
        ));
    }
    let bytes = serde_json::to_vec_pretty(&ProjectWindowPlacementEnvelope {
        schema_version: SCHEMA_VERSION,
        placement,
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_atomically(
        &app_paths.project_window_placement_file(),
        &bytes,
        "project-window.json",
    )
}

#[cfg(windows)]
pub(crate) fn capture(window: &tauri::Window) -> io::Result<ProjectWindowPlacement> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, WINDOWPLACEMENT,
        WPF_RESTORETOMAXIMIZED,
    };

    let handle = window.hwnd().map_err(io::Error::other)?;
    let mut placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: the live Window supplied this HWND and the structure carries its length.
    unsafe { GetWindowPlacement(handle, &mut placement) }.map_err(io::Error::other)?;
    let shown_as = placement.showCmd as i32;
    // A minimized window remembers whether it returns maximized.
    let maximized = shown_as == SW_SHOWMAXIMIZED.0
        || (shown_as == SW_SHOWMINIMIZED.0 && placement.flags.0 & WPF_RESTORETOMAXIMIZED.0 != 0);
    let bounds = placement.rcNormalPosition;
    Ok(ProjectWindowPlacement {
        left: bounds.left,
        top: bounds.top,
        right: bounds.right,
        bottom: bounds.bottom,
        maximized,
    })
}

/// Moves the still hidden Project window to the remembered restored bounds.
/// A window that reopens maximized instead puts its client area exactly on
/// the work area of that monitor while hidden, where the maximized window's
/// client will be, so the page lays out once at its final size and place.
#[cfg(windows)]
pub(crate) fn prepare_hidden(
    window: &tauri::WebviewWindow,
    placement: ProjectWindowPlacement,
) -> io::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowInfo, SW_HIDE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WINDOWINFO,
    };

    set_placement(window, placement, SW_HIDE)?;
    if !placement.maximized {
        return Ok(());
    }
    let Some(monitor) = window.current_monitor().map_err(io::Error::other)? else {
        return Ok(());
    };
    let work_area = monitor.work_area();
    let handle = window.hwnd().map_err(io::Error::other)?;
    let place = |x: i32, y: i32, width: i32, height: i32| {
        // SAFETY: the window was created on this thread and is still hidden.
        unsafe {
            SetWindowPos(
                handle,
                None,
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(io::Error::other)
    };
    let (x, y) = (work_area.position.x, work_area.position.y);
    let (width, height) = (work_area.size.width as i32, work_area.size.height as i32);
    place(x, y, width, height)?;
    // The borderless window keeps an invisible resize frame around its client
    // area. Measure it and grow the window by it.
    let mut info = WINDOWINFO {
        cbSize: size_of::<WINDOWINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: the structure carries its size; the system fills it for this live window.
    unsafe { GetWindowInfo(handle, &mut info) }.map_err(io::Error::other)?;
    let (outer, client) = (info.rcWindow, info.rcClient);
    let (left, top) = (client.left - outer.left, client.top - outer.top);
    let frame_width = (outer.right - outer.left) - (client.right - client.left);
    let frame_height = (outer.bottom - outer.top) - (client.bottom - client.top);
    place(
        x - left,
        y - top,
        width + frame_width,
        height + frame_height,
    )
}

/// Shows the hidden Project window maximized, already final when it appears,
/// and keeps the remembered restored bounds for when it is restored. Call it
/// on the main thread instead of tao's show. A caller that has cloaked the
/// window itself keeps it cloaked.
#[cfg(windows)]
pub(crate) fn show_maximized(
    window: &tauri::WebviewWindow,
    placement: ProjectWindowPlacement,
    cloaked_by_caller: bool,
) -> io::Result<()> {
    use tauri::Manager;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWMAXIMIZED;

    let handle = window.hwnd().map_err(io::Error::other)?;
    // Tao reports every size Windows gives the window on the way, and Tauri
    // would resize the WebViews to each. They keep the size the page laid out
    // with until those reports, delivered after this main-thread task, are past.
    let webviews = window
        .app_handle()
        .get_window(window.label())
        .map(|native| native.webviews())
        .unwrap_or_default();
    for webview in &webviews {
        webview.set_auto_resize(false).map_err(io::Error::other)?;
    }
    // Tao shows a window created without focus with SW_SHOWNOACTIVATE, which
    // restores a maximized one, so it must show the window before Windows
    // maximizes it; and Windows passes through the restored bounds on the way.
    // Cloaked, none of it reaches the screen: the window appears once, final.
    if !cloaked_by_caller {
        crate::opening_focus::set_cloaked(handle, true);
    }
    let shown = window
        .show()
        .map_err(io::Error::other)
        .and_then(|()| set_placement(window, placement, SW_SHOWMAXIMIZED));
    if !cloaked_by_caller {
        crate::opening_focus::set_cloaked(handle, false);
    }
    let resume = window.clone();
    // Posted from another thread: on the main thread Tauri would run it at once.
    std::thread::spawn(move || {
        let follow = resume.clone();
        let _ = resume.run_on_main_thread(move || {
            let Ok(size) = follow.inner_size() else {
                return;
            };
            for webview in webviews {
                let _ = webview.set_size(size);
                let _ = webview.set_auto_resize(true);
            }
        });
    });
    shown
}

#[cfg(windows)]
fn set_placement(
    window: &tauri::WebviewWindow,
    placement: ProjectWindowPlacement,
    show: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD,
) -> io::Result<()> {
    use windows::Win32::{
        Foundation::{POINT, RECT},
        UI::WindowsAndMessaging::{SetWindowPlacement, WINDOWPLACEMENT},
    };

    let handle = window.hwnd().map_err(io::Error::other)?;
    let unset = POINT { x: -1, y: -1 };
    let placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        showCmd: show.0 as u32,
        ptMinPosition: unset,
        ptMaxPosition: unset,
        rcNormalPosition: RECT {
            left: placement.left,
            top: placement.top,
            right: placement.right,
            bottom: placement.bottom,
        },
        ..Default::default()
    };
    // SAFETY: the live WebviewWindow supplied this HWND and the structure carries its length.
    unsafe { SetWindowPlacement(handle, &placement) }.map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use myalbuns_paths::AppPaths;

    use super::{ProjectWindowPlacement, load, save};

    fn paths() -> (tempfile::TempDir, AppPaths) {
        let root = tempfile::tempdir().expect("temporary application data root");
        let paths = AppPaths::from_roots(root.path(), root.path());
        (root, paths)
    }

    const PLACEMENT: ProjectWindowPlacement = ProjectWindowPlacement {
        left: -1200,
        top: 40,
        right: 240,
        bottom: 940,
        maximized: true,
    };

    #[test]
    fn the_last_saved_placement_is_read_back() {
        let (_root, paths) = paths();

        assert_eq!(load(&paths), None);
        save(&paths, PLACEMENT).expect("the placement persists");
        assert_eq!(load(&paths), Some(PLACEMENT));

        let restored = ProjectWindowPlacement {
            maximized: false,
            ..PLACEMENT
        };
        save(&paths, restored).expect("the newer placement persists");
        assert_eq!(load(&paths), Some(restored));
    }

    #[test]
    fn unreadable_or_implausible_state_uses_the_default_window() {
        let (_root, paths) = paths();
        let file = paths.project_window_placement_file();
        fs::create_dir_all(file.parent().expect("State parent")).expect("State is writable");

        let stored_states: [&[u8]; 4] = [
            br#"{"schemaVersion":2,"left":0,"top":0,"right":800,"bottom":600,"maximized":false}"#,
            br#"{"schemaVersion":1,"left":800,"top":0,"right":0,"bottom":600,"maximized":false}"#,
            br#"{"schemaVersion":1,"left":0,"top":0,"right":99999,"bottom":600,"maximized":false}"#,
            b"not json",
        ];
        for stored in stored_states {
            fs::write(&file, stored).expect("state fixture is writable");
            assert_eq!(load(&paths), None);
        }

        let empty = ProjectWindowPlacement {
            right: PLACEMENT.left,
            ..PLACEMENT
        };
        assert!(save(&paths, empty).is_err());
    }
}
