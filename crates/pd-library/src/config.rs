//! Per-computer configuration shared by the desktop app and the `pd` MCP
//! server: settings, the inbox where Claude drops designs, and the design
//! currently open in the app. `PD_HOME` overrides the folder (tests, portable
//! installs).

use serde_json::{json, Value as Json};
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    if let Ok(h) = std::env::var("PD_HOME") {
        return PathBuf::from(h);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join("AppData").join("Roaming"))
            .join("ParametricDesign")
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("ParametricDesign")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".config"))
            .join("parametricdesign")
    }
}

fn documents() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    if cfg!(windows) || cfg!(target_os = "macos") {
        home.join("Documents")
    } else {
        home
    }
}

/// Suggested library folder on first start.
pub fn default_library_path() -> PathBuf {
    documents().join("ParametricDesign Library")
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn inbox_dir() -> PathBuf {
    config_dir().join("inbox")
}

pub fn current_path() -> PathBuf {
    config_dir().join("current.json")
}

/// Settings as a JSON object (empty object when there are none yet).
pub fn load_settings() -> Json {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Json>(&s).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| json!({}))
}

pub fn save_settings(settings: &Json) -> Result<(), String> {
    crate::write_json(&settings_path(), settings)
}

/// The library folder the user chose, if any.
pub fn library_path() -> Option<PathBuf> {
    load_settings()
        .get("libraryPath")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

/// The user's printer as `{"bed": [x, y, z], "nozzle": n}`, if set.
pub fn printer() -> Option<Json> {
    load_settings()
        .get("printer")
        .cloned()
        .filter(|p| p.get("bed").is_some())
}

/// Default folder for files exported by the MCP server.
pub fn exports_dir() -> PathBuf {
    match library_path() {
        Some(l) => l.join("exports"),
        None => documents().join("ParametricDesign Exports"),
    }
}

/// Drop an item for the app: `{"kind": "new" | "update", "designId"?, "design", "message"?}`.
pub fn write_inbox(item: &Json) -> Result<PathBuf, String> {
    let dir = inbox_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let name = item
        .get("design")
        .and_then(|d| d.get("meta"))
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("design");
    let path = dir.join(format!("{}-{}.json", crate::now_ms(), crate::slug(name)));
    let mut item = item.clone();
    item["created"] = json!(crate::now_ms());
    crate::write_json(&path, &item)?;
    Ok(path)
}

/// What the app has open right now (written by the app, read by the MCP server).
pub fn read_current() -> Option<Json> {
    std::fs::read_to_string(current_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn write_current(state: &Json) -> Result<(), String> {
    crate::write_json(&current_path(), state)
}
