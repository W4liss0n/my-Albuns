//! Projects opened together hand the focus only to the first editor that
//! becomes ready; later editors open without taking the person away from the
//! window in use. For each group of files opened together the Global creates
//! one named auto-reset event, signalled, and passes its name to every Host it
//! starts. The first Host whose editor becomes ready consumes the signal and
//! takes the focus; the others find it consumed. The Global keeps the event
//! until the group's launches end, so closing the first editor early does not
//! hand the focus to the next one.
//!
//! The editor's own focus request (`tao`'s `set_focus`) bypasses the Windows
//! foreground lock with a simulated Alt key, so the operating system alone
//! cannot keep a later editor from coming forward.

pub(crate) const FOCUS_CLAIM_ENV: &str = "MYALBUNS_HOST_FOCUS_CLAIM";

fn claim_name(id: &str) -> Option<String> {
    // The id is a Global-generated identifier; anything else is ignored.
    (!id.is_empty() && id.len() <= 64 && id.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .then(|| format!(r"Local\MyAlbuns.OpeningFocus.v2.{id}"))
}

/// Whether this Host's editor may take the focus when it is first shown.
/// Without a claim (a single opening from the Welcome, Save As) it always may;
/// a claim that cannot be read keeps today's behaviour too.
pub(crate) fn may_take_focus() -> bool {
    let Ok(id) = std::env::var(FOCUS_CLAIM_ENV) else {
        return true;
    };
    let Some(name) = claim_name(&id) else {
        return true;
    };
    static RESULT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *RESULT.get_or_init(|| consume_claim(&name).unwrap_or(true))
}

/// The Global's side: one claim for a group of files opened together.
pub(crate) struct OpeningFocusClaim {
    id: String,
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: the event handle is a kernel object handle, usable from any thread;
// it is only closed on drop.
#[cfg(windows)]
unsafe impl Send for OpeningFocusClaim {}
#[cfg(windows)]
unsafe impl Sync for OpeningFocusClaim {}

impl OpeningFocusClaim {
    /// `None` when the event cannot be created: the Hosts then keep today's
    /// behaviour.
    pub(crate) fn new() -> Option<Self> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = claim_name(&id)?;
        create_claim(id, &name)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

#[cfg(windows)]
fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn create_claim(id: String, name: &str) -> Option<OpeningFocusClaim> {
    use windows_sys::Win32::System::Threading::CreateEventW;

    let name = wide(name);
    // Auto-reset, initially signalled: exactly one successful wait consumes it.
    // SAFETY: `name` is a terminated UTF-16 buffer that outlives the call.
    let handle = unsafe { CreateEventW(std::ptr::null(), 0, 1, name.as_ptr()) };
    (!handle.is_null()).then_some(OpeningFocusClaim { id, handle })
}

#[cfg(not(windows))]
fn create_claim(id: String, _name: &str) -> Option<OpeningFocusClaim> {
    Some(OpeningFocusClaim { id })
}

#[cfg(windows)]
impl Drop for OpeningFocusClaim {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this claim and is closed only here.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.handle) };
    }
}

/// `Some(true)` when this Host consumed the claim, `Some(false)` when another
/// Host already did, `None` when the claim cannot be read.
#[cfg(windows)]
fn consume_claim(name: &str) -> Option<bool> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{
            EVENT_MODIFY_STATE, OpenEventW, SYNCHRONIZATION_SYNCHRONIZE, WaitForSingleObject,
        },
    };

    let name = wide(name);
    // SAFETY: `name` is a terminated UTF-16 buffer that outlives the call.
    let handle = unsafe {
        OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE | EVENT_MODIFY_STATE,
            0,
            name.as_ptr(),
        )
    };
    if handle.is_null() {
        return None;
    }
    // SAFETY: a live event handle and a non-blocking wait.
    let consumed = unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0;
    // SAFETY: this handle is owned here and not used again.
    unsafe { CloseHandle(handle) };
    Some(consumed)
}

#[cfg(not(windows))]
fn consume_claim(_name: &str) -> Option<bool> {
    None
}

/// Shows an editor that did not take the focus without covering the window in
/// use. Showing a remembered maximized placement activates the window, so the
/// editor is cloaked while it is shown; if it was activated anyway, the focus
/// returns to the window that had it and the editor goes right behind that
/// window. Its taskbar button flashes until the person switches to it.
#[cfg(windows)]
pub(crate) struct QuietShow {
    window: windows::Win32::Foundation::HWND,
    previous: windows::Win32::Foundation::HWND,
}

#[cfg(windows)]
impl QuietShow {
    pub(crate) fn begin(window: &tauri::WebviewWindow) -> Option<Self> {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        let window = window.hwnd().ok()?;
        // SAFETY: reads the current foreground window; no pointer is kept.
        let previous = unsafe { GetForegroundWindow() };
        set_cloaked(window, true);
        Some(Self { window, previous })
    }

    pub(crate) fn finish(self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx, GetForegroundWindow,
            IsWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
            SetForegroundWindow, SetWindowPos,
        };

        // SAFETY: plain window-manager calls on handles this process obtained
        // just before; a handle that became invalid makes them fail harmlessly.
        unsafe {
            let previous_alive = !self.previous.0.is_null()
                && self.previous != self.window
                && IsWindow(Some(self.previous)).as_bool();
            let activated = GetForegroundWindow() == self.window;
            if previous_alive {
                if activated {
                    let _ = SetForegroundWindow(self.previous);
                }
                let _ = SetWindowPos(
                    self.window,
                    Some(self.previous),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                );
            }
            set_cloaked(self.window, false);
            tracing::info!(
                target: "myalbuns.desktop",
                had_previous = previous_alive,
                activated,
                kept_foreground = GetForegroundWindow() == self.window,
                event = "project_window_shown_behind",
            );
            let flash = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.window,
                dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
                uCount: 0,
                dwTimeout: 0,
            };
            let _ = FlashWindowEx(&flash);
        }
    }
}

#[cfg(windows)]
fn set_cloaked(window: windows::Win32::Foundation::HWND, cloaked: bool) {
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};

    let value = windows::core::BOOL::from(cloaked);
    // SAFETY: the attribute is a BOOL read synchronously from this stack value.
    let _ = unsafe {
        DwmSetWindowAttribute(
            window,
            DWMWA_CLOAK,
            (&raw const value).cast(),
            std::mem::size_of_val(&value) as u32,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_editor_of_a_group_takes_the_focus() {
        let claim = OpeningFocusClaim::new().expect("the claim is created");
        let name = claim_name(claim.id()).unwrap();
        assert_eq!(consume_claim(&name), Some(true), "the first editor");
        assert_eq!(
            consume_claim(&name),
            Some(false),
            "a later editor of the group"
        );

        let other = OpeningFocusClaim::new().expect("another group");
        let other_name = claim_name(other.id()).unwrap();
        assert_eq!(
            consume_claim(&other_name),
            Some(true),
            "another group has its own claim"
        );

        drop(claim);
        assert_eq!(
            consume_claim(&name),
            None,
            "an ended group no longer has a claim"
        );
    }

    #[test]
    fn a_malformed_claim_is_ignored() {
        assert_eq!(claim_name(""), None);
        assert_eq!(claim_name(r"..\Global\x"), None);
        assert!(claim_name("0123abcd").is_some());
    }
}
