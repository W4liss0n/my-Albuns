use crate::ipc_contract::MediaFileDrag;
use std::{path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager};

pub(crate) const MEDIA_FILE_DRAG_EVENT: &str = "myalbuns-media-file-drag";

/// Native drag events of the Project window, wherever they come from: Tauri's
/// own drop targets or the ones [`complete_drop_targets`] adds.
pub(crate) fn forward(window: &tauri::Window, event: &tauri::DragDropEvent) {
    if let Some(payload) = window.state::<NativeMediaDrops>().receive(event) {
        let _ = window.emit(MEDIA_FILE_DRAG_EVENT, payload);
    }
}

pub(crate) fn on_project_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::DragDrop(event) => forward(window, event),
        // WebView2 creates or replaces its inner windows after the Project
        // window is shown and when it is restored.
        #[cfg(windows)]
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(true) => {
            if let Some(webview) = window
                .webviews()
                .into_iter()
                .find(|webview| webview.label() == window.label())
            {
                complete_drop_targets(&webview);
            }
        }
        _ => {}
    }
}

/// Wry gives the WebView its file drop target only on the WebView2 windows
/// that exist when the WebView is created (tauri-apps/wry#1638). The Project
/// window is created hidden, and the windows under the pointer appear only
/// once it is shown, so Windows refused every file dragged over the window.
/// Registers a target on each inner window still without one; the targets
/// wry registered stay as they are.
#[cfg(windows)]
pub(crate) fn complete_drop_targets(webview: &tauri::Webview) {
    let window = webview.window();
    let label = window.label().to_owned();
    let result = webview.with_webview(move |native| {
        // with_webview holds Tauri's WebView dispatcher lock: only native
        // calls here. The listener runs later, during a drag.
        let mut container = windows::Win32::Foundation::HWND::default();
        if unsafe { native.controller().ParentWindow(&mut container) }.is_err() {
            return;
        }
        let listener = std::rc::Rc::new(move |event: tauri::DragDropEvent| {
            if let Some(payload) = window.state::<NativeMediaDrops>().receive(&event) {
                emit_in_order(window.clone(), payload);
            }
        });
        let registered = native_drop::register_missing(container, listener);
        if registered > 0 {
            tracing::info!(
                target: "myalbuns.desktop",
                window_label = label,
                registered,
                event = "media_drop_targets_completed",
            );
        }
    });
    if let Err(error) = result {
        tracing::warn!(
            target: "myalbuns.desktop",
            error = %error,
            event = "media_drop_targets_unavailable",
        );
    }
}

/// Windows calls a drop target on the main thread while the drag source waits
/// for its answer; Tauri itself only posts wry's drop events to its loop. One
/// thread emits them, in the order the drag produced them.
#[cfg(windows)]
fn emit_in_order(window: tauri::Window, payload: MediaFileDrag) {
    use std::sync::{OnceLock, mpsc};
    static EMITTER: OnceLock<mpsc::Sender<(tauri::Window, MediaFileDrag)>> = OnceLock::new();
    let sender = EMITTER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<(tauri::Window, MediaFileDrag)>();
        let emitter = std::thread::Builder::new()
            .name("media-file-drop".into())
            .spawn(move || {
                for (window, payload) in receiver {
                    let _ = window.emit(MEDIA_FILE_DRAG_EVENT, payload);
                }
            });
        if let Err(error) = emitter {
            tracing::warn!(
                target: "myalbuns.desktop",
                error = %error,
                event = "media_drop_emitter_unavailable",
            );
        }
        sender
    });
    let _ = sender.send((window, payload));
}

#[cfg(windows)]
mod native_drop {
    use std::{cell::Cell, ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, rc::Rc};

    use windows::{
        Win32::{
            Foundation::{DRAGDROP_E_ALREADYREGISTERED, HWND, LPARAM, POINT, POINTL},
            Graphics::Gdi::ScreenToClient,
            System::{
                Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
                Ole::{
                    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget,
                    IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium,
                },
                SystemServices::MODIFIERKEYS_FLAGS,
            },
            UI::{
                Shell::{DragQueryFileW, HDROP},
                WindowsAndMessaging::EnumChildWindows,
            },
        },
        core::{BOOL, Ref, implement},
    };

    pub(super) type Listener = Rc<dyn Fn(tauri::DragDropEvent)>;

    /// Registers one target on every descendant of `container` that has
    /// none. Returns how many windows it registered.
    pub(super) fn register_missing(container: HWND, listener: Listener) -> usize {
        let target: IDropTarget = FileDropTarget {
            container,
            listener,
            accepted: Cell::new(false),
        }
        .into();
        descendants(container)
            .into_iter()
            .filter(|hwnd| match unsafe { RegisterDragDrop(*hwnd, &target) } {
                Ok(()) => true,
                Err(error) if error.code() == DRAGDROP_E_ALREADYREGISTERED => false,
                Err(error) => {
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "media_drop_target_registration_failed",
                    );
                    false
                }
            })
            .count()
    }

    fn descendants(parent: HWND) -> Vec<HWND> {
        unsafe extern "system" fn collect(hwnd: HWND, found: LPARAM) -> BOOL {
            unsafe { (*(found.0 as *mut Vec<HWND>)).push(hwnd) };
            true.into()
        }
        let mut found = Vec::new();
        let _ = unsafe {
            EnumChildWindows(
                Some(parent),
                Some(collect),
                LPARAM(&mut found as *mut Vec<HWND> as isize),
            )
        };
        found
    }

    /// Same events as wry's target: only file drags count, and positions are
    /// physical pixels of the WebView's client area.
    #[implement(IDropTarget)]
    struct FileDropTarget {
        container: HWND,
        listener: Listener,
        accepted: Cell<bool>,
    }

    impl FileDropTarget {
        fn position(&self, point: &POINTL) -> tauri::PhysicalPosition<f64> {
            let mut point = POINT {
                x: point.x,
                y: point.y,
            };
            let _ = unsafe { ScreenToClient(self.container, &mut point) };
            tauri::PhysicalPosition::new(point.x.into(), point.y.into())
        }

        fn effect(&self) -> DROPEFFECT {
            if self.accepted.get() {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            }
        }
    }

    fn file_paths(data: Ref<'_, IDataObject>) -> Option<Vec<PathBuf>> {
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let mut medium = unsafe { data.as_ref()?.GetData(&format) }.ok()?;
        let hdrop = HDROP(unsafe { medium.u.hGlobal.0 });
        let count = unsafe { DragQueryFileW(hdrop, u32::MAX, None) };
        let paths = (0..count)
            .map(|index| {
                let length = unsafe { DragQueryFileW(hdrop, index, None) } as usize;
                let mut path = vec![0; length + 1];
                unsafe { DragQueryFileW(hdrop, index, Some(&mut path)) };
                PathBuf::from(OsString::from_wide(&path[..length]))
            })
            .collect();
        unsafe { ReleaseStgMedium(&mut medium) };
        Some(paths)
    }

    impl IDropTarget_Impl for FileDropTarget_Impl {
        fn DragEnter(
            &self,
            data: Ref<'_, IDataObject>,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            let paths = file_paths(data);
            self.accepted.set(paths.is_some());
            if let Some(paths) = paths {
                (self.listener)(tauri::DragDropEvent::Enter {
                    paths,
                    position: self.position(point),
                });
            }
            unsafe { *effect = self.effect() };
            Ok(())
        }

        fn DragOver(
            &self,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            if self.accepted.get() {
                (self.listener)(tauri::DragDropEvent::Over {
                    position: self.position(point),
                });
            }
            unsafe { *effect = self.effect() };
            Ok(())
        }

        fn DragLeave(&self) -> windows::core::Result<()> {
            if self.accepted.replace(false) {
                (self.listener)(tauri::DragDropEvent::Leave);
            }
            Ok(())
        }

        fn Drop(
            &self,
            data: Ref<'_, IDataObject>,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            let accepted = self.accepted.replace(false);
            match file_paths(data).filter(|_| accepted) {
                Some(paths) => {
                    (self.listener)(tauri::DragDropEvent::Drop {
                        paths,
                        position: self.position(point),
                    });
                    unsafe { *effect = DROPEFFECT_COPY };
                }
                None => unsafe { *effect = DROPEFFECT_NONE },
            }
            Ok(())
        }
    }
}

struct PendingDrop {
    id: String,
    paths: Vec<PathBuf>,
}

/// At most one unclaimed drop belongs to this Project window. Paths stay native;
/// the WebView only decides whether the drop landed inside the visible Panel.
#[derive(Default)]
pub(crate) struct NativeMediaDrops(Mutex<Option<PendingDrop>>);

impl NativeMediaDrops {
    pub(crate) fn receive(&self, event: &tauri::DragDropEvent) -> Option<MediaFileDrag> {
        match event {
            tauri::DragDropEvent::Enter { position, .. } => {
                *self.0.lock().ok()? = None;
                Some(MediaFileDrag::Over {
                    x: position.x,
                    y: position.y,
                })
            }
            tauri::DragDropEvent::Over { position } => Some(MediaFileDrag::Over {
                x: position.x,
                y: position.y,
            }),
            tauri::DragDropEvent::Drop { paths, position } => {
                let drop_id = uuid::Uuid::new_v4().to_string();
                *self.0.lock().ok()? = Some(PendingDrop {
                    id: drop_id.clone(),
                    paths: paths.clone(),
                });
                Some(MediaFileDrag::Drop {
                    x: position.x,
                    y: position.y,
                    drop_id,
                })
            }
            // A native Leave can follow Drop before the async import claims it.
            tauri::DragDropEvent::Leave => Some(MediaFileDrag::Leave),
            _ => None,
        }
    }

    pub(crate) fn take(&self, drop_id: &str) -> Result<Vec<PathBuf>, String> {
        let mut pending = self
            .0
            .lock()
            .map_err(|_| "Não foi possível receber os arquivos arrastados.".to_string())?;
        if pending.as_ref().is_none_or(|pending| pending.id != drop_id) {
            return Err(
                "Esta soltura não está mais disponível. Arraste os arquivos novamente.".into(),
            );
        }
        Ok(pending.take().expect("the matching drop exists").paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

    #[test]
    fn native_drop_keeps_non_unicode_windows_paths_outside_json_and_consumes_once() {
        let path = PathBuf::from(OsString::from_wide(&[
            67, 58, 92, 70, 111, 116, 111, 115, 92, 0xd800, 46, 112, 110, 103,
        ]));
        let drops = NativeMediaDrops::default();
        let event = drops
            .receive(&tauri::DragDropEvent::Drop {
                paths: vec![path.clone()],
                position: tauri::PhysicalPosition::new(30.0, 60.0),
            })
            .unwrap();
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "drop");
        assert!(json.get("paths").is_none());
        let id = json["dropId"].as_str().unwrap();
        assert_eq!(drops.take(id).unwrap(), vec![path]);
        assert!(drops.take(id).is_err());
        assert!(drops.take("an unrelated drop").is_err());
    }

    #[test]
    fn drop_targets_reach_inner_windows_created_after_the_first_registration() {
        use windows::{
            Win32::{
                Foundation::HWND,
                System::Ole::{OleInitialize, OleUninitialize, RevokeDragDrop},
                UI::WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD,
                    WS_POPUP,
                },
            },
            core::w,
        };

        unsafe { OleInitialize(None) }.unwrap();
        let create = |parent: Option<HWND>, style: WINDOW_STYLE| {
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!(""),
                    style,
                    0,
                    0,
                    10,
                    10,
                    parent,
                    None,
                    None,
                    None,
                )
            }
            .unwrap()
        };
        let listener = || -> super::native_drop::Listener { std::rc::Rc::new(|_| {}) };
        // The hidden window only has its first inner window when wry registers.
        let container = create(None, WS_POPUP);
        let first = create(Some(container), WS_CHILD);
        assert_eq!(
            super::native_drop::register_missing(container, listener()),
            1
        );
        // Shown, WebView2 adds the windows that end up under the pointer.
        let late = create(Some(container), WS_CHILD);
        let late_inner = create(Some(late), WS_CHILD);
        assert_eq!(
            super::native_drop::register_missing(container, listener()),
            2
        );
        assert_eq!(
            super::native_drop::register_missing(container, listener()),
            0
        );

        for hwnd in [first, late, late_inner] {
            unsafe { RevokeDragDrop(hwnd) }.unwrap();
        }
        unsafe { DestroyWindow(container) }.unwrap();
        unsafe { OleUninitialize() };
    }
}
