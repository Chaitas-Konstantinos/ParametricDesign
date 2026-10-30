//! WebAssembly bindings: the same operations as the desktop backend, for the
//! browser build of the app and (later) the catalog website.

use wasm_bindgen::prelude::*;

fn js_err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

/// JSON `{report, meshes}`.
#[wasm_bindgen]
pub fn evaluate(design: &str, params: &str) -> Result<String, JsValue> {
    pd_app::evaluate(design, params)
        .map(|e| serde_json::to_string(&e).unwrap())
        .map_err(js_err)
}

/// JSON report, including every param's min and max.
#[wasm_bindgen]
pub fn check_all(design: &str, params: &str) -> Result<String, JsValue> {
    pd_app::check_all(design, params)
        .map(|r| serde_json::to_string(&r).unwrap())
        .map_err(js_err)
}

/// Print file bytes (`export` is a reserved word in JavaScript).
#[wasm_bindgen(js_name = exportFile)]
pub fn export_file(
    design: &str,
    params: &str,
    format: &str,
    part: Option<String>,
) -> Result<Vec<u8>, JsValue> {
    pd_app::export(design, params, format, part.as_deref()).map_err(js_err)
}

#[wasm_bindgen]
pub fn import_3mf(bytes: &[u8]) -> Result<String, JsValue> {
    pd_app::import_3mf(bytes)
        .map(|j| j.to_string())
        .map_err(js_err)
}
