mod content;
mod dim;
mod idle;
mod schedule;
mod settings;
mod stats;
mod update;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

use content::{Item, Rotation};
use schedule::{Event, Kind, Outcome, Phase, Scheduler};
use base64::Engine;
use settings::Settings;
use update::UpdateState;

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

fn label(kind: Kind) -> &'static str {
    match kind {
        Kind::Mini => "Short break",
        Kind::Long => "Movement break",
    }
}

#[derive(Clone, Serialize)]
struct BreakInfo {
    kind: Kind,
    label: String,
    length: f64,
    ends_at_ms: f64,
    item: Item,
    can_postpone: bool,
    can_skip: bool,
    sound: String,
    volume: f32,
    theme: String,
    background: String,
}

struct Inner {
    sched: Scheduler,
    settings: Settings,
    rotation: Rotation,
    current: Option<BreakInfo>,
}

struct TrayItems {
    tray: TrayIcon<Wry>,
    status: MenuItem<Wry>,
    resume: MenuItem<Wry>,
    last_text: String,
    dot: bool,
}

struct AppState {
    inner: Mutex<Inner>,
    tray: Mutex<Option<TrayItems>>,
    update: Mutex<UpdateState>,
    dir: PathBuf,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }
    fn image_path(&self) -> PathBuf {
        self.dir.join("background.jpg")
    }
    fn history_path(&self) -> PathBuf {
        self.dir.join("history.jsonl")
    }
}

fn handle(app: &AppHandle, events: Vec<Event>) {
    let state = app.state::<AppState>();
    for event in events {
        match event {
            Event::HeadsUp { kind, seconds } => {
                if state.inner.lock().unwrap().settings.heads_up {
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("{} in {} seconds", label(kind), seconds))
                        .body("Finish your sentence; BrightPause will take over the screen.")
                        .show();
                }
            }
            Event::BreakStarted { kind, length, can_postpone } => {
                let all_screens = {
                    let mut inner = state.inner.lock().unwrap();
                    let item = inner.rotation.pick(kind);
                    let s = inner.settings.clone();
                    inner.current = Some(BreakInfo {
                        kind,
                        label: item.label().into(),
                        length,
                        ends_at_ms: (now() + length) * 1000.0,
                        item,
                        can_postpone,
                        can_skip: !s.strict,
                        sound: s.sound.clone(),
                        volume: s.volume,
                        theme: s.theme.clone(),
                        background: s.background.clone(),
                    });
                    s.all_screens
                };
                open_break_windows(app, all_screens);
            }
            Event::BreakEnded { kind, outcome } => {
                state.inner.lock().unwrap().current = None;
                close_break_windows(app);
                let _ = stats::append(&state.history_path(), kind, outcome);
            }
            Event::IdleReset | Event::Paused { .. } | Event::Resumed => {}
        }
    }
    update_tray(app);
}

fn open_break_windows(app: &AppHandle, all_screens: bool) {
    let monitors = app.available_monitors().unwrap_or_default();
    let Some(main) = app.primary_monitor().ok().flatten().or_else(|| monitors.first().cloned()) else { return };
    let built = WebviewWindowBuilder::new(app, "break-0", WebviewUrl::App("break.html".into()))
        .title("BrightPause break")
        .additional_browser_args(BROWSER_ARGS)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build();
    if let Ok(w) = built {
        let _ = w.set_position(*main.position());
        let _ = w.set_size(*main.size());
        let _ = w.show();
        let _ = w.set_focus();
    }
    if all_screens {
        let others: Vec<(i32, i32, i32, i32)> = monitors
            .iter()
            .filter(|m| m.position() != main.position())
            .map(|m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32))
            .collect();
        let _ = app.run_on_main_thread(move || dim::open(&others));
    }
}

fn close_break_windows(app: &AppHandle) {
    for (label, w) in app.webview_windows() {
        if label.starts_with("break-") {
            let _ = w.destroy();
        }
    }
    let _ = app.run_on_main_thread(dim::close);
}

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("BrightPause")
        .additional_browser_args(BROWSER_ARGS)
        .inner_size(720.0, 760.0)
        .min_inner_size(520.0, 560.0)
        .center()
        .build();
}

fn status_text(inner: &Inner, t: f64) -> (String, bool) {
    match inner.sched.phase {
        Phase::Working { .. } => {
            let (kind, secs) = inner.sched.seconds_to_next(t).unwrap();
            let when = if secs < 60.0 { "in under a minute".to_string() } else { format!("in {} min", (secs / 60.0).ceil() as u32) };
            (format!("Next: {} {}", label(kind).to_lowercase(), when), false)
        }
        Phase::Break { kind, .. } => (format!("On a {}", label(kind).to_lowercase()), false),
        Phase::Paused { until: Some(until) } => {
            let local = jiff::Timestamp::from_second(until as i64).map(|t| t.to_zoned_datetime(jiff::tz::TimeZone::system()).strftime("%-I:%M %p").to_string()).unwrap_or_default();
            (format!("Paused until {local}"), true)
        }
        Phase::Paused { until: None } => ("Paused".into(), true),
    }
}

fn update_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (text, paused) = status_text(&state.inner.lock().unwrap(), now());
    let dot = state.update.lock().unwrap().state == "available";
    let mut tray = state.tray.lock().unwrap();
    if let Some(t) = tray.as_mut() {
        if t.dot != dot {
            let _ = t.tray.set_icon(Some(tray_icon(app, dot)));
            t.dot = dot;
        }
        if t.last_text != text {
            let _ = t.status.set_text(&text);
            let _ = t.tray.set_tooltip(Some(format!("BrightPause: {text}")));
            let _ = t.resume.set_enabled(paused);
            t.last_text = text;
        }
    }
}

fn tomorrow_morning() -> f64 {
    let now_here = jiff::ZonedDateTime::now();
    let at = (now_here.date() + jiff::Span::new().days(1)).at(7, 0, 0, 0);
    at.to_zoned_datetime(now_here.time_zone().clone()).map(|z| z.timestamp().as_second() as f64).unwrap_or(now() + 12.0 * 3600.0)
}

fn act(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let t = now();
    let events = {
        let mut inner = state.inner.lock().unwrap();
        match id {
            "break-now" => inner.sched.break_now(t),
            "reset" => {
                inner.sched.reset(t);
                vec![]
            }
            "pause-30" => inner.sched.pause(t, Some(t + 30.0 * 60.0)),
            "pause-60" => inner.sched.pause(t, Some(t + 3600.0)),
            "pause-120" => inner.sched.pause(t, Some(t + 7200.0)),
            "pause-morning" => inner.sched.pause(t, Some(tomorrow_morning())),
            "pause-forever" => inner.sched.pause(t, None),
            "resume" => inner.sched.resume(t),
            "skip" | "postpone" if inner.current.as_ref().is_some_and(|c| !c.can_skip) => vec![],
            "skip" => inner.sched.end_break(t, Outcome::Skipped),
            "postpone" => inner.sched.end_break(t, Outcome::Postponed),
            "pause-toggle" if matches!(inner.sched.phase, Phase::Paused { .. }) => inner.sched.resume(t),
            "pause-toggle" => inner.sched.pause(t, Some(t + 3600.0)),
            _ => vec![],
        }
    };
    match id {
        "settings" => {
            let app = app.clone();
            std::thread::spawn(move || open_settings(&app));
        }
        "quit" => app.exit(0),
        _ => {}
    }
    handle(app, events);
}

fn handle_args(app: &AppHandle, args: &[String], second_launch: bool) {
    let mut acted = false;
    for (flag, id) in [("--break-now", "break-now"), ("--settings", "settings"), ("--skip", "skip"), ("--postpone", "postpone"), ("--pause", "pause-forever"), ("--resume", "resume"), ("--quit", "quit")] {
        if args.iter().any(|a| a == flag) {
            let app2 = app.clone();
            std::thread::spawn(move || act(&app2, id));
            acted = true;
        }
    }
    if second_launch && !acted {
        let app2 = app.clone();
        std::thread::spawn(move || open_settings(&app2));
    }
}

fn installed() -> bool {
    std::env::current_exe().map(|p| !p.to_string_lossy().contains("\\target\\")).unwrap_or(false)
}

fn apply_autostart(app: &AppHandle, on: bool) {
    if !installed() {
        return;
    }
    let auto = app.autolaunch();
    let _ = if on { auto.enable() } else { auto.disable() };
}

// Same arguments for every window, as WebView2 shares one environment. They switch off the
// runtime's own background traffic, so the app sends nothing unless the user asks.
// msOneAuthWAM: without it, the runtime's Microsoft-account component signs in to Microsoft
// services in the background whenever a web view is open.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msOneAuthWAM,msForceOneAuthWAM --disable-background-networking --disable-component-update --disable-domain-reliability --disable-sync --no-pings --no-proxy-server";

fn tray_icon(app: &AppHandle, dot: bool) -> tauri::image::Image<'static> {
    let base = app.default_window_icon().unwrap();
    let (w, h) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    if dot {
        let r = w as f32 * 0.24;
        let (cx, cy) = (w as f32 - r - 1.0, h as f32 - r - 1.0);
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d <= r + 1.5 {
                    let px = &mut rgba[((y * w + x) * 4) as usize..][..4];
                    let color = if d <= r { [0xe0, 0x5a, 0x3a, 0xff] } else { [0xff, 0xff, 0xff, 0xff] };
                    px.copy_from_slice(&color);
                }
            }
        }
    }
    tauri::image::Image::new_owned(rgba, w, h)
}

fn start_check(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.update.lock().unwrap().state = "checking".into();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = update::check(&app, now()).await;
        st.save(&update::path(&app));
        *app.state::<AppState>().update.lock().unwrap() = st;
        update_tray(&app);
    });
}

#[tauri::command]
fn check_updates(app: AppHandle) {
    start_check(&app);
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>().update.lock().unwrap().state = "installing".into();
    let result = update::install(&app).await;
    if let Err(e) = &result {
        let state = app.state::<AppState>();
        let mut st = state.update.lock().unwrap();
        st.state = "error".into();
        st.message = e.clone();
    }
    result
}

#[tauri::command]
fn set_background_image(state: tauri::State<AppState>, jpeg_base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(jpeg_base64).map_err(|e| e.to_string())?;
    if bytes.len() > 20 * 1024 * 1024 || !bytes.starts_with(&[0xff, 0xd8]) {
        return Err("That image could not be used.".into());
    }
    std::fs::create_dir_all(&state.dir).map_err(|e| e.to_string())?;
    std::fs::write(state.image_path(), bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_background_image(state: tauri::State<AppState>) -> Option<String> {
    let bytes = std::fs::read(state.image_path()).ok()?;
    Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
fn set_side_picture(app: AppHandle, width: i32, height: i32, rgba_base64: String) -> Result<(), String> {
    let rgba = base64::engine::general_purpose::STANDARD.decode(rgba_base64).map_err(|e| e.to_string())?;
    if width <= 0 || height <= 0 || width > 1024 || height > 1024 || rgba.len() != (width * height * 4) as usize {
        return Err("bad picture size".into());
    }
    let _ = app.run_on_main_thread(move || dim::set_picture(width, height, rgba));
    Ok(())
}

#[tauri::command]
fn get_break(state: tauri::State<AppState>) -> Option<BreakInfo> {
    state.inner.lock().unwrap().current.clone()
}

#[tauri::command]
async fn end_break(app: AppHandle, outcome: String) -> Result<(), String> {
    let id = match outcome.as_str() {
        "skipped" => "skip",
        "postponed" => "postpone",
        "taken" => "taken",
        _ => return Err("unknown outcome".into()),
    };
    if id == "taken" {
        let state = app.state::<AppState>();
        let events = state.inner.lock().unwrap().sched.end_break(now(), Outcome::Taken);
        handle(&app, events);
    } else {
        let allowed = {
            let state = app.state::<AppState>();
            let inner = state.inner.lock().unwrap();
            inner.current.as_ref().map(|c| if id == "skip" { c.can_skip } else { c.can_postpone }).unwrap_or(false)
        };
        if allowed {
            act(&app, id);
        }
    }
    Ok(())
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.inner.lock().unwrap().settings.clone()
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    settings.validate()?;
    let state = app.state::<AppState>();
    settings.save(&state.settings_path()).map_err(|e| e.to_string())?;
    {
        let mut inner = state.inner.lock().unwrap();
        inner.sched.set_timing(settings.timing());
        inner.settings = settings.clone();
    }
    apply_autostart(&app, settings.start_at_login);
    update_tray(&app);
    Ok(settings)
}

#[derive(Serialize)]
struct Status {
    text: String,
    paused: bool,
    stats: stats::Summary,
    update: UpdateState,
}

#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> Status {
    let (text, paused) = status_text(&state.inner.lock().unwrap(), now());
    let update = state.update.lock().unwrap().clone();
    Status { text, paused, stats: stats::summary(&state.history_path(), jiff::ZonedDateTime::now().date()), update }
}

#[tauri::command]
async fn do_action(app: AppHandle, id: String) {
    act(&app, &id);
}

fn build_tray(app: &AppHandle) -> tauri::Result<TrayItems> {
    let status = MenuItem::with_id(app, "status", "Starting...", false, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume breaks", false, None::<&str>)?;
    let more = Submenu::with_items(
        app,
        "Pause for longer",
        true,
        &[
            &MenuItem::with_id(app, "pause-120", "For 2 hours", true, None::<&str>)?,
            &MenuItem::with_id(app, "pause-forever", "Until I resume", true, None::<&str>)?,
        ],
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "break-now", "Take a break now", true, Some("Ctrl+Alt+B"))?,
            &MenuItem::with_id(app, "reset", "Restart the timer", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "pause-30", "Pause for 30 minutes", true, None::<&str>)?,
            &MenuItem::with_id(app, "pause-60", "Pause for 1 hour", true, Some("Ctrl+Alt+P"))?,
            &MenuItem::with_id(app, "pause-morning", "Pause until tomorrow", true, None::<&str>)?,
            &more,
            &resume,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "settings", "Settings and stats...", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit BrightPause", true, None::<&str>)?,
        ],
    )?;
    let tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("BrightPause")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| act(app, event.id.as_ref()))
        .build(app)?;
    Ok(TrayItems { tray, status, resume, last_text: String::new(), dot: false })
}

// The app was first released under this identifier; carry its data over once after a rename.
const OLD_IDENTIFIER: &str = "ca.jordanalbayrak.breather";

fn migrate_old_data(dir: &std::path::Path) {
    let Some(old) = dir.parent().map(|p| p.join(OLD_IDENTIFIER)) else { return };
    let test_build = dir.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".dev"));
    if test_build || old == dir || dir.join("settings.json").exists() || !old.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    for name in ["settings.json", "history.jsonl", "background.jpg"] {
        let _ = std::fs::copy(old.join(name), dir.join(name));
    }
}

const HOTKEYS: [(Code, &str); 3] = [(Code::KeyB, "break-now"), (Code::KeyP, "pause-toggle"), (Code::KeyS, "skip")];

fn register_hotkeys(app: &AppHandle) {
    let hotkey = |code| Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), code);
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some((_, id)) = HOTKEYS.iter().find(|(code, _)| hotkey(*code) == *shortcut) {
                    act(app, id);
                }
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("hotkeys unavailable: {e}");
        return;
    }
    for (code, _) in HOTKEYS {
        if let Err(e) = app.global_shortcut().register(hotkey(code)) {
            eprintln!("hotkey {code:?} not registered: {e}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| handle_args(app, &args, true)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![get_break, end_break, get_settings, save_settings, get_status, do_action, check_updates, install_update, set_background_image, get_background_image, set_side_picture])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label().starts_with("break-") {
                    let app = window.app_handle().clone();
                    let can_skip = app.state::<AppState>().inner.lock().unwrap().current.as_ref().map(|c| c.can_skip);
                    if can_skip == Some(false) {
                        api.prevent_close();
                    } else if can_skip == Some(true) {
                        api.prevent_close();
                        std::thread::spawn(move || act(&app, "skip"));
                    }
                }
            }
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            migrate_old_data(&dir);
            let mut settings = Settings::load(&dir.join("settings.json"));
            let choice = dir.join("autostart-choice");
            if let Ok(c) = std::fs::read_to_string(&choice) {
                settings.start_at_login = c.trim() == "on";
                let _ = settings.save(&dir.join("settings.json"));
                let _ = std::fs::remove_file(&choice);
            }
            let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
            app.manage(AppState {
                inner: Mutex::new(Inner {
                    sched: Scheduler::new(settings.timing(), now()),
                    settings: settings.clone(),
                    rotation: Rotation::new(seed),
                    current: None,
                }),
                tray: Mutex::new(None),
                update: Mutex::new(UpdateState::load(&dir.join("update.json"))),
                dir,
            });
            let ah = app.handle().clone();
            let items = build_tray(&ah)?;
            *app.state::<AppState>().tray.lock().unwrap() = Some(items);
            apply_autostart(&ah, settings.start_at_login);
            register_hotkeys(&ah);
            update::register(&ah);
            let args: Vec<String> = std::env::args().collect();
            handle_args(&ah, &args, false);
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = ah.state::<AppState>();
                let (events, auto) = {
                    let mut inner = state.inner.lock().unwrap();
                    (inner.sched.tick_busy(now(), idle::idle_seconds(), idle::screen_busy()), inner.settings.auto_update)
                };
                handle(&ah, events);
                if auto && state.update.lock().unwrap().due(now()) {
                    start_check(&ah);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building BrightPause");
    app.run(|_app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn pause_until_tomorrow_ends_at_seven_local_time() {
        let at = jiff::Timestamp::from_second(super::tomorrow_morning() as i64).unwrap();
        let local = at.to_zoned_datetime(jiff::tz::TimeZone::system());
        assert_eq!((local.hour(), local.minute()), (7, 0));
        assert_eq!(local.date(), jiff::ZonedDateTime::now().date().tomorrow().unwrap());
    }
}
