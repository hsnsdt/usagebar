//! Tiny Win32 helpers.

/// True when the foreground window belongs to this process. Used to tell a
/// real focus loss (user clicked elsewhere) from WebView2's child HWND taking
/// focus inside our own popup, which tao also reports as `Focused(false)`.
#[cfg(windows)]
pub fn foreground_is_ours() -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    // SAFETY: plain Win32 queries with valid out-pointers; no ownership involved.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid == GetCurrentProcessId()
    }
}

/// (pid, window title) of the foreground window, for debugging focus loss.
#[cfg(windows)]
pub fn foreground_info() -> (u32, String) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (0, String::new());
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut buf = [0u16; 128];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        (pid, String::from_utf16_lossy(&buf[..n.max(0) as usize]))
    }
}

#[cfg(not(windows))]
pub fn foreground_is_ours() -> bool {
    false
}

#[cfg(not(windows))]
pub fn foreground_info() -> (u32, String) {
    (0, String::new())
}
