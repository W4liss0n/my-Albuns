//! Windows taskbar presentation for progress whose owner is hidden.
use std::io;

use tauri::WebviewWindow;
use windows::{
    Win32::{
        Foundation::{ERROR_SUCCESS, GetLastError, HWND, LPARAM, LRESULT, SetLastError, WPARAM},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GWL_EXSTYLE, GetWindowLongPtrW, RegisterWindowMessageW, STYLESTRUCT,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
                SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WM_NCDESTROY, WM_STYLECHANGING,
                WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
            },
        },
    },
    core::w,
};

const TASKBAR_SUBCLASS_ID: usize = 1;

pub(crate) fn show_owned_window(window: &WebviewWindow) -> io::Result<()> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let result = target
                .hwnd()
                .map_err(io::Error::other)
                .and_then(prepare_owned_window)
                .and_then(|()| target.set_skip_taskbar(false).map_err(io::Error::other));
            let _ = sender.send(result);
        })
        .map_err(io::Error::other)?;
    receiver
        .recv()
        .map_err(|_| io::Error::other("the taskbar presentation became unavailable"))?
}

// Called on the HWND's thread. Keep the native owner for activation/modality;
// AddTab alone does not expose an owned popup whose owner is hidden on Windows 11.
fn prepare_owned_window(handle: HWND) -> io::Result<()> {
    unsafe {
        let button_created = RegisterWindowMessageW(w!("TaskbarButtonCreated"));
        if button_created == 0 {
            return Err(io::Error::last_os_error());
        }
        if !SetWindowSubclass(
            handle,
            Some(taskbar_window_proc),
            TASKBAR_SUBCLASS_ID,
            button_created as usize,
        )
        .as_bool()
        {
            return Err(io::Error::other(
                "the taskbar window policy could not be installed",
            ));
        }
        let style = GetWindowLongPtrW(handle, GWL_EXSTYLE) as u32;
        SetLastError(ERROR_SUCCESS);
        let previous = SetWindowLongPtrW(handle, GWL_EXSTYLE, taskbar_style(style) as isize);
        if previous == 0 && GetLastError() != ERROR_SUCCESS {
            let error = io::Error::last_os_error();
            let _ = RemoveWindowSubclass(handle, Some(taskbar_window_proc), TASKBAR_SUBCLASS_ID);
            return Err(error);
        }
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOACTIVATE
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER,
        )
        .map_err(io::Error::other)?;
    }
    Ok(())
}

fn taskbar_style(style: u32) -> u32 {
    (style | WS_EX_APPWINDOW.0) & !WS_EX_TOOLWINDOW.0
}

unsafe extern "system" fn taskbar_window_proc(
    handle: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    id: usize,
    button_created: usize,
) -> LRESULT {
    if message == WM_STYLECHANGING && wp.0 as i32 == GWL_EXSTYLE.0 {
        // Tao 0.35 rebuilds extended styles from cached flags on show/hide.
        // Preserve this policy through that rewrite and through dialog reuse.
        let styles = unsafe { &mut *(lp.0 as *mut STYLESTRUCT) };
        styles.styleNew = taskbar_style(styles.styleNew);
    } else if message as usize == button_created {
        tracing::info!(target: "myalbuns.desktop", event = "opening_taskbar_button_created");
    } else if message == WM_NCDESTROY {
        let _ = unsafe { RemoveWindowSubclass(handle, Some(taskbar_window_proc), id) };
    }
    unsafe { DefSubclassProc(handle, message, wp, lp) }
}

#[cfg(test)]
#[path = "native_dialog_taskbar_tests.rs"]
mod tests;
