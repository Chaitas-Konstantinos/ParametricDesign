// The desktop app: the native engine and the design library behind Tauri
// commands, plus a watcher that brings in designs Claude sends through the
// MCP server.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pd_library::{config, Library};
use serde_json::{json, Value as Json};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

struct AppState {
    lib: Mutex<Option<Library>>,
}

type R<T> = Result<T, String>;

fn with_lib<T>(state: &State<AppState>, f: impl FnOnce(&Library) -> R<T>) -> R<T> {
    let guard = state
        .lib
        .lock()
        .map_err(|_| "library lock poisoned".to_string())?;
    match guard.as_ref() {
        Some(lib) => f(lib),
        None => Err("No library folder is set up yet. Choose one in Settings.".into()),
    }
}

fn parse(design: &str) -> R<Json> {
    serde_json::from_str(design).map_err(|e| format!("The design is not valid JSON: {e}"))
}

// --- engine -------------------------------------------------------------------

#[tauri::command(async)]
fn evaluate(design: String, params: String, opts: String) -> R<pd_app::Evaluation> {
    pd_app::evaluate(&design, &params, &opts)
}

#[tauri::command(async)]
fn check_all(design: String, params: String, opts: String) -> R<pd_app::Report> {
    pd_app::check_all(&design, &params, &opts)
}

#[tauri::command(async)]
fn export_to_file(
    design: String,
    params: String,
    opts: String,
    format: String,
    part: Option<String>,
    path: String,
) -> R<()> {
    let bytes = pd_app::export(&design, &params, &opts, &format, part.as_deref())?;
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write {path}: {e}"))
}

// --- skills and requests for Claude ----------------------------------------------

#[tauri::command(async)]
fn skills_catalog() -> R<Json> {
    pd_app::skills_catalog()
}

/// Opens the skills folder in Explorer (Finder, the file manager), creating it
/// with the built-in skills first if needed.
#[tauri::command(async)]
fn skills_open_folder() -> R<()> {
    let (dir, managed) = pd_library::config::skills_dir();
    if managed {
        pd_skills::sync_builtin(&dir, false)?;
    }
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("cannot open {}: {e}", dir.display()))
}

/// Writes the original built-in skills back; returns how many files changed.
#[tauri::command(async)]
fn skills_restore() -> R<usize> {
    pd_skills::sync_builtin(&pd_library::config::skills_dir().0, true)
}

/// Writes the tailored skill (.zip); returns `{name, fileName, starter}`.
#[tauri::command(async)]
fn skills_export_bundle(request: String, path: String) -> R<Json> {
    let (info, zip) = pd_app::skills_bundle(&request)?;
    std::fs::write(&path, zip).map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(info)
}

#[tauri::command(async)]
fn skills_export_one(id: String, path: String) -> R<()> {
    let zip = pd_app::skill_zip(&id)?;
    std::fs::write(&path, zip).map_err(|e| format!("cannot write {path}: {e}"))
}

/// `{prompt, fileName, starter}`.
#[tauri::command(async)]
fn skills_prompt(request: String) -> R<Json> {
    pd_app::skills_prompt(&request, None)
}

/// Writes the one-prompt file; returns `{prompt, fileName, starter}` for that file name.
#[tauri::command(async)]
fn skills_save_prompt(request: String, path: String) -> R<Json> {
    let name = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let info = pd_app::skills_prompt(&request, name.as_deref())?;
    std::fs::write(&path, info["prompt"].as_str().unwrap_or_default())
        .map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(info)
}

/// Hands a design request to Claude: the MCP server's `get_design_request` reads it.
#[tauri::command(async)]
fn send_design_request(request: Json) -> R<()> {
    config::write_request(&request).map(|_| ())
}

/// The `pd` MCP server installed next to the app executable, if present.
#[tauri::command]
fn mcp_server_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "pd.exe" } else { "pd" };
    let beside = exe.parent()?.join(name);
    if beside.exists() {
        return Some(beside.to_string_lossy().into_owned());
    }
    // running from a checkout: use `cargo build --release -p pd-cli` from the repo
    exe.ancestors()
        .filter(|dir| dir.join("crates").join("pd-cli").exists())
        .map(|dir| dir.join("target").join("release").join(name))
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
}

// --- settings -----------------------------------------------------------------

#[tauri::command]
fn get_settings() -> Json {
    let mut s = config::load_settings();
    s["defaultLibraryPath"] = json!(config::default_library_path().to_string_lossy());
    s
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Json) -> R<()> {
    let mut s = settings;
    if let Some(o) = s.as_object_mut() {
        o.remove("defaultLibraryPath");
    }
    if let Some(path) = s
        .get("libraryPath")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
    {
        let lib = Library::open(path)?;
        *state.lib.lock().map_err(|_| "library lock poisoned")? = Some(lib);
    }
    config::save_settings(&s)
}

/// Tell the MCP server what is open (`{"open": false}` in the library).
#[tauri::command]
fn set_current(state: Json) -> R<()> {
    let mut s = state;
    s["updated"] = json!(pd_library::now_ms());
    config::write_current(&s)
}

// --- library ------------------------------------------------------------------

#[tauri::command(async)]
fn library_list(state: State<AppState>) -> R<Vec<pd_library::Summary>> {
    with_lib(&state, |l| Ok(l.list()))
}

#[tauri::command(async)]
fn library_detail(state: State<AppState>, id: String) -> R<pd_library::Detail> {
    with_lib(&state, |l| l.detail(&id))
}

#[tauri::command(async)]
fn library_read_version(state: State<AppState>, id: String, file: Option<String>) -> R<Json> {
    with_lib(&state, |l| match file {
        Some(f) => l.read_version(&id, &f),
        None => l.current_version(&id),
    })
}

#[tauri::command(async)]
fn library_create(
    state: State<AppState>,
    design: String,
    params: Json,
    source: String,
    message: String,
) -> R<String> {
    let d = parse(&design)?;
    with_lib(&state, |l| l.create(&d, &params, &source, &message, false))
}

#[tauri::command(async)]
fn library_save_version(
    state: State<AppState>,
    id: String,
    design: String,
    params: Json,
    author: String,
    message: String,
    rejected: bool,
) -> R<pd_library::VersionInfo> {
    let d = parse(&design)?;
    with_lib(&state, |l| {
        l.save_version(&id, &d, &params, &author, &message, rejected)
    })
}

#[tauri::command(async)]
fn library_update_meta(
    state: State<AppState>,
    id: String,
    patch: Json,
) -> R<pd_library::DesignMeta> {
    with_lib(&state, |l| l.update_meta(&id, &patch))
}

#[tauri::command(async)]
fn library_set_presets(
    state: State<AppState>,
    id: String,
    presets: Vec<pd_library::Preset>,
) -> R<()> {
    with_lib(&state, |l| l.set_presets(&id, &presets))
}

#[tauri::command(async)]
fn library_set_thumbnail(state: State<AppState>, id: String, png: String) -> R<()> {
    let bytes = pd_library::unbase64(&png)?;
    with_lib(&state, |l| l.set_thumbnail(&id, &bytes))
}

#[tauri::command(async)]
fn library_delete(state: State<AppState>, id: String) -> R<()> {
    with_lib(&state, |l| l.delete_forever(&id))
}

#[tauri::command(async)]
fn library_duplicate(
    state: State<AppState>,
    id: String,
    file: Option<String>,
    name: String,
) -> R<String> {
    with_lib(&state, |l| l.duplicate(&id, file.as_deref(), &name))
}

/// Join `other`'s versions into `id`; `other` goes to the trash.
#[tauri::command(async)]
fn library_combine(state: State<AppState>, id: String, other: String) -> R<pd_library::DesignMeta> {
    with_lib(&state, |l| l.combine(&id, &other))
}

#[tauri::command(async)]
fn library_read_pending(state: State<AppState>, id: String, file: String) -> R<Json> {
    with_lib(&state, |l| l.read_pending(&id, &file))
}

#[tauri::command(async)]
fn library_remove_pending(state: State<AppState>, id: String, file: String) -> R<()> {
    with_lib(&state, |l| l.remove_pending(&id, &file))
}

#[tauri::command(async)]
fn library_collections(state: State<AppState>) -> R<Json> {
    with_lib(&state, |l| Ok(l.collections()))
}

#[tauri::command(async)]
fn library_set_collections(state: State<AppState>, collections: Json) -> R<()> {
    with_lib(&state, |l| l.set_collections(&collections))
}

/// Add a `.pd.json` or a `.3mf` exported by the app to the library.
#[tauri::command(async)]
fn library_import_file(state: State<AppState>, path: String) -> R<String> {
    let p = Path::new(&path);
    let (design, params) = if path.to_lowercase().ends_with(".3mf") {
        let bytes = std::fs::read(p).map_err(|e| format!("cannot read {path}: {e}"))?;
        let embedded = pd_app::import_3mf(&bytes)?;
        (
            embedded["design"].clone(),
            embedded.get("params").cloned().unwrap_or(json!({})),
        )
    } else {
        let text = std::fs::read_to_string(p).map_err(|e| format!("cannot read {path}: {e}"))?;
        (parse(&text)?, json!({}))
    };
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    with_lib(&state, |l| {
        l.create(
            &design,
            &params,
            "import",
            &format!("Imported from {name}"),
            false,
        )
    })
}

const EXAMPLES: &[&str] = &[
    include_str!("../../../../examples/snap-lid-box.pd.json"),
    include_str!("../../../../examples/board-enclosure.pd.json"),
    include_str!("../../../../examples/wall-bracket.pd.json"),
    include_str!("../../../../examples/dovetail-tray.pd.json"),
    include_str!("../../../../examples/vase.pd.json"),
];

#[tauri::command(async)]
fn library_add_examples(state: State<AppState>) -> R<usize> {
    with_lib(&state, |l| {
        let mut n = 0;
        for e in EXAMPLES {
            l.create(&parse(e)?, &json!({}), "example", "Example design", false)?;
            n += 1;
        }
        Ok(n)
    })
}

/// Bring in designs Claude dropped in the inbox; tell the window about them.
fn watch_inbox(app: tauri::AppHandle) {
    loop {
        std::thread::sleep(Duration::from_millis(800));
        let state = app.state::<AppState>();
        let events = match state.lib.lock() {
            Ok(g) => match g.as_ref() {
                Some(lib) => lib.process_inbox(&config::inbox_dir()),
                None => continue,
            },
            Err(_) => continue,
        };
        if !events.is_empty() {
            let _ = app.emit("library-inbox", &events);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }
    }
}

fn main() {
    let lib = config::library_path().and_then(|p| Library::open(p).ok());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            lib: Mutex::new(lib),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || watch_inbox(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            evaluate,
            check_all,
            export_to_file,
            mcp_server_path,
            get_settings,
            save_settings,
            set_current,
            library_list,
            library_detail,
            library_read_version,
            library_create,
            library_save_version,
            library_update_meta,
            library_set_presets,
            library_set_thumbnail,
            library_delete,
            library_duplicate,
            library_combine,
            library_read_pending,
            library_remove_pending,
            library_collections,
            library_set_collections,
            library_import_file,
            library_add_examples,
            skills_catalog,
            skills_open_folder,
            skills_restore,
            skills_export_bundle,
            skills_export_one,
            skills_prompt,
            skills_save_prompt,
            send_design_request
        ])
        .run(tauri::generate_context!())
        .expect("error while running ParametricDesign");
}
