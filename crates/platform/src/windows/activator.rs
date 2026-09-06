//! Foreground activation with the `AttachThreadInput` focus-steal pattern,
//! ported from the original C# implementation.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::AttachThreadInput;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow,
    ShowWindow, SW_RESTORE,
};

use crate::{PlatformError, WindowActivator, WindowRef};

pub struct WindowsActivator;

impl WindowsActivator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsActivator {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowActivator for WindowsActivator {
    fn activate(&self, win: &WindowRef) -> Result<(), PlatformError> {
        let WindowRef::Hwnd(raw) = win else {
            return Err(PlatformError::InvalidData(format!(
                "windows activator got a non-HWND window ref: {win:?}"
            )));
        };
        let hwnd = *raw as HWND;

        unsafe {
            if IsWindow(hwnd) == 0 {
                return Err(PlatformError::InvalidData(
                    "window handle is no longer valid".to_string(),
                ));
            }

            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }

            // Windows refuses SetForegroundWindow from a background process;
            // briefly attaching our input queue to the target window's thread
            // lifts that restriction (same trick the C# original used).
            let current_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
            let attached = target_thread != 0
                && target_thread != current_thread
                && AttachThreadInput(current_thread, target_thread, 1) != 0;

            let foreground_ok = SetForegroundWindow(hwnd) != 0;
            BringWindowToTop(hwnd);

            if attached {
                AttachThreadInput(current_thread, target_thread, 0);
            }

            if !foreground_ok {
                return Err(PlatformError::CommandFailed(
                    "SetForegroundWindow failed".to_string(),
                ));
            }
        }

        Ok(())
    }
}
