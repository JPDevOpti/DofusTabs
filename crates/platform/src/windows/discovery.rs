//! Game-window discovery via `EnumWindows`.

use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use dofustabs_core::window_selection::best_per_pid;

use crate::{is_game_process_name, DiscoveredWindow, GameDiscovery, PlatformError, WindowRef};

pub struct WindowsDiscovery;

impl WindowsDiscovery {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

struct EnumContext {
    found: Vec<DiscoveredWindow>,
}

impl GameDiscovery for WindowsDiscovery {
    fn scan(&self) -> Result<Vec<DiscoveredWindow>, PlatformError> {
        let mut ctx = EnumContext { found: Vec::new() };

        // EnumWindows only reports FALSE when the callback aborted the walk;
        // our callback never does, so a FALSE here is a real failure.
        let ok =
            unsafe { EnumWindows(Some(enum_callback), &mut ctx as *mut EnumContext as LPARAM) };
        if ok == 0 {
            return Err(PlatformError::CommandFailed(
                "EnumWindows failed".to_string(),
            ));
        }

        // A client may own several visible windows (IME helpers, "Release"
        // frames); keep the one whose title identifies the character.
        Ok(best_per_pid(ctx.found, |w| w.pid, |w| &w.title))
    }
}

// Returns i32 rather than naming the BOOL alias (it moved between
// windows-sys releases); the alias resolves to i32 in all of them.
unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> i32 {
    const CONTINUE_ENUM: i32 = 1;

    let ctx = &mut *(lparam as *mut EnumContext);

    if IsWindowVisible(hwnd) == 0 {
        return CONTINUE_ENUM;
    }

    let Some(title) = window_title(hwnd) else {
        return CONTINUE_ENUM;
    };

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 {
        return CONTINUE_ENUM;
    }

    let Some(process_name) = process_basename(pid) else {
        return CONTINUE_ENUM;
    };
    if !is_game_process_name(&process_name) {
        return CONTINUE_ENUM;
    }

    ctx.found.push(DiscoveredWindow {
        pid,
        title,
        process_name,
        window_ref: WindowRef::Hwnd(hwnd as isize),
    });

    CONTINUE_ENUM
}

/// Non-empty window title, or `None` for untitled windows.
unsafe fn window_title(hwnd: HWND) -> Option<String> {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return None;
    }

    let mut buf = vec![0u16; len as usize + 1];
    let copied = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if copied <= 0 {
        return None;
    }

    let title = String::from_utf16_lossy(&buf[..copied as usize]);
    let trimmed = title.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Executable basename without the `.exe` suffix, e.g. `C:\...\Dofus.exe`
/// becomes `Dofus`.
unsafe fn process_basename(pid: u32) -> Option<String> {
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return None;
    }

    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size);
    CloseHandle(process);
    if ok == 0 {
        return None;
    }

    let path = String::from_utf16_lossy(&buf[..size as usize]);
    let basename = path.rsplit(['\\', '/']).next()?;
    let name = basename
        .strip_suffix(".exe")
        .or_else(|| basename.strip_suffix(".EXE"))
        .unwrap_or(basename);
    (!name.is_empty()).then(|| name.to_string())
}
