// The desktop app: the native engine behind Tauri commands, plus a watcher
// that opens designs LLMs drop into the inbox through the MCP server.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{Emitter, Manager};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpenedDesign {
    text: String,
    params: serde_json::Value,
    file_name: String,
}

#[tauri::command(async)]
fn evaluate(design: String, params: String) -> Result<pd_app::Evaluation, String> {
    pd_app::evaluate(&design, &params)
}

#[tauri::command(async)]
fn check_all(design: String, params: String) -> Result<pd_app::Report, String> {
    pd_app::check_all(&design, &params)
}

#[tauri::command(async)]
fn export_to_file(
    design: String,
    params: String,
    format: String,
    path: String,
) -> Result<(), String> {
    let bytes = pd_app::export(&design, &params, &format, None)?;
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
}

fn read_design(path: &Path) -> Result<OpenedDesign, String> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if file_name.to_lowercase().ends_with(".3mf") {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let embedded = pd_app::import_3mf(&bytes)?;
        return Ok(OpenedDesign {
            text: serde_json::to_string_pretty(&embedded["design"]).unwrap(),
            params: embedded
                .get("params")
                .cloned()
                .filter(|p| p.is_object())
                .unwrap_or(serde_json::json!({})),
            file_name,
        });
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(OpenedDesign {
        text,
        params: serde_json::json!({}),
        file_name,
    })
}

#[tauri::command(async)]
fn open_design_file(path: String) -> Result<OpenedDesign, String> {
    read_design(Path::new(&path))
}

/// The `pd` MCP server installed next to the app executable, if present.
#[tauri::command]
fn mcp_server_path() -> Option<String> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = if cfg!(windows) { "pd.exe" } else { "pd" };
    let p = dir.join(name);
    p.exists().then(|| p.to_string_lossy().into_owned())
}

/// Poll the inbox; send each new design to the window, then move it to inbox/opened.
fn watch_inbox(app: tauri::AppHandle) {
    let inbox = pd_mcp::paths::inbox_dir();
    let done = inbox.join("opened");
    let _ = std::fs::create_dir_all(&done);
    loop {
        if let Ok(entries) = std::fs::read_dir(&inbox) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && p.to_string_lossy().ends_with(".pd.json"))
                .collect();
            files.sort();
            for f in files {
                if let Ok(d) = read_design(&f) {
                    let _ = app.emit("inbox-design", d);
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.set_focus();
                    }
                }
                let _ = std::fs::rename(&f, done.join(f.file_name().unwrap()));
            }
        }
        std::thread::sleep(Duration::from_millis(800));
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || watch_inbox(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            evaluate,
            check_all,
            export_to_file,
            open_design_file,
            mcp_server_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running ParametricDesign");
}
