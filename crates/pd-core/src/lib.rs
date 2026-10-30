//! ParametricDesign core: the `pd/0.1` JSON design format, its expression
//! language, the component library, placement by anchors, splitting with
//! joints, validation and measurement. Geometry runs on `manifold-rust`
//! (pure Rust, so the same code compiles natively and to WebAssembly).

pub mod build;
pub mod checks;
pub mod components;
pub mod design;
pub mod eval;
pub mod expr;
pub mod issues;
pub mod joints;
pub mod math;
pub mod measure;
pub mod mesh;
pub mod spec;
pub mod validate;

pub use build::{build, BuiltPart, Model};
pub use design::{parse, parse_str, Design, FORMAT};
pub use eval::Overrides;
pub use expr::Value;
pub use issues::{Issue, Severity};
pub use mesh::Mesh;
pub use validate::{validate, Printer, Report, ValidateOptions};

/// Convert a JSON object of `{name: value}` into parameter overrides.
pub fn overrides_from_json(v: Option<&serde_json::Value>) -> Overrides {
    let mut o = Overrides::new();
    if let Some(serde_json::Value::Object(m)) = v {
        for (k, val) in m {
            if let Some(x) = Value::from_json(val) {
                o.insert(k.clone(), x);
            }
        }
    }
    o
}

/// Accept a design given either as a JSON object or as a JSON string.
pub fn design_from_any(v: &serde_json::Value) -> Result<serde_json::Value, Issue> {
    match v {
        serde_json::Value::String(s) => serde_json::from_str(s).map_err(|e| {
            Issue::error(
                "json_syntax",
                "",
                format!("design text is not valid JSON: {e}"),
            )
            .hint("Send plain JSON: double-quoted keys, no comments, no trailing commas.")
        }),
        other => Ok(other.clone()),
    }
}
