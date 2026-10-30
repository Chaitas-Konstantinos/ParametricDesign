//! What the app's UI asks of the engine, as JSON in / JSON (or bytes) out, so
//! the Tauri backend and the WebAssembly build expose exactly the same API.

pub use pd_core::validate::Report;
use pd_core::validate::{validate, ValidateOptions};
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

fn parse_inputs(design: &str, params: &str) -> Result<(Json, pd_core::Overrides), String> {
    let raw: Json =
        serde_json::from_str(design).map_err(|e| format!("The design is not valid JSON: {e}"))?;
    let design = design_from_any(&raw).map_err(|e| e.message)?;
    let params: Json = if params.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(params).map_err(|e| e.to_string())?
    };
    Ok((design, overrides_from_json(Some(&params))))
}

/// Build at the given param values: report (fast checks) plus a mesh per part.
pub fn evaluate(design: &str, params: &str) -> Result<Evaluation, String> {
    let (design, overrides) = parse_inputs(design, params)?;
    let (report, _, model) = validate(
        &design,
        &ValidateOptions {
            overrides,
            extremes: false,
            thorough: true,
        },
    );
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
pub fn check_all(design: &str, params: &str) -> Result<Report, String> {
    let (design, overrides) = parse_inputs(design, params)?;
    Ok(validate(
        &design,
        &ValidateOptions {
            overrides,
            extremes: true,
            thorough: true,
        },
    )
    .0)
}

/// Print file bytes: `format` is "stl" or "3mf"; `part` limits STL to one part.
pub fn export(
    design: &str,
    params: &str,
    format: &str,
    part: Option<&str>,
) -> Result<Vec<u8>, String> {
    let (design_json, overrides) = parse_inputs(design, params)?;
    let (report, d, m) = validate(
        &design_json,
        &ValidateOptions {
            overrides,
            extremes: false,
            thorough: false,
        },
    );
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
                .filter(|p| part.map_or(true, |id| p.id == id))
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
            pd_export::three_mf(
                &m,
                &pd_export::ThreeMfOptions {
                    title: d.name.clone(),
                    bed: d.print.bed.unwrap_or([256.0, 256.0, 256.0]),
                    design: Some(design_json),
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

pub fn measure(design: &str, params: &str, queries: &str) -> Result<Json, String> {
    let (design, overrides) = parse_inputs(design, params)?;
    let (report, _, m) = validate(
        &design,
        &ValidateOptions {
            overrides,
            extremes: false,
            thorough: false,
        },
    );
    let m = m
        .filter(|m| !m.has_errors())
        .ok_or_else(|| report.to_text())?;
    let q: Vec<Json> = serde_json::from_str(queries).unwrap_or_default();
    Ok(Json::Array(pd_core::measure::measure(&m, &q)))
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
        let e = evaluate(&d, r#"{"width": 120}"#).unwrap();
        assert!(e.report.ok);
        assert_eq!(e.meshes.len(), 2);
        assert_eq!(e.report.parts[0].size[0], 124.0);
        let bytes = export(&d, "{}", "3mf", None).unwrap();
        assert!(import_3mf(&bytes).unwrap()["design"]["format"] == "pd/0.1");
    }
}
