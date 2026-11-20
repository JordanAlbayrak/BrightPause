use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::schedule::Timing;

pub const MY_TEXT_CHARS: usize = 140;
pub const MY_TEXTS_PER_KIND: usize = 30;

/// The user's own break texts, shown alongside the built-in ones or instead of them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MyTexts {
    pub short: Vec<String>,
    pub long: Vec<String>,
    pub only_mine: bool,
}

impl MyTexts {
    pub fn validate(&self) -> Result<(), String> {
        for list in [&self.short, &self.long] {
            if list.len() > MY_TEXTS_PER_KIND {
                return Err(format!("Up to {MY_TEXTS_PER_KIND} texts for each kind of break."));
            }
            for text in list {
                if text.trim().is_empty() {
                    return Err("A break text is empty.".into());
                }
                if text.chars().count() > MY_TEXT_CHARS {
                    let start: String = text.chars().take(24).collect();
                    return Err(format!("Keep each text to {MY_TEXT_CHARS} characters: \"{start}...\""));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hotkey {
    pub keys: String,
    pub on: bool,
}

/// Global shortcuts, each with its own key combination and on/off switch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub break_now: Hotkey,
    pub pause: Hotkey,
    pub skip: Hotkey,
}

impl Default for Shortcuts {
    fn default() -> Self {
        let on = |keys: &str| Hotkey { keys: keys.into(), on: true };
        Shortcuts { break_now: on("Ctrl+Alt+B"), pause: on("Ctrl+Alt+P"), skip: on("Ctrl+Alt+S") }
    }
}

impl Shortcuts {
    /// (action id, hotkey) for each shortcut, in menu order.
    pub fn all(&self) -> [(&'static str, &Hotkey); 3] {
        [("break-now", &self.break_now), ("pause-toggle", &self.pause), ("skip", &self.skip)]
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut seen: Vec<String> = Vec::new();
        for (_, h) in self.all() {
            let parts: Vec<&str> = h.keys.split('+').collect();
            let mods = parts.len() - 1;
            let ok = mods >= 1 && parts[..mods].iter().all(|m| ["Ctrl", "Alt", "Shift"].contains(m)) && !parts[mods].is_empty();
            if !ok {
                return Err(format!("Not a shortcut: {}", h.keys));
            }
            if h.on {
                let norm = h.keys.to_lowercase();
                if seen.contains(&norm) {
                    return Err(format!("Two shortcuts use {}.", h.keys));
                }
                seen.push(norm);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mini_every_min: u32,
    pub mini_length_sec: u32,
    pub long_every_nth: u32,
    pub long_length_min: u32,
    pub heads_up: bool,
    pub strict: bool,
    pub allow_postpone: bool,
    pub idle_reset_min: u32,
    pub sound: String,
    pub volume: f32,
    pub start_at_login: bool,
    pub theme: String,
    pub all_screens: bool,
    pub background: String,
    pub auto_update: bool,
    pub language: String,
    pub pause_in_calls: bool,
    pub shortcuts: Shortcuts,
    pub my_texts: MyTexts,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mini_every_min: 20,
            mini_length_sec: 20,
            long_every_nth: 3,
            long_length_min: 5,
            heads_up: true,
            strict: false,
            allow_postpone: true,
            idle_reset_min: 5,
            sound: "chime".into(),
            volume: 0.5,
            start_at_login: true,
            theme: "system".into(),
            all_screens: true,
            background: "theme".into(),
            auto_update: false,
            language: "system".into(),
            pause_in_calls: true,
            shortcuts: Shortcuts::default(),
            my_texts: MyTexts::default(),
        }
    }
}

fn valid_background(b: &str) -> bool {
    match b.split_once(':') {
        None => b == "theme" || b == "image",
        Some(("color", hex)) => hex.len() == 7 && hex.starts_with('#') && hex[1..].chars().all(|c| c.is_ascii_hexdigit()),
        _ => false,
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=240).contains(&self.mini_every_min) {
            return Err("Eye breaks must come every 1 to 240 minutes.".into());
        }
        if !(5..=300).contains(&self.mini_length_sec) {
            return Err("An eye break lasts 5 to 300 seconds.".into());
        }
        if !(1..=10).contains(&self.long_every_nth) {
            return Err("Movement breaks replace every 1st to 10th break.".into());
        }
        if !(1..=60).contains(&self.long_length_min) {
            return Err("A movement break lasts 1 to 60 minutes.".into());
        }
        if !(1..=60).contains(&self.idle_reset_min) {
            return Err("Away time is 1 to 60 minutes.".into());
        }
        if !["chime", "bell", "soft", "none"].contains(&self.sound.as_str()) {
            return Err("Unknown sound.".into());
        }
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            return Err("Unknown theme.".into());
        }
        if self.language != "system" && !crate::i18n::LANGS.contains(&self.language.as_str()) {
            return Err("Unknown language.".into());
        }
        self.shortcuts.validate()?;
        self.my_texts.validate()?;
        if !valid_background(&self.background) {
            return Err("Unknown background.".into());
        }
        if !(0.0..=1.0).contains(&self.volume) {
            return Err("Volume is 0 to 100%.".into());
        }
        Ok(())
    }

    pub fn timing(&self) -> Timing {
        Timing {
            mini_every: self.mini_every_min as f64 * 60.0,
            mini_length: self.mini_length_sec as f64,
            long_every_nth: self.long_every_nth,
            long_length: self.long_length_min as f64 * 60.0,
            idle_reset: self.idle_reset_min as f64 * 60.0,
            postpone_limit: if self.allow_postpone && !self.strict { 2 } else { 0 },
            ..Timing::default()
        }
    }

    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
            .map(|mut s| {
                if !valid_background(&s.background) {
                    s.background = "theme".into();
                }
                if !crate::i18n::LANGS.contains(&s.language.as_str()) {
                    s.language = "system".into();
                }
                s
            })
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_map_to_timing() {
        let s = Settings::default();
        s.validate().unwrap();
        let t = s.timing();
        assert_eq!((t.mini_every, t.mini_length, t.long_every_nth, t.long_length, t.postpone_limit), (1200.0, 20.0, 3, 300.0, 2));
    }

    #[test]
    fn strict_mode_forbids_postpone() {
        let s = Settings { strict: true, ..Settings::default() };
        assert_eq!(s.timing().postpone_limit, 0);
    }

    #[test]
    fn own_texts_are_checked_and_old_files_get_none() {
        let with = |short: Vec<&str>| Settings { my_texts: MyTexts { short: short.into_iter().map(String::from).collect(), ..MyTexts::default() }, ..Settings::default() };
        assert!(with(vec!["Drink some water", "Stand up and stretch"]).validate().is_ok());
        assert!(with(vec!["   "]).validate().is_err());
        assert!(with(vec![&"é".repeat(140)]).validate().is_ok());
        assert!(with(vec![&"é".repeat(141)]).validate().is_err());
        assert!(with(vec!["x"; 31]).validate().is_err());
        let old: Settings = serde_json::from_str(r#"{"mini_every_min": 25}"#).unwrap();
        assert_eq!(old.my_texts, MyTexts::default());
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(Settings { mini_every_min: 0, ..Settings::default() }.validate().is_err());
        assert!(Settings { sound: "horn".into(), ..Settings::default() }.validate().is_err());
        for bad in ["builtin:sea", "color:#12345", "color:#12345g", "photo", ""] {
            assert!(Settings { background: bad.into(), ..Settings::default() }.validate().is_err(), "{bad}");
        }
        for good in ["theme", "image", "color:#1D2a3a"] {
            Settings { background: good.into(), ..Settings::default() }.validate().unwrap();
        }
        assert!(!Settings::default().auto_update);
        let mut s = Settings::default();
        s.shortcuts.skip.keys = "Ctrl+Alt+B".into();
        assert!(s.validate().is_err());
        s.shortcuts.skip.on = false;
        s.validate().unwrap();
        s.shortcuts.pause.keys = "P".into();
        assert!(s.validate().is_err());
        assert!(Settings { language: "it".into(), ..Settings::default() }.validate().is_err());
        Settings { language: "de".into(), ..Settings::default() }.validate().unwrap();
    }

    #[test]
    fn missing_or_broken_file_gives_defaults_and_partial_file_fills_in() {
        let dir = std::env::temp_dir().join(format!("brightpause-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, r#"{"mini_every_min": 30}"#).unwrap();
        assert_eq!(Settings::load(&path).mini_every_min, 30);
        std::fs::write(&path, r#"{"mini_every_min": 30, "background": "builtin:sea"}"#).unwrap();
        let s = Settings::load(&path);
        assert_eq!((s.mini_every_min, s.background.as_str()), (30, "theme"));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        let s = Settings { strict: true, ..Settings::default() };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
