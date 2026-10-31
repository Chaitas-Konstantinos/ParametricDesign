//! Per-computer configuration shared by the desktop app and the `pd` MCP
//! server: settings, the inbox where Claude drops designs, the design
//! currently open in the app and the design requests prepared for Claude.
//! `PD_HOME` overrides the folder (tests, portable installs).

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

pub fn requests_dir() -> PathBuf {
    config_dir().join("requests")
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

/// The skills folder the app and the MCP server read: `PD_SKILLS` if set (for
/// working on skills, read as-is), else the `skillsPath` setting, else
/// `Documents/ParametricDesign Skills` (`PD_HOME/skills` when `PD_HOME` is set).
/// The second value says whether the built-in skills are kept in sync there.
pub fn skills_dir() -> (PathBuf, bool) {
    if let Some(d) = std::env::var_os("PD_SKILLS").filter(|d| !d.is_empty()) {
        return (PathBuf::from(d), false);
    }
    let set = load_settings()
        .get("skillsPath")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    let dir = set.unwrap_or_else(|| match std::env::var_os("PD_HOME") {
        Some(_) => config_dir().join("skills"),
        None => documents().join("ParametricDesign Skills"),
    });
    (dir, true)
}

/// The skill catalog from the skills folder, read fresh so edits apply at once.
pub fn skills_catalog() -> Result<pd_skills::Catalog, String> {
    match skills_dir() {
        (dir, true) => pd_skills::Catalog::installed(&dir),
        (dir, false) => pd_skills::Catalog::load(Some(&dir)),
    }
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

/// How many design requests are kept (the newest ones).
const KEEP_REQUESTS: usize = 20;

fn request_folders() -> Vec<(u64, PathBuf)> {
    let mut dirs: Vec<(u64, PathBuf)> = std::fs::read_dir(requests_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let stamp = p.file_name()?.to_str()?.split('-').next()?.parse().ok()?;
            p.is_dir().then_some((stamp, p))
        })
        .collect();
    dirs.sort();
    dirs
}

/// Save a design request the user prepared for Claude in the app:
/// `{"title", "brief", "optional": [skill ids], "printer", "photos": [{"name",
/// "caption", "data"}]}`, photo data as base64 or data URLs. The photos are
/// written next to `request.json`. Returns the request's folder.
pub fn write_request(request: &Json) -> Result<PathBuf, String> {
    let title = request
        .get("title")
        .and_then(|t| t.as_str())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or("request");
    let created = crate::now_ms();
    let folder = requests_dir().join(format!("{created}-{}", crate::slug(title)));
    let mut photos = vec![];
    for (i, p) in request
        .get("photos")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .enumerate()
    {
        let data = p
            .get("data")
            .and_then(|d| d.as_str())
            .ok_or_else(|| format!("photo {} has no data", i + 1))?;
        let ext = match data.split_once(";base64,").map(|(m, _)| m) {
            Some("data:image/png") => "png",
            Some("data:image/webp") => "webp",
            Some("data:image/gif") => "gif",
            _ => "jpg",
        };
        let file = format!("photo-{}.{ext}", i + 1);
        crate::write_bytes(&folder.join(&file), &crate::unbase64(data)?)?;
        photos.push(
            json!({"file": file, "caption": p.get("caption").cloned().unwrap_or(Json::Null)}),
        );
    }
    let mut saved = request.clone();
    saved["photos"] = json!(photos);
    saved["created"] = json!(created);
    crate::write_json(&folder.join("request.json"), &saved)?;
    let all = request_folders();
    for (_, old) in all.iter().take(all.len().saturating_sub(KEEP_REQUESTS)) {
        let _ = std::fs::remove_dir_all(old);
    }
    Ok(folder)
}

/// The newest design request: its folder (with the photos) and `request.json`.
pub fn latest_request() -> Option<(PathBuf, Json)> {
    let (_, dir) = request_folders().pop()?;
    let req = std::fs::read_to_string(dir.join("request.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())?;
    Some((dir, req))
}
