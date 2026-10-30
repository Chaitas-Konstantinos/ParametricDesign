//! WebAssembly bindings: the same operations as the desktop backend, for the
//! browser build of the app and (later) the catalog website.

use wasm_bindgen::prelude::*;

fn js_err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

/// JSON `{report, meshes}`.
#[wasm_bindgen]
pub fn evaluate(design: &str, params: &str, opts: &str) -> Result<String, JsValue> {
    pd_app::evaluate(design, params, opts)
        .map(|e| serde_json::to_string(&e).unwrap())
        .map_err(js_err)
}

/// JSON report, including every param's min and max.
#[wasm_bindgen]
pub fn check_all(design: &str, params: &str, opts: &str) -> Result<String, JsValue> {
    pd_app::check_all(design, params, opts)
        .map(|r| serde_json::to_string(&r).unwrap())
        .map_err(js_err)
}

/// Print file bytes (`export` is a reserved word in JavaScript).
#[wasm_bindgen(js_name = exportFile)]
pub fn export_file(
    design: &str,
    params: &str,
    opts: &str,
    format: &str,
    part: Option<String>,
) -> Result<Vec<u8>, JsValue> {
    pd_app::export(design, params, opts, format, part.as_deref()).map_err(js_err)
}

#[wasm_bindgen]
pub fn import_3mf(bytes: &[u8]) -> Result<String, JsValue> {
    pd_app::import_3mf(bytes)
        .map(|j| j.to_string())
        .map_err(js_err)
}

/// JSON `[{id, title, core, group, requires, summary}]`.
#[wasm_bindgen]
pub fn skills_catalog() -> Result<String, JsValue> {
    pd_app::skills_catalog()
        .map(|j| j.to_string())
        .map_err(js_err)
}

/// JSON `{name, fileName, starter}` for the tailored skill of a request.
#[wasm_bindgen]
pub fn skills_bundle_info(request: &str) -> Result<String, JsValue> {
    pd_app::skills_bundle(request)
        .map(|(info, _)| info.to_string())
        .map_err(js_err)
}

/// The tailored skill as .zip bytes.
#[wasm_bindgen]
pub fn skills_bundle_zip(request: &str) -> Result<Vec<u8>, JsValue> {
    pd_app::skills_bundle(request)
        .map(|(_, zip)| zip)
        .map_err(js_err)
}

/// One skill as .zip bytes.
#[wasm_bindgen]
pub fn skill_zip(id: &str) -> Result<Vec<u8>, JsValue> {
    pd_app::skill_zip(id).map_err(js_err)
}

/// JSON `{prompt, fileName, starter}`.
#[wasm_bindgen]
pub fn skills_prompt(request: &str, file_name: Option<String>) -> Result<String, JsValue> {
    pd_app::skills_prompt(request, file_name.as_deref())
        .map(|j| j.to_string())
        .map_err(js_err)
}
