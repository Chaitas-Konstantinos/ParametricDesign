//! Where the app keeps its inbox (designs sent by LLMs) and exports.
//! `PD_HOME` overrides the base folder.

use std::path::PathBuf;

pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("PD_HOME") {
        return PathBuf::from(h);
    }
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    if cfg!(windows) {
        base.join("Documents").join("ParametricDesign")
    } else {
        base.join("ParametricDesign")
    }
}

pub fn inbox_dir() -> PathBuf {
    home().join("inbox")
}

pub fn exports_dir() -> PathBuf {
    home().join("exports")
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').chars().take(48).collect::<String>();
    if out.is_empty() {
        "design".into()
    } else {
        out
    }
}

/// Save a design into the inbox with a unique name; returns its path.
pub fn write_inbox(name_hint: &str, design: &serde_json::Value) -> Result<PathBuf, String> {
    let dir = inbox_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("{}-{stamp}.pd.json", slug(name_hint)));
    std::fs::write(&path, serde_json::to_string_pretty(design).unwrap())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}
