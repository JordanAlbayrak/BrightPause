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

/// True while a full-screen game or video, a presentation, or Do Not Disturb is on.
#[cfg(windows)]
pub fn screen_busy() -> bool {
    use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
    let mut state = 0;
    unsafe {
        if SHQueryUserNotificationState(&mut state) != 0 {
            return false;
        }
    }
    matches!(state, 2 | 3 | 4 | 6)
}

#[cfg(not(windows))]
pub fn idle_seconds() -> f64 {
    0.0
}

#[cfg(not(windows))]
pub fn screen_busy() -> bool {
    false
}
