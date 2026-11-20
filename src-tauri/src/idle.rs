#[cfg(windows)]
pub fn idle_seconds() -> f64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return 0.0;
        }
        GetTickCount().wrapping_sub(info.dwTime) as f64 / 1000.0
    }
}

/// Why Windows would hold a notification now, from SHQueryUserNotificationState; None when it would not.
pub fn busy_reason(state: i32) -> Option<&'static str> {
    match state {
        1 => Some("screen locked or screen saver on"),
        2 => Some("full-screen app"),
        3 => Some("exclusive full-screen game (Direct3D)"),
        4 => Some("presentation mode"),
        6 => Some("Do Not Disturb"),
        _ => None,
    }
}

#[cfg(windows)]
pub fn notification_state() -> i32 {
    use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
    let mut state = 0;
    if unsafe { SHQueryUserNotificationState(&mut state) } != 0 {
        return 0;
    }
    state
}

pub struct Foreground {
    pub exe: String,
    /// A borderless window over a whole screen, on any monitor: a game or video in full screen.
    pub full_screen: bool,
}

/// Windows reports full screen only on the main monitor; this covers the side ones too. A maximized
/// window is ordinary work even when it covers the screen (auto-hidden taskbar), and the desktop
/// itself is a window that covers every screen.
fn fills_screen(window: (i32, i32, i32, i32), monitor: (i32, i32, i32, i32), maximized: bool, class: &str) -> bool {
    let covers = window.0 <= monitor.0 && window.1 <= monitor.1 && window.2 >= monitor.2 && window.3 >= monitor.3;
    covers && !maximized && !["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"].contains(&class)
}

#[cfg(windows)]
pub fn foreground() -> Option<Foreground> {
    use windows_sys::Win32::Foundation::{CloseHandle, RECT};
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsZoomed};
    unsafe {
        let h = GetForegroundWindow();
        if h.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(h, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        let mut exe = String::from("?");
        if !process.is_null() {
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) != 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                exe = path.rsplit('\\').next().unwrap_or(&path).to_string();
            }
            CloseHandle(process);
        }
        let mut class = [0u16; 128];
        let n = GetClassNameW(h, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(h, &mut r);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut info);
        let m = info.rcMonitor;
        let own = pid == std::process::id();
        let full_screen = !own && fills_screen((r.left, r.top, r.right, r.bottom), (m.left, m.top, m.right, m.bottom), IsZoomed(h) != 0, &class);
        Some(Foreground { exe, full_screen })
    }
}

#[cfg(not(windows))]
pub fn idle_seconds() -> f64 {
    0.0
}

#[cfg(not(windows))]
pub fn notification_state() -> i32 {
    0
}

#[cfg(not(windows))]
pub fn foreground() -> Option<Foreground> {
    None
}

#[cfg(test)]
mod tests {
    use super::{busy_reason, fills_screen};

    #[test]
    fn full_screen_presentation_and_do_not_disturb_hold_breaks() {
        for state in [1, 2, 3, 4, 6] {
            assert!(busy_reason(state).is_some(), "{state}");
        }
        for state in [0, 5, 7] {
            assert!(busy_reason(state).is_none(), "{state}");
        }
    }

    #[test]
    fn a_borderless_window_over_any_screen_counts_as_full_screen() {
        let side = (-1920, -87, 0, 993);
        assert!(fills_screen(side, side, false, "UnityWndClass"));
        assert!(fills_screen((-1, -1, 2561, 1441), (0, 0, 2560, 1440), false, "SDL_app"));
        assert!(!fills_screen((100, 100, 1000, 700), (0, 0, 2560, 1440), false, "Notepad"));
        assert!(!fills_screen((-8, -8, 2568, 1448), (0, 0, 2560, 1440), true, "Chrome_WidgetWin_1"), "maximized");
        assert!(!fills_screen((0, 0, 2560, 1440), (0, 0, 2560, 1440), false, "Progman"), "desktop");
        assert!(!fills_screen((0, 0, 2560, 1440), (0, 0, 2560, 1440), false, "WorkerW"), "desktop");
    }
}