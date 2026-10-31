//! What the app's UI asks of the engine, as JSON in / JSON (or bytes) out, so
//! the Tauri backend and the WebAssembly build expose exactly the same API.
//!
//! `opts` is a JSON object: `{"printer": {"bed": [x, y, z], "nozzle": 0.4}}`;
//! the user's printer overrides the design's own print hints.
//!
//! Skill requests are JSON too: `{"optional": [skill ids], "title", "brief",
//! "printer": {"name", "bed", "nozzle"}, "photos": [{"name", "caption", "data"}]}`
//! with photo data as base64 or data URLs.

pub use pd_core::validate::Report;
use pd_core::validate::{validate, Printer, ValidateOptions};
use pd_core::{design_from_any, overrides_from_json, Mesh};
use pd_render::{render_png, RenderOptions, View};
use serde::Serialize;
use serde_json::{json, Value as Json};

#[derive(Serialize)]
pub struct MeshOut {
    pub id: String,
    pub name: Option<String>,
    pub color: String,
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Serialize)]
pub struct Evaluation {
    pub report: Report,
    pub meshes: Vec<MeshOut>,
}

struct Inputs {
    design: Json,
    overrides: pd_core::Overrides,
    printer: Option<Printer>,
}

fn parse_inputs(design: &str, params: &str, opts: &str) -> Result<Inputs, String> {
    let raw: Json =
        serde_json::from_str(design).map_err(|e| format!("The design is not valid JSON: {e}"))?;
    let design = design_from_any(&raw).map_err(|e| e.message)?;
    let parse = |s: &str| -> Result<Json, String> {
        if s.trim().is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(s).map_err(|e| e.to_string())
        }
    };
    let params = parse(params)?;
    let opts = parse(opts)?;
    Ok(Inputs {
        design,
        overrides: overrides_from_json(Some(&params)),
        printer: Printer::from_json(opts.get("printer")),
    })
}

fn options(i: &Inputs, extremes: bool, thorough: bool) -> ValidateOptions {
    ValidateOptions {
        overrides: i.overrides.clone(),
        printer: i.printer,
        extremes,
        thorough,
    }
}

/// Build at the given param values: report (fast checks) plus a mesh per part.
pub fn evaluate(design: &str, params: &str, opts: &str) -> Result<Evaluation, String> {
    let i = parse_inputs(design, params, opts)?;
    let (report, _, model) = validate(&i.design, &options(&i, false, true));
    let meshes = match model {
        Some(m) if !m.has_errors() => m
            .parts
            .iter()
            .map(|p| {
                let mesh = Mesh::of(&p.solid);
                MeshOut {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    color: p.color.clone(),
                    positions: mesh.positions,
                    indices: mesh.indices,
                }
            })
            .collect(),
        _ => vec![],
    };
    Ok(Evaluation { report, meshes })
}

/// Full check: also builds at every param's min and max.
pub fn check_all(design: &str, params: &str, opts: &str) -> Result<Report, String> {
    let i = parse_inputs(design, params, opts)?;
    Ok(validate(&i.design, &options(&i, true, true)).0)
}

/// Print file bytes: `format` is "stl" or "3mf"; `part` limits STL to one part.
pub fn export(
    design: &str,
    params: &str,
    opts: &str,
    format: &str,
    part: Option<&str>,
) -> Result<Vec<u8>, String> {
    let i = parse_inputs(design, params, opts)?;
    let (report, d, m) = validate(&i.design, &options(&i, false, false));
    let (Some(d), Some(m)) = (d, m) else {
        return Err(report.to_text());
    };
    if m.has_errors() {
        return Err(report.to_text());
    }
    match format {
        "stl" => {
            let parts: Vec<&pd_core::BuiltPart> = m
                .parts
                .iter()
                .filter(|p| part.is_none_or(|id| p.id == id))
                .collect();
            if parts.is_empty() {
                return Err(format!("no part {}", part.unwrap_or("")));
            }
            Ok(pd_export::stl(&parts, pd_export::Pose::Print))
        }
        "3mf" => {
            let thumb = render_png(
                &m,
                &RenderOptions {
                    views: vec![View::Iso],
                    cell: 256,
                    ..Default::default()
                },
            );
            let params: serde_json::Map<String, Json> = m
                .env
                .iter()
                .filter(|(k, _)| d.param(k).is_some())
                .map(|(k, v)| (k.clone(), v.to_json()))
                .collect();
            let bed = i
                .printer
                .map(|p| p.bed)
                .or(d.print.bed)
                .unwrap_or([256.0, 256.0, 256.0]);
            pd_export::three_mf(
                &m,
                &pd_export::ThreeMfOptions {
                    title: d.name.clone(),
                    bed,
                    design: Some(i.design.clone()),
                    params: Some(Json::Object(params)),
                    thumbnail_png: Some(thumb),
                },
            )
        }
        other => Err(format!("unknown format {other}")),
    }
}

/// Design (and saved param values) embedded in a 3MF exported by the app.
pub fn import_3mf(bytes: &[u8]) -> Result<Json, String> {
    pd_export::embedded_design(bytes)
        .ok_or_else(|| "this 3MF was not made by ParametricDesign (no embedded design)".to_string())
}

pub fn measure(design: &str, params: &str, opts: &str, queries: &str) -> Result<Json, String> {
    let i = parse_inputs(design, params, opts)?;
    let (report, _, m) = validate(&i.design, &options(&i, false, false));
    let m = m
        .filter(|m| !m.has_errors())
        .ok_or_else(|| report.to_text())?;
    let q: Vec<Json> = serde_json::from_str(queries).unwrap_or_default();
    Ok(Json::Array(pd_core::measure::measure(&m, &q)))
}

// --- skills --------------------------------------------------------------------

/// The skills: from the skills folder on the desktop, built in in the browser.
fn catalog() -> Result<pd_skills::Catalog, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return pd_library::config::skills_catalog();
    #[cfg(target_arch = "wasm32")]
    return Ok(pd_skills::Catalog::builtin());
}

/// The skill catalog: `{skills: [{id, title, category, core, group, requires,
/// summary, order, thumbnail}], problems: [text], folder}`, where `thumbnail`
/// is a data URL and `folder` the skills folder (null in the browser).
pub fn skills_catalog() -> Result<Json, String> {
    let c = catalog()?;
    let skills: Vec<Json> = c
        .skills
        .iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap();
            v["thumbnail"] = json!(c.thumbnail_data_url(&s.id));
            v
        })
        .collect();
    #[cfg(not(target_arch = "wasm32"))]
    let folder = json!(pd_library::config::skills_dir().0.display().to_string());
    #[cfg(target_arch = "wasm32")]
    let folder = Json::Null;
    Ok(json!({"skills": skills, "problems": c.problems, "folder": folder}))
}

fn bundle_request(request: &str) -> Result<pd_skills::BundleRequest, String> {
    let v: Json = serde_json::from_str(request).map_err(|e| format!("bad request: {e}"))?;
    pd_skills::BundleRequest::from_json(&v)
}

/// The tailored skill as a .zip, with `{name, fileName, starter}`: the starter
/// is what to paste into a chat once the skill is installed.
pub fn skills_bundle(request: &str) -> Result<(Json, Vec<u8>), String> {
    let req = bundle_request(request)?;
    let (name, zip) = catalog()?.bundle_zip(&req)?;
    let info = json!({
        "name": name,
        "fileName": format!("{name}.zip"),
        "starter": pd_skills::skill_starter(&name, &req),
    });
    Ok((info, zip))
}

/// One skill as a .zip.
pub fn skill_zip(id: &str) -> Result<Vec<u8>, String> {
    catalog()?.skill_zip(id)
}

/// Everything as one prompt: `{prompt, fileName, starter}`. The starter is what
/// to send when the prompt goes into the chat as a file (`file_name` if the
/// user picked another name).
pub fn skills_prompt(request: &str, file_name: Option<&str>) -> Result<Json, String> {
    let req = bundle_request(request)?;
    let c = catalog()?;
    let file = match file_name {
        Some(f) => f.to_string(),
        None => format!("{}.md", c.bundle_name(&req)?),
    };
    Ok(json!({
        "prompt": c.prompt(&req)?,
        "starter": pd_skills::prompt_starter(&file, &req),
        "fileName": file,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_example() {
        let d = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/snap-lid-box.pd.json"
        ))
        .unwrap();
        let e = evaluate(&d, r#"{"width": 120}"#, "").unwrap();
        assert!(e.report.ok);
        assert_eq!(e.meshes.len(), 2);
        assert_eq!(e.report.parts[0].size[0], 124.0);
        let bytes = export(&d, "{}", "", "3mf", None).unwrap();
        assert!(import_3mf(&bytes).unwrap()["design"]["format"] == "pd/0.1");
    }

    #[test]
    fn printer_bed_is_used_for_checks() {
        let d = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/vase.pd.json"
        ))
        .unwrap();
        let small = r#"{"printer": {"bed": [60, 60, 60], "nozzle": 0.4}}"#;
        let e = evaluate(&d, "{}", small).unwrap();
        assert!(e
            .report
            .warnings
            .iter()
            .any(|w| w.code == "too_big_for_bed"));
        let e = evaluate(&d, "{}", "").unwrap();
        assert!(!e
            .report
            .warnings
            .iter()
            .any(|w| w.code == "too_big_for_bed"));
    }

    #[test]
    fn skills_api() {
        // the repository's skills, read as-is (nothing written to the user's folder)
        std::env::set_var(
            "PD_SKILLS",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../skills"),
        );
        let cat = skills_catalog().unwrap();
        assert!(cat["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["core"] == true));
        assert_eq!(cat["problems"], json!([]));
        assert!(cat["folder"].as_str().unwrap().ends_with("skills"));
        let req = r#"{"optional": ["connector-pin"], "title": "Long tray", "brief": "Two pieces", "photos": []}"#;
        let (info, zip) = skills_bundle(req).unwrap();
        assert_eq!(info["fileName"], "parametric-design-long-tray.zip");
        assert!(zip.starts_with(b"PK"));
        let p = skills_prompt(req, Some("tray.md")).unwrap();
        assert!(p["prompt"].as_str().unwrap().contains("Long tray"));
        assert!(p["starter"].as_str().unwrap().contains("tray.md"));
        assert!(skills_bundle(r#"{"optional": ["connector-pin", "connector-magnet"]}"#).is_err());
        assert!(skill_zip("multi-part").unwrap().starts_with(b"PK"));
    }
}
