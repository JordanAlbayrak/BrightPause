mod content;
mod dim;
mod i18n;
mod meeting;
mod idle;
mod schedule;
mod settings;
mod sound;
mod stats;
mod update;

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, Wry};
#[cfg(not(feature = "store"))]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;

use content::{Item, Rotation};
use schedule::{Event, Kind, Outcome, Phase, Scheduler};
use base64::Engine;
use settings::Settings;
use update::UpdateState;

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

fn lang_of(settings: &Settings) -> &'static str {
    i18n::resolve(&settings.language)
}

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Mini => "kind.short",
        Kind::Long => "kind.long",
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
    theme: String,
    background: String,
    lang: String,
}

struct Inner {
    sched: Scheduler,
    settings: Settings,
    rotation: Rotation,
    current: Option<BreakInfo>,
    in_call: bool,
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
                let (heads_up, lang) = {
                    let inner = state.inner.lock().unwrap();
                    (inner.settings.heads_up, lang_of(&inner.settings))
                };
                if heads_up {
                    let title = i18n::fmt(lang, "notify.title", &[("kind", &i18n::t(lang, kind_key(kind))), ("n", &seconds.to_string())]);
                    let _ = app.notification().builder().title(title).body(i18n::t(lang, "notify.body")).show();
                }
            }
            Event::BreakStarted { kind, length, can_postpone } => {
                let (all_screens, sound_name, volume) = {
                    let mut inner = state.inner.lock().unwrap();
                    let item = inner.rotation.pick(kind);
                    let s = inner.settings.clone();
                    let lang = lang_of(&s);
                    inner.current = Some(BreakInfo {
                        kind,
                        label: i18n::t(lang, item.label_key()),
                        length,
                        ends_at_ms: (now() + length) * 1000.0,
                        item: i18n::tip(lang, &item),
                        can_postpone,
                        can_skip: !s.strict,
                        theme: s.theme.clone(),
                        background: s.background.clone(),
                        lang: lang.into(),
                    });
                    (s.all_screens, s.sound, s.volume)
                };
                open_break_windows(app, all_screens);
                sound::play(&sound_name, volume);
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

// Source links (target=_blank) go to the default browser, never to a webview popup.
fn open_in_browser(url: tauri::webview::Url, _: tauri::webview::NewWindowFeatures) -> tauri::webview::NewWindowResponse<Wry> {
    if url.scheme() == "https" {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Shell::ShellExecuteW;
            use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
            let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
            let (verb, target) = (wide("open"), wide(url.as_str()));
            ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
        }
    }
    tauri::webview::NewWindowResponse::Deny
}

fn open_break_windows(app: &AppHandle, all_screens: bool) {
    let monitors = app.available_monitors().unwrap_or_default();
    let Some(main) = app.primary_monitor().ok().flatten().or_else(|| monitors.first().cloned()) else { return };
    let built = WebviewWindowBuilder::new(app, "break-0", WebviewUrl::App("break.html".into()))
        .title("BrightPause break")
        .additional_browser_args(BROWSER_ARGS)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .on_new_window(open_in_browser)
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
    let built = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("BrightPause")
        .additional_browser_args(BROWSER_ARGS)
        .inner_size(720.0, 760.0)
        .min_inner_size(520.0, 560.0)
        .center()
        .focused(true)
        .on_new_window(open_in_browser)
        .build();
    if let Ok(w) = built {
        // Windows may refuse focus to a window opened by a background thread; float it above the
        // others for a few seconds so the first-run welcome is seen.
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        if !w.is_focused().unwrap_or(true) {
            let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            let _ = w.set_always_on_top(false);
        });
    }
}

fn status_text(inner: &Inner, t: f64) -> (String, bool) {
    let lang = lang_of(&inner.settings);
    let per_kind = |kind: Kind, mini: &str, long: &str| match kind {
        Kind::Mini => mini.to_string(),
        Kind::Long => long.to_string(),
    };
    match inner.sched.phase {
        Phase::Working { .. } if inner.in_call => (i18n::t(lang, "status.in_call"), false),
        Phase::Working { .. } => {
            let (kind, secs) = inner.sched.seconds_to_next(t).unwrap();
            let when = if secs < 60.0 { i18n::t(lang, "status.soon") } else { i18n::fmt(lang, "status.in_min", &[("n", &((secs / 60.0).ceil() as u32).to_string())]) };
            (i18n::fmt(lang, &per_kind(kind, "status.next_mini", "status.next_long"), &[("when", &when)]), false)
        }
        Phase::Break { kind, .. } => (i18n::t(lang, &per_kind(kind, "status.on_mini", "status.on_long")), false),
        Phase::Paused { until: Some(until) } => {
            let format = i18n::t(lang, "time_format");
            let local = jiff::Timestamp::from_second(until as i64).map(|t| t.to_zoned_datetime(jiff::tz::TimeZone::system()).strftime(&format).to_string()).unwrap_or_default();
            (i18n::fmt(lang, "status.paused_until", &[("time", &local)]), true)
        }
        Phase::Paused { until: None } => (i18n::t(lang, "status.paused"), true),
    }
}

fn update_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (text, paused) = status_text(&state.inner.lock().unwrap(), now());
    let dot = state.update.lock().unwrap().state == "available";
    let mut tray = state.tray.lock().unwrap();
    if let Some(t) = tray.as_mut() {
        if t.dot != dot {
            let _ = t.tray.set_icon(Some(tray_icon(dot)));
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
    #[cfg(feature = "store")]
    {
        let _ = app;
        store_startup(on);
    }
    #[cfg(not(feature = "store"))]
    {
        let auto = app.autolaunch();
        let _ = if on { auto.enable() } else { auto.disable() };
    }
}

// A packaged app may not use the Run key; the package declares a startup task instead
// (TaskId in tools/msix/AppxManifest.xml). Windows refuses to enable it once the user has
// switched it off in Task Manager or Settings.
#[cfg(feature = "store")]
fn store_startup(on: bool) {
    use windows::ApplicationModel::StartupTask;
    let task = StartupTask::GetAsync(&windows::core::HSTRING::from("BrightPauseStartup")).and_then(|op| op.join());
    if let Ok(task) = task {
        let _ = if on { task.RequestEnableAsync().and_then(|op| op.join()).map(|_| ()) } else { task.Disable() };
    }
}

// Same arguments for every window, as WebView2 shares one environment. They switch off the
// runtime's own background traffic, so the app sends nothing unless the user asks.
// msOneAuthWAM: without it, the runtime's Microsoft-account component signs in to Microsoft
// services in the background whenever a web view is open.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msOneAuthWAM,msForceOneAuthWAM --disable-background-networking --disable-component-update --disable-domain-reliability --disable-sync --no-pings --no-proxy-server";

fn tray_icon(dot: bool) -> tauri::image::Image<'static> {
    // Plain ring and pause bars: the sun's rays blur at tray size.
    let base = tauri::include_image!("icons/tray.png");
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
fn get_strings(state: tauri::State<AppState>) -> i18n::Strings {
    i18n::strings(lang_of(&state.inner.lock().unwrap().settings))
}

#[tauri::command]
fn play_sound(name: String, volume: f32) {
    sound::play(&name, volume);
}

// Set on the first run after install: the settings page then opens with a short welcome.
static WELCOME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn take_welcome() -> bool {
    WELCOME.swap(false, std::sync::atomic::Ordering::Relaxed)
}

// While settings records a new key combination, the current shortcuts must not fire.
#[tauri::command]
fn suspend_hotkeys(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    ACTIVE.lock().unwrap().clear();
}

#[tauri::command]
fn resume_hotkeys(app: AppHandle) {
    let shortcuts = app.state::<AppState>().inner.lock().unwrap().settings.shortcuts.clone();
    apply_hotkeys(&app, &shortcuts);
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.inner.lock().unwrap().settings.clone()
}

#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    settings.validate()?;
    let state = app.state::<AppState>();
    let old = state.inner.lock().unwrap().settings.clone();
    if old.shortcuts != settings.shortcuts {
        // Report only keys that are new: one that already failed at startup should not block other changes.
        let was_on = |k: &String| old.shortcuts.all().iter().any(|(_, h)| h.on && &h.keys == k);
        if let Some(keys) = apply_hotkeys(&app, &settings.shortcuts).into_iter().find(|k| !was_on(k)) {
            apply_hotkeys(&app, &old.shortcuts);
            return Err(i18n::fmt(lang_of(&settings), "short.taken", &[("keys", &keys)]));
        }
    }
    settings.save(&state.settings_path()).map_err(|e| e.to_string())?;
    {
        let mut inner = state.inner.lock().unwrap();
        inner.sched.set_timing(settings.timing());
        inner.settings = settings.clone();
    }
    apply_autostart(&app, settings.start_at_login);
    if old.language != settings.language || old.shortcuts != settings.shortcuts {
        rebuild_menu(&app, &settings);
    }
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

type TrayMenu = (Menu<Wry>, MenuItem<Wry>, MenuItem<Wry>);

fn build_menu(app: &AppHandle, settings: &Settings) -> tauri::Result<TrayMenu> {
    let lang = lang_of(settings);
    let tr = |key: &str| i18n::t(lang, key);
    let sc = &settings.shortcuts;
    let accel = |h: &settings::Hotkey| if h.on { Some(h.keys.clone()) } else { None };
    let item = |id: &str, key: &str, accel: Option<String>| MenuItem::with_id(app, id, tr(key), true, accel);
    let status = MenuItem::with_id(app, "status", tr("status.starting"), false, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", tr("tray.resume"), false, None::<&str>)?;
    let more = Submenu::with_items(app, tr("tray.pause_more"), true, &[&item("pause-120", "tray.pause_120", None)?, &item("pause-forever", "tray.pause_forever", None)?])?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &item("break-now", "tray.break_now", accel(&sc.break_now))?,
            &item("reset", "tray.reset", None)?,
            &PredefinedMenuItem::separator(app)?,
            &item("pause-30", "tray.pause_30", None)?,
            &item("pause-60", "tray.pause_60", accel(&sc.pause))?,
            &item("pause-morning", "tray.pause_morning", None)?,
            &more,
            &resume,
            &PredefinedMenuItem::separator(app)?,
            &item("settings", "tray.settings", None)?,
            &item("quit", "tray.quit", None)?,
        ],
    )?;
    Ok((menu, status, resume))
}

fn rebuild_menu(app: &AppHandle, settings: &Settings) {
    let state = app.state::<AppState>();
    let mut tray = state.tray.lock().unwrap();
    if let (Some(t), Ok((menu, status, resume))) = (tray.as_mut(), build_menu(app, settings)) {
        let _ = t.tray.set_menu(Some(menu));
        t.status = status;
        t.resume = resume;
        t.last_text.clear();
    }
}

fn build_tray(app: &AppHandle, settings: &Settings) -> tauri::Result<TrayItems> {
    let (menu, status, resume) = build_menu(app, settings)?;
    let tray = TrayIconBuilder::with_id("main")
        .icon(tray_icon(false))
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

// The shortcuts registered right now, and the action each one runs.
static ACTIVE: Mutex<Vec<(Shortcut, &'static str)>> = Mutex::new(Vec::new());

fn register_hotkeys(app: &AppHandle, shortcuts: &settings::Shortcuts) {
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let id = ACTIVE.lock().unwrap().iter().find(|(s, _)| s == shortcut).map(|(_, id)| *id);
                if let Some(id) = id {
                    act(app, id);
                }
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("hotkeys unavailable: {e}");
        return;
    }
    for keys in apply_hotkeys(app, shortcuts) {
        eprintln!("hotkey {keys} not registered");
    }
}

/// Registers the switched-on shortcuts. Returns the keys that could not be registered,
/// usually because another app already holds them.
fn apply_hotkeys(app: &AppHandle, shortcuts: &settings::Shortcuts) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let (mut active, mut failed) = (Vec::new(), Vec::new());
    for (id, h) in shortcuts.all() {
        if !h.on {
            continue;
        }
        match h.keys.parse::<Shortcut>() {
            Ok(sc) if gs.register(sc).is_ok() => active.push((sc, id)),
            _ => failed.push(h.keys.clone()),
        }
    }
    *ACTIVE.lock().unwrap() = active;
    failed
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| handle_args(app, &args, true)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![take_welcome, play_sound, get_strings, suspend_hotkeys, resume_hotkeys, get_break, end_break, get_settings, save_settings, get_status, do_action, check_updates, install_update, set_background_image, get_background_image, set_side_picture])
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
            let first_run = !dir.join("settings.json").exists();
            let mut settings = Settings::load(&dir.join("settings.json"));
            if first_run {
                let _ = settings.save(&dir.join("settings.json"));
            }
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
                    in_call: false,
                }),
                tray: Mutex::new(None),
                update: Mutex::new(UpdateState::load(&dir.join("update.json"))),
                dir,
            });
            let ah = app.handle().clone();
            let items = build_tray(&ah, &settings)?;
            *app.state::<AppState>().tray.lock().unwrap() = Some(items);
            apply_autostart(&ah, settings.start_at_login);
            register_hotkeys(&ah, &settings.shortcuts);
            update::register(&ah);
            let args: Vec<String> = std::env::args().collect();
            handle_args(&ah, &args, false);
            if first_run {
                let ah2 = ah.clone();
                WELCOME.store(true, std::sync::atomic::Ordering::Relaxed);
                std::thread::spawn(move || open_settings(&ah2));
            }
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = ah.state::<AppState>();
                let (events, auto) = {
                    let mut inner = state.inner.lock().unwrap();
                    let t = now();
                    // Only look for a call when a break is close: the check reads the registry.
                    let near = inner.sched.seconds_to_next(t).is_some_and(|(_, secs)| secs <= 60.0);
                    inner.in_call = near && inner.settings.pause_in_calls && meeting::in_call();
                    let busy = inner.in_call || idle::screen_busy();
                    (inner.sched.tick_busy(t, idle::idle_seconds(), busy), inner.settings.auto_update)
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

    #[test]
    fn status_says_when_a_call_holds_the_break() {
        let settings = super::Settings { language: "en".into(), ..super::Settings::default() };
        let mut inner = super::Inner {
            sched: super::Scheduler::new(settings.timing(), 0.0),
            settings,
            rotation: super::Rotation::new(1),
            current: None,
            in_call: false,
        };
        assert_eq!(super::status_text(&inner, 0.0).0, "Next: short break in 20 min");
        inner.in_call = true;
        assert_eq!(super::status_text(&inner, 0.0).0, "In a call: the break waits until it ends");
        inner.settings.language = "fr".into();
        assert_eq!(super::status_text(&inner, 0.0).0, "En appel : la pause attend la fin");
    }
}
