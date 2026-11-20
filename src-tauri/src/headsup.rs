// The warning before a break: a small native window at the bottom right of the main screen, with
// Snooze and Skip. It never takes focus, so typing goes on where it was. Our own window rather than
// a Windows notification: toast buttons need activation handling that differs between the
// installed and the Store build. show and close must run on the main thread.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Snooze,
    Skip,
}

pub struct Notice {
    /// May hold {n}, filled with the seconds left.
    pub title: String,
    pub body: String,
    pub seconds: u32,
    pub buttons: Vec<(Choice, String)>,
    pub dark: bool,
}

// Layout in 96-dpi pixels.
const WIDTH: i32 = 360;
const PAD: i32 = 16;
const BUTTON_TOP: i32 = 90;
const BUTTON_HEIGHT: i32 = 32;
const BUTTON_GAP: i32 = 8;
const BUTTON_MIN: i32 = 76;
const BUTTON_TEXT_PAD: i32 = 14;

fn height(has_buttons: bool) -> i32 {
    if has_buttons {
        BUTTON_TOP + BUTTON_HEIGHT + PAD
    } else {
        BUTTON_TOP - 4
    }
}

fn px(v: i32, scale: f32) -> i32 {
    (v as f32 * scale).round() as i32
}

/// Button rectangles (left, top, right, bottom), right-aligned in label order, from each label's
/// width in pixels at this scale.
fn button_rects(scale: f32, text_widths: &[i32]) -> Vec<(i32, i32, i32, i32)> {
    let widths: Vec<i32> = text_widths.iter().map(|w| (w + 2 * px(BUTTON_TEXT_PAD, scale)).max(px(BUTTON_MIN, scale))).collect();
    let mut right = px(WIDTH - PAD, scale);
    let mut rects: Vec<_> = widths
        .iter()
        .rev()
        .map(|w| {
            let r = (right - w, px(BUTTON_TOP, scale), right, px(BUTTON_TOP + BUTTON_HEIGHT, scale));
            right -= w + px(BUTTON_GAP, scale);
            r
        })
        .collect();
    rects.reverse();
    rects
}

fn hit(rects: &[(i32, i32, i32, i32)], x: i32, y: i32) -> Option<usize> {
    rects.iter().position(|&(l, t, r, b)| x >= l && x < r && y >= t && y < b)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    struct Open {
        hwnd: usize,
        notice: Notice,
        shown: Instant,
        scale: f32,
        rects: Vec<(i32, i32, i32, i32)>,
        hover: Option<usize>,
        tracking: bool,
    }

    const WM_MOUSELEAVE: u32 = 0x02A3;

    static OPEN: Mutex<Option<Open>> = Mutex::new(None);
    static ON_CHOICE: Mutex<Option<Box<dyn Fn(Choice) + Send>>> = Mutex::new(None);

    struct Palette {
        card: u32,
        ink: u32,
        muted: u32,
        line: u32,
        accent: u32,
        accent_ink: u32,
        accent_soft: u32,
    }

    // The settings page's colours (ui/style.css), as COLORREF.
    fn palette(dark: bool) -> Palette {
        let c = |hex: u32| ((hex & 0xff) << 16) | (hex & 0xff00) | ((hex >> 16) & 0xff);
        if dark {
            Palette { card: c(0x1a221e), ink: c(0xe8eee9), muted: c(0x93a099), line: c(0x2a3530), accent: c(0x5cc7a0), accent_ink: c(0x0b1411), accent_soft: c(0x1d3129) }
        } else {
            Palette { card: c(0xfffdf8), ink: c(0x1b2823), muted: c(0x68736d), line: c(0xe3ded1), accent: c(0x1f6f54), accent_ink: c(0xffffff), accent_soft: c(0xe3efe8) }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    unsafe fn font(size: i32, weight: i32, scale: f32) -> HFONT {
        let face = wide("Segoe UI");
        CreateFontW(-px(size, scale), 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, 0, face.as_ptr())
    }

    unsafe fn draw_text(hdc: HDC, text: &str, mut r: RECT, flags: u32) {
        let w: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(hdc, w.as_ptr(), w.len() as i32, &mut r, flags | DT_NOPREFIX);
    }

    unsafe fn paint(h: HWND) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(h, &mut ps);
        if let Some(o) = OPEN.lock().unwrap().as_ref() {
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(h, &mut r);
            let mem = CreateCompatibleDC(hdc);
            let bmp = CreateCompatibleBitmap(hdc, r.right, r.bottom);
            let old_bmp = SelectObject(mem, bmp);
            let p = palette(o.notice.dark);
            let s = o.scale;

            let bg = CreateSolidBrush(p.card);
            FillRect(mem, &r, bg);
            DeleteObject(bg);
            SetBkMode(mem, TRANSPARENT as i32);

            let left = o.notice.seconds.saturating_sub(o.shown.elapsed().as_secs() as u32).max(1);
            let title = o.notice.title.replace("{n}", &left.to_string());
            let (title_font, body_font, button_font) = (font(15, 600, s), font(13, 400, s), font(13, 600, s));
            let old_font = SelectObject(mem, title_font);
            SetTextColor(mem, p.ink);
            draw_text(mem, &title, RECT { left: px(PAD, s), top: px(PAD, s), right: px(WIDTH - PAD, s), bottom: px(PAD + 24, s) }, DT_SINGLELINE | DT_END_ELLIPSIS);
            SelectObject(mem, body_font);
            SetTextColor(mem, p.muted);
            draw_text(mem, &o.notice.body, RECT { left: px(PAD, s), top: px(PAD + 28, s), right: px(WIDTH - PAD, s), bottom: px(BUTTON_TOP - 6, s) }, DT_WORDBREAK | DT_END_ELLIPSIS);

            SelectObject(mem, button_font);
            let pen = CreatePen(PS_NULL, 0, 0);
            let old_pen = SelectObject(mem, pen);
            for (i, ((_, label), &(l, t, rr, b))) in o.notice.buttons.iter().zip(&o.rects).enumerate() {
                let hot = o.hover == Some(i);
                let brush = CreateSolidBrush(if hot { p.accent } else { p.accent_soft });
                let old_brush = SelectObject(mem, brush);
                RoundRect(mem, l, t, rr + 1, b + 1, px(10, s), px(10, s));
                SelectObject(mem, old_brush);
                DeleteObject(brush);
                SetTextColor(mem, if hot { p.accent_ink } else { p.ink });
                draw_text(mem, label, RECT { left: l, top: t, right: rr, bottom: b }, DT_SINGLELINE | DT_CENTER | DT_VCENTER);
            }
            SelectObject(mem, old_pen);
            DeleteObject(pen);
            SelectObject(mem, old_font);
            for f in [title_font, body_font, button_font] {
                DeleteObject(f);
            }

            BitBlt(hdc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
            SelectObject(mem, old_bmp);
            DeleteObject(bmp);
            DeleteDC(mem);
        }
        EndPaint(h, &ps);
    }

    fn xy(l: LPARAM) -> (i32, i32) {
        ((l & 0xffff) as i16 as i32, ((l >> 16) & 0xffff) as i16 as i32)
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_PAINT => {
                paint(h);
                0
            }
            WM_ERASEBKGND => 1,
            WM_TIMER => {
                let expired = OPEN.lock().unwrap().as_ref().is_some_and(|o| o.shown.elapsed().as_secs() > o.notice.seconds as u64 + 5);
                if expired {
                    close();
                } else {
                    InvalidateRect(h, null(), 0);
                }
                0
            }
            WM_MOUSEMOVE => {
                let (x, y) = xy(l);
                let mut guard = OPEN.lock().unwrap();
                if let Some(o) = guard.as_mut() {
                    if !o.tracking {
                        let mut t = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: h, dwHoverTime: 0 };
                        TrackMouseEvent(&mut t);
                        o.tracking = true;
                    }
                    let hover = hit(&o.rects, x, y);
                    if hover != o.hover {
                        o.hover = hover;
                        InvalidateRect(h, null(), 0);
                    }
                }
                0
            }
            WM_MOUSELEAVE => {
                if let Some(o) = OPEN.lock().unwrap().as_mut() {
                    o.tracking = false;
                    o.hover = None;
                }
                InvalidateRect(h, null(), 0);
                0
            }
            WM_SETCURSOR => {
                let hot = OPEN.lock().unwrap().as_ref().is_some_and(|o| o.hover.is_some());
                SetCursor(LoadCursorW(null_mut(), if hot { IDC_HAND } else { IDC_ARROW }));
                1
            }
            WM_LBUTTONUP => {
                let (x, y) = xy(l);
                let choice = OPEN.lock().unwrap().as_ref().and_then(|o| hit(&o.rects, x, y).map(|i| o.notice.buttons[i].0));
                if let Some(choice) = choice {
                    close();
                    if let Some(f) = ON_CHOICE.lock().unwrap().as_ref() {
                        f(choice);
                    }
                }
                0
            }
            _ => DefWindowProcW(h, msg, w, l),
        }
    }

    pub fn on_choice(f: impl Fn(Choice) + Send + 'static) {
        *ON_CHOICE.lock().unwrap() = Some(Box::new(f));
    }

    pub fn show(notice: Notice) {
        close();
        let class = wide("BrightPauseHeadsUp");
        unsafe {
            let inst = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                style: CS_DROPSHADOW,
                lpfnWndProc: Some(proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: inst,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc);

            let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let (mut dx, mut dy) = (96u32, 96u32);
            GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            let scale = dx as f32 / 96.0;
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            GetMonitorInfoW(monitor, &mut info);
            let work = info.rcWork;

            let screen = GetDC(null_mut());
            let f = font(13, 600, scale);
            let old = SelectObject(screen, f);
            let widths: Vec<i32> = notice
                .buttons
                .iter()
                .map(|(_, label)| {
                    let w: Vec<u16> = label.encode_utf16().collect();
                    let mut size = SIZE { cx: 0, cy: 0 };
                    GetTextExtentPoint32W(screen, w.as_ptr(), w.len() as i32, &mut size);
                    size.cx
                })
                .collect();
            SelectObject(screen, old);
            DeleteObject(f);
            ReleaseDC(null_mut(), screen);

            let (w, h) = (px(WIDTH, scale), px(height(!notice.buttons.is_empty()), scale));
            let margin = px(12, scale);
            let title = wide(&notice.title.replace("{n}", &notice.seconds.to_string()));
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                work.right - w - margin,
                work.bottom - h - margin,
                w,
                h,
                null_mut(),
                null_mut(),
                inst,
                null(),
            );
            if hwnd.is_null() {
                return;
            }
            let round: u32 = 2; // DWMWCP_ROUND
            DwmSetWindowAttribute(hwnd, 33, (&round as *const u32).cast(), 4);
            let border = palette(notice.dark).line;
            DwmSetWindowAttribute(hwnd, 34, (&border as *const u32).cast(), 4);
            let rects = button_rects(scale, &widths);
            *OPEN.lock().unwrap() = Some(Open { hwnd: hwnd as usize, notice, shown: Instant::now(), scale, rects, hover: None, tracking: false });
            SetTimer(hwnd, 1, 250, None);
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub fn is_open() -> bool {
        OPEN.lock().unwrap().is_some()
    }

    pub fn close() {
        let open = OPEN.lock().unwrap().take();
        if let Some(o) = open {
            unsafe { DestroyWindow(o.hwnd as HWND) };
        }
    }

    pub fn system_dark() -> bool {
        use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let (key, name) = (wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"), wide("AppsUseLightTheme"));
        let (mut value, mut size) = (1u32, 4u32);
        let ok = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, null_mut(), (&mut value as *mut u32).cast(), &mut size) } == 0;
        ok && value == 0
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn on_choice(_f: impl Fn(Choice) + Send + 'static) {}
    pub fn show(_notice: Notice) {}
    pub fn is_open() -> bool {
        false
    }
    pub fn close() {}
    pub fn system_dark() -> bool {
        false
    }
}

pub use imp::{close, is_open, on_choice, show, system_dark};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_sit_right_aligned_in_order_with_a_minimum_width() {
        let r = button_rects(1.0, &[90, 20]);
        assert_eq!(r[1], (WIDTH - PAD - BUTTON_MIN, BUTTON_TOP, WIDTH - PAD, BUTTON_TOP + BUTTON_HEIGHT));
        assert_eq!(r[0].2, r[1].0 - BUTTON_GAP);
        assert_eq!(r[0].2 - r[0].0, 90 + 2 * BUTTON_TEXT_PAD);
        assert!(r[0].0 >= PAD);
    }

    #[test]
    fn buttons_scale_with_the_screen() {
        let one = button_rects(1.0, &[40]);
        let big = button_rects(1.5, &[60]);
        assert_eq!(big[0].2, px(WIDTH - PAD, 1.5));
        assert_eq!(big[0].3 - big[0].1, px(BUTTON_HEIGHT, 1.5));
        assert!(big[0].2 - big[0].0 > one[0].2 - one[0].0);
    }

    #[test]
    fn clicks_find_the_button_under_them_and_nothing_else() {
        let r = button_rects(1.0, &[60, 30]);
        assert_eq!(hit(&r, r[0].0 + 1, r[0].1 + 1), Some(0));
        assert_eq!(hit(&r, r[1].2 - 1, r[1].3 - 1), Some(1));
        assert_eq!(hit(&r, r[1].2, r[1].1), None);
        assert_eq!(hit(&r, r[0].2 + 1, r[0].1 + 5), None);
        assert_eq!(hit(&r, PAD, PAD), None);
    }

    #[test]
    fn without_buttons_the_notice_is_shorter() {
        assert!(height(false) < height(true));
        assert!(button_rects(1.0, &[]).is_empty());
    }
}
