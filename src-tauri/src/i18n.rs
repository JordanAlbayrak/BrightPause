use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::content::Item;

pub const LANGS: [&str; 4] = ["en", "fr", "es", "de"];

fn tables() -> &'static HashMap<&'static str, Map<String, Value>> {
    static T: OnceLock<HashMap<&'static str, Map<String, Value>>> = OnceLock::new();
    T.get_or_init(|| {
        [
            ("en", include_str!("../i18n/en.json")),
            ("fr", include_str!("../i18n/fr.json")),
            ("es", include_str!("../i18n/es.json")),
            ("de", include_str!("../i18n/de.json")),
        ]
        .into_iter()
        .map(|(lang, text)| (lang, serde_json::from_str(text).expect(lang)))
        .collect()
    })
}

/// The language to show: the one picked in settings, or else the Windows display language.
pub fn resolve(setting: &str) -> &'static str {
    LANGS.iter().find(|l| **l == setting).copied().unwrap_or_else(system)
}

#[cfg(windows)]
fn system() -> &'static str {
    let primary = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match primary {
        0x0c => "fr",
        0x0a => "es",
        0x07 => "de",
        _ => "en",
    }
}

#[cfg(not(windows))]
fn system() -> &'static str {
    "en"
}

fn lookup(lang: &str, key: &str) -> Option<&'static str> {
    tables().get(lang)?.get(key)?.as_str()
}

pub fn t(lang: &str, key: &str) -> String {
    lookup(lang, key).or_else(|| lookup("en", key)).unwrap_or(key).to_string()
}

pub fn fmt(lang: &str, key: &str, args: &[(&str, &str)]) -> String {
    args.iter().fold(t(lang, key), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
}

/// The item with its title and text in `lang`, when a translation exists.
pub fn tip(lang: &str, item: &Item) -> Item {
    let mut item = item.clone();
    if let Some(tr) = tables().get(lang).and_then(|m| m.get("tips")).and_then(|t| t.get(&item.id)) {
        if let (Some(title), Some(text)) = (tr["title"].as_str(), tr["text"].as_str()) {
            item.title = title.into();
            item.text = text.into();
        }
    }
    item
}

#[derive(Serialize)]
pub struct Strings {
    lang: &'static str,
    strings: Map<String, Value>,
    languages: Vec<(&'static str, String)>,
}

/// Every interface string for the page, English filling any gap.
pub fn strings(lang: &'static str) -> Strings {
    let mut strings = tables()["en"].clone();
    for (k, v) in &tables()[lang] {
        if k != "tips" {
            strings.insert(k.clone(), v.clone());
        }
    }
    Strings { lang, strings, languages: LANGS.iter().map(|l| (*l, t(l, "language_name"))).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_key_and_placeholder() {
        let en = &tables()["en"];
        for lang in LANGS {
            let m = &tables()[lang];
            for (k, v) in en {
                let tr = m.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{lang}: missing {k}"));
                for word in v.as_str().unwrap().split('{').skip(1) {
                    let ph = format!("{{{}}}", word.split('}').next().unwrap());
                    assert!(tr.contains(&ph), "{lang}: {k} lacks {ph}");
                }
            }
        }
    }

    #[test]
    fn every_tip_is_translated() {
        for lang in ["fr", "es", "de"] {
            for item in crate::content::all() {
                let tr = tip(lang, item);
                assert_ne!((tr.title.as_str(), tr.text.as_str()), (item.title.as_str(), item.text.as_str()), "{lang}: {}", item.id);
                assert!(tr.text.chars().count() <= 140, "{lang}: {} too long", item.id);
            }
        }
    }

    #[test]
    fn unknown_setting_falls_back_and_formatting_fills_placeholders() {
        assert!(LANGS.contains(&resolve("system")));
        assert_eq!(resolve("fr"), "fr");
        assert_eq!(fmt("en", "status.in_min", &[("n", "5")]), "in 5 min");
        assert_eq!(t("de", "no.such.key"), "no.such.key");
    }
}
