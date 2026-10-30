//! Every reference design in /examples must validate cleanly, including at
//! the extremes of every parameter.

use pd_core::{validate, ValidateOptions};
use std::path::Path;

#[test]
fn examples_validate() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut failures = vec![];
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".pd.json") {
            continue;
        }
        n += 1;
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let (report, _, _) = validate(
            &json,
            &ValidateOptions {
                extremes: true,
                thorough: true,
                ..Default::default()
            },
        );
        println!("== {}\n{}", path.display(), report.to_text());
        if !report.ok || !report.warnings.is_empty() {
            failures.push(path.display().to_string());
        }
    }
    assert!(n > 0);
    assert!(
        failures.is_empty(),
        "examples with errors or warnings: {failures:?}"
    );
}
