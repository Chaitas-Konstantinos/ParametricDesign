//! Skill content must stay true to the engine: example copies match
//! /examples, and every complete design shown in a skill validates.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.to_string_lossy().ends_with(ext) {
            out.push(p);
        }
    }
}

#[test]
fn skill_examples_are_copies_of_examples() {
    let mut found = vec![];
    files(&root().join("skills"), ".pd.json", &mut found);
    assert!(!found.is_empty());
    for f in found {
        let canonical = root().join("examples").join(f.file_name().unwrap());
        assert!(
            canonical.exists(),
            "{} has no counterpart in /examples",
            f.display()
        );
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            std::fs::read_to_string(&canonical).unwrap(),
            "{} differs from {}; copy it again",
            f.display(),
            canonical.display()
        );
    }
}

#[test]
fn complete_designs_in_skill_text_validate() {
    let mut md = vec![];
    files(&root().join("skills"), ".md", &mut md);
    let mut checked = 0;
    for f in md {
        let text = std::fs::read_to_string(&f).unwrap();
        for block in text.split("```json").skip(1) {
            let body = block.split("```").next().unwrap();
            let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
                assert!(
                    !body.contains("\"format\": \"pd/0.1\""),
                    "a full design in {} is not valid JSON",
                    f.display()
                );
                continue;
            };
            if json.get("format").is_none()
                || json
                    .get("parts")
                    .and_then(|p| p.as_array())
                    .map_or(true, |p| p.is_empty())
            {
                continue;
            }
            // snippets may reference params they don't declare; only judge self-contained ones
            let (report, _, _) = pd_core::validate(
                &json,
                &pd_core::ValidateOptions {
                    extremes: true,
                    thorough: false,
                    ..Default::default()
                },
            );
            let undeclared = report
                .errors
                .iter()
                .all(|e| e.code == "eval_error" && e.message.contains("unknown name"));
            if !report.errors.is_empty() && undeclared {
                continue;
            }
            assert!(
                report.ok,
                "design in {} does not validate:\n{}",
                f.display(),
                report.to_text()
            );
            checked += 1;
        }
    }
    println!("checked {checked} inline designs");
}
