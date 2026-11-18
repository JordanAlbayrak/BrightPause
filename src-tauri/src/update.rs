// Update checks. Nothing here touches the network unless the user presses "Check for updates"
// or has turned on automatic updates (then at most once a day).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

pub const DAY: f64 = 24.0 * 3600.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    // "idle", "checking", "none", "available", "installing", "error" or "unconfigured"
    pub state: String,
    pub version: String,
    pub notes: String,
    pub message: String,
    pub checked_at: f64,
}

impl UpdateState {
    pub fn load(path: &Path) -> UpdateState {
        let saved: UpdateState = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        UpdateState { state: "idle".into(), checked_at: saved.checked_at, ..UpdateState::default() }
    }

    pub fn save(&self, path: &Path) {
        let _ = std::fs::write(path, serde_json::to_string(&serde_json::json!({ "checked_at": self.checked_at })).unwrap());
    }

    pub fn due(&self, now: f64) -> bool {
        self.state != "checking" && self.state != "installing" && self.state != "available" && now - self.checked_at >= DAY
    }
}

pub fn configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

pub async fn check(app: &AppHandle, now: f64) -> UpdateState {
    let mut st = UpdateState { checked_at: now, ..UpdateState::default() };
    if !configured(app) {
        st.state = "unconfigured".into();
        st.message = "Updates are not set up in this build.".into();
        return st;
    }
    match app.updater() {
        Ok(u) => match u.check().await {
            Ok(Some(up)) => {
                st.state = "available".into();
                st.version = up.version.clone();
                st.notes = up.body.clone().unwrap_or_default();
            }
            Ok(None) => st.state = "none".into(),
            Err(e) => {
                st.state = "error".into();
                st.message = e.to_string();
            }
        },
        Err(e) => {
            st.state = "error".into();
            st.message = e.to_string();
        }
    }
    st
}

pub async fn install(app: &AppHandle) -> Result<(), String> {
    let up = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("No update found.")?;
    up.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}

pub fn register(app: &AppHandle) {
    if configured(app) {
        let _ = app.plugin(tauri_plugin_updater::Builder::new().build());
    }
}

pub fn path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("update.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_at_most_once_a_day_and_not_while_busy() {
        let mut s = UpdateState { state: "idle".into(), checked_at: 1000.0, ..UpdateState::default() };
        assert!(!s.due(1000.0 + DAY - 1.0));
        assert!(s.due(1000.0 + DAY));
        s.state = "checking".into();
        assert!(!s.due(1000.0 + 2.0 * DAY));
        s.state = "available".into();
        assert!(!s.due(1000.0 + 2.0 * DAY));
        assert!(UpdateState::default().due(DAY));
    }
}
