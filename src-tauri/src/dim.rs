// Plain windows for the extra screens during a break: native, no webview. They show a small
// copy of the break background, stretched; until it arrives they are a solid dark colour.
// open and close must run on the main thread, whose event loop pumps their messages.

#[cfg(windows)]
mod imp {
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    struct Bitmap {
        width: i32,
        height: i32,
        bgra: Vec<u8>,
    }

    static OPEN: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    static PICTURE: Mutex<Option<Bitmap>> = Mutex::new(None);
    const COLOR: u32 = 0x0015_1711;

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(h, &mut ps);
                if let Some(b) = PICTURE.lock().unwrap().as_ref() {
                    let mut r: RECT = std::mem::zeroed();
                    GetClientRect(h, &mut r);
                    let mut info: BITMAPINFO = std::mem::zeroed();
                    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    info.bmiHeader.biWidth = b.width;
                    info.bmiHeader.biHeight = -b.height;
                    info.bmiHeader.biPlanes = 1;
                    info.bmiHeader.biBitCount = 32;
                    SetStretchBltMode(hdc, HALFTONE);
                    StretchDIBits(hdc, 0, 0, r.right, r.bottom, 0, 0, b.width, b.height, b.bgra.as_ptr().cast(), &info, DIB_RGB_COLORS, SRCCOPY);
                }
                EndPaint(h, &ps);
                0
            }
            _ => DefWindowProcW(h, msg, w, l),
        }
    }

    pub fn open(rects: &[(i32, i32, i32, i32)]) {
        let class: Vec<u16> = "BrightPauseDim\0".encode_utf16().collect();
        let mut list = OPEN.lock().unwrap();
        unsafe {
            let inst = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: inst,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: CreateSolidBrush(COLOR),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc);
            for &(x, y, w, h) in rects {
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class.as_ptr(),
                    class.as_ptr(),
                    WS_POPUP,
                    x,
                    y,
                    w,
                    h,
                    null_mut(),
                    null_mut(),
                    inst,
                    null(),
                );
                if !hwnd.is_null() {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    list.push(hwnd as usize);
                }
            }
        }
    }

    // rgba is what a canvas gives; GDI wants BGRA.
    pub fn set_picture(width: i32, height: i32, mut rgba: Vec<u8>) {
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        *PICTURE.lock().unwrap() = Some(Bitmap { width, height, bgra: rgba });
        for &h in OPEN.lock().unwrap().iter() {
            unsafe { InvalidateRect(h as HWND, null(), 0) };
        }
    }

    pub fn close() {
        for h in OPEN.lock().unwrap().drain(..) {
            unsafe { DestroyWindow(h as HWND) };
        }
        *PICTURE.lock().unwrap() = None;
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn open(_rects: &[(i32, i32, i32, i32)]) {}
    pub fn set_picture(_width: i32, _height: i32, _rgba: Vec<u8>) {}
    pub fn close() {}
}

pub use imp::{close, open, set_picture};
