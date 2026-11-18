//! "In a call": another app is recording from a microphone right now, or (on Windows builds that
//! still record it in the registry) using the camera. Call apps keep the microphone stream open for
//! the whole call, muted or not, so this covers Teams, Zoom and browser calls, offline and without
//! any sign-in.

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ};

    const STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0) };
        }
    }

    impl Key {
        fn open(parent: HKEY, path: &str) -> Option<Key> {
            let mut key: HKEY = std::ptr::null_mut();
            let ok = unsafe { RegOpenKeyExW(parent, wide(path).as_ptr(), 0, KEY_READ, &mut key) } == ERROR_SUCCESS;
            ok.then(|| Key(key))
        }

        fn children(&self) -> Vec<String> {
            let mut names = Vec::new();
            let mut buf = [0u16; 512];
            for i in 0.. {
                let mut len = buf.len() as u32;
                let rc = unsafe { RegEnumKeyExW(self.0, i, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
                if rc != ERROR_SUCCESS {
                    break;
                }
                names.push(String::from_utf16_lossy(&buf[..len as usize]));
            }
            names
        }

        fn qword(&self, name: &str) -> Option<u64> {
            let mut value = 0u64;
            let mut size = 8u32;
            let rc = unsafe { RegQueryValueExW(self.0, wide(name).as_ptr(), std::ptr::null(), std::ptr::null_mut(), &mut value as *mut u64 as *mut u8, &mut size) };
            (rc == ERROR_SUCCESS && size == 8).then_some(value)
        }

        fn in_use(&self) -> bool {
            matches!((self.qword("LastUsedTimeStart"), self.qword("LastUsedTimeStop")), (Some(start), Some(0)) if start != 0)
        }
    }

    fn device_in_use(device: &str) -> bool {
        let Some(root) = Key::open(HKEY_CURRENT_USER, &format!(r"{STORE}\{device}")) else { return false };
        root.children().iter().any(|name| {
            let Some(app) = Key::open(root.0, name) else { return false };
            if name == "NonPackaged" {
                app.children().iter().any(|exe| Key::open(app.0, exe).is_some_and(|k| k.in_use()))
            } else {
                app.in_use()
            }
        })
    }

    /// True when another process has an active recording session on any microphone.
    fn mic_session_active() -> bool {
        use windows::core::Interface;
        use windows::Win32::Media::Audio::*;
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
        let me = std::process::id();
        let check = || -> windows::core::Result<bool> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let mics = devices.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
                for i in 0..mics.GetCount()? {
                    let manager: IAudioSessionManager2 = mics.Item(i)?.Activate(CLSCTX_ALL, None)?;
                    let sessions = manager.GetSessionEnumerator()?;
                    for j in 0..sessions.GetCount()? {
                        let session = sessions.GetSession(j)?;
                        if session.GetState()? != AudioSessionStateActive {
                            continue;
                        }
                        let s2: IAudioSessionControl2 = session.cast()?;
                        if s2.GetProcessId().unwrap_or(0) != me {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
        };
        check().unwrap_or(false)
    }

    pub fn in_call() -> bool {
        mic_session_active() || device_in_use("microphone") || device_in_use("webcam")
    }
}

#[cfg(windows)]
pub use win::in_call;

#[cfg(not(windows))]
pub fn in_call() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs a live microphone session"]
    fn live_in_call() {
        println!("in_call = {}", super::in_call());
    }
}
