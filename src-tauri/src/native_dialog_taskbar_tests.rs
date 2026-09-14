use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Com::{
            CLSCTX_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Shell::{DefSubclassProc, ITaskbarList, SetWindowSubclass, TaskbarList},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

use super::prepare_owned_window;

struct TestWindow(HWND);

impl TestWindow {
    fn new(owner: Option<HWND>) -> Self {
        let style = WS_CAPTION | WS_CLIPSIBLINGS | WS_SYSMENU;
        let style = if owner.is_some() {
            style | WS_POPUP
        } else {
            style
        };
        Self(unsafe {
            CreateWindowExW(
                WS_EX_WINDOWEDGE,
                w!("STATIC"),
                w!("MyAlbuns — teste da barra"),
                style,
                80,
                80,
                380,
                164,
                owner,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

#[test]
fn owned_taskbar_style_survives_framework_style_reapplication() {
    let owner = TestWindow::new(None);
    let dialog = TestWindow::new(Some(owner.0));
    prepare_owned_window(dialog.0).unwrap();
    // Tao recomputes GWL_EXSTYLE from its cached flags on visibility changes.
    unsafe {
        SetWindowLongPtrW(dialog.0, GWL_EXSTYLE, WS_EX_WINDOWEDGE.0 as isize);
    }
    let style = unsafe { GetWindowLongPtrW(dialog.0, GWL_EXSTYLE) } as u32;
    assert_ne!(
        style & WS_EX_APPWINDOW.0,
        0,
        "the owned progress must remain eligible for the taskbar"
    );
    assert_eq!(unsafe { GetWindow(dialog.0, GW_OWNER) }.unwrap(), owner.0);
}

unsafe extern "system" fn observe_button(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) } {
        unsafe { &*(data as *const AtomicU32) }.fetch_add(1, Ordering::Relaxed);
    }
    unsafe { DefSubclassProc(hwnd, message, wp, lp) }
}

fn pump(duration: Duration) {
    let started = Instant::now();
    while started.elapsed() < duration {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "opens temporary native windows; requires an interactive Windows desktop"]
fn shell_creates_button_for_progress_with_hidden_owner() {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .unwrap();
    {
        let owner_buttons = AtomicU32::new(0);
        let dialog_buttons = AtomicU32::new(0);
        let owner = TestWindow::new(None);
        let dialog = TestWindow::new(Some(owner.0));
        let taskbar: ITaskbarList =
            unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_SERVER) }.unwrap();
        unsafe {
            taskbar.HrInit().unwrap();
            assert!(
                SetWindowSubclass(
                    owner.0,
                    Some(observe_button),
                    1,
                    &owner_buttons as *const _ as usize
                )
                .as_bool()
            );
            assert!(
                SetWindowSubclass(
                    dialog.0,
                    Some(observe_button),
                    1,
                    &dialog_buttons as *const _ as usize
                )
                .as_bool()
            );
            let _ = ShowWindow(owner.0, SW_SHOWNOACTIVATE);
        }
        pump(Duration::from_millis(800));
        unsafe {
            let _ = ShowWindow(owner.0, SW_HIDE);
            let _ = ShowWindow(dialog.0, SW_SHOWNOACTIVATE);
        }
        prepare_owned_window(dialog.0).unwrap();
        unsafe { taskbar.AddTab(dialog.0) }.unwrap();
        pump(Duration::from_millis(1800));
        unsafe { taskbar.DeleteTab(dialog.0) }.unwrap();
        assert!(
            owner_buttons.load(Ordering::Relaxed) > 0,
            "the interactive Shell must create the control window button"
        );
        assert!(
            dialog_buttons.load(Ordering::Relaxed) > 0,
            "Windows never confirmed creation of the progress taskbar button"
        );
        assert!(!unsafe { IsWindowVisible(owner.0) }.as_bool());
        assert_eq!(unsafe { GetWindow(dialog.0, GW_OWNER) }.unwrap(), owner.0);
    }
    unsafe {
        CoUninitialize();
    }
}
