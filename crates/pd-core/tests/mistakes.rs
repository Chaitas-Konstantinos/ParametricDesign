//! Common LLM mistakes must be caught with a precise, fixable message.

use pd_core::{validate, Report, ValidateOptions};
use serde_json::{json, Value};

fn run(parts: Value, extra: Value) -> Report {
    let mut d = json!({"format": "pd/0.1", "meta": {"name": "t"}, "parts": parts});
    if let Value::Object(m) = extra {
        for (k, v) in m {
            d[k] = v;
        }
    }
    validate(
        &d,
        &ValidateOptions {
            extremes: true,
            thorough: true,
            ..Default::default()
        },
    )
    .0
}

fn has(r: &Report, code: &str, path_part: &str) -> bool {
    r.errors
        .iter()
        .chain(r.warnings.iter())
        .any(|i| i.code == code && i.path.contains(path_part))
}

#[test]
fn reports_all_reference_typos_together() {
    let r = run(
        json!([
            {"id": "box", "shape": {"type": "hollow_box", "inner": [40, 40, 20], "wal": 2}},
            {"id": "lid", "shape": {"type": "lid", "fits": "boxx"}}
        ]),
        json!({}),
    );
    assert!(has(&r, "unknown_field", "parts[0].shape.wal"));
    let fits = r
        .errors
        .iter()
        .find(|i| i.path == "parts[1].shape.fits")
        .expect("fits typo reported in the same round");
    assert!(fits.hint.as_ref().unwrap().contains("\"box\""));
}

#[test]
fn floating_feature_is_an_error() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "box", "size": [20, 20, 5]},
        "add": [{"shape": {"type": "cylinder", "d": 5, "h": 5}, "place": {"at": [0, 0, 30]}}]}]),
        json!({}),
    );
    assert!(!r.ok);
    assert!(has(&r, "disconnected", "parts[0]"));
}

#[test]
fn cut_that_misses_is_reported() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "box", "size": [20, 20, 5]},
        "cut": [{"shape": {"type": "cylinder", "d": 5, "h": 5}, "place": {"at": [0, 0, 10]}}]}]),
        json!({}),
    );
    assert!(has(&r, "cut_no_effect", "parts[0].cut[0]"));
}

#[test]
fn cut_on_a_face_sinks_into_it() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "box", "size": [20, 20, 5]},
        "cut": [{"shape": {"type": "cylinder", "d": 5, "h": 2}, "place": {"on": "top"}},
                {"shape": {"type": "cylinder", "d": 3, "h": 2}, "place": {"on": "front"}}]}]),
        json!({}),
    );
    assert!(r.ok, "{}", r.to_text());
    assert!(r.warnings.is_empty(), "{}", r.to_text());
    // 20*20*5 - pi*2.5^2*2 - pi*1.5^2*2
    let v = r.parts[0].volume_mm3;
    assert!((v - (2000.0 - 39.27 - 14.14)).abs() < 1.5, "volume {v}");
}

#[test]
fn overlapping_parts_are_an_error() {
    let r = run(
        json!([
            {"id": "a", "shape": {"type": "box", "size": [20, 20, 10]}},
            {"id": "b", "shape": {"type": "box", "size": [10, 10, 10]}, "place": {"at": [5, 0, 0]}}
        ]),
        json!({}),
    );
    assert!(has(&r, "parts_overlap", "parts[1]"));
}

#[test]
fn param_extremes_are_checked() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "tube", "d": "d", "wall": "wall", "h": 10}}]),
        json!({"params": {"d": {"value": 20, "min": 4, "max": 40}, "wall": {"value": 2, "min": 1, "max": 3}}}),
    );
    assert!(!r.ok, "d=4 with wall=2 leaves no hole");
    let e = r
        .errors
        .iter()
        .find(|e| e.when.is_some())
        .expect("issue tagged with the param value");
    assert!(
        e.when.as_ref().unwrap().contains("d = 4 (min)"),
        "{:?}",
        e.when
    );
}

#[test]
fn through_hole_over_air_is_explained() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "hollow_box", "inner": [30, 30, 20]},
        "cut": [{"shape": {"type": "screw_hole", "size": "M3", "through": true}, "place": {"on": "top"}}]}]),
        json!({}),
    );
    assert!(has(&r, "no_material", "parts[0].cut[0]"));
}

#[test]
fn through_hole_goes_through_one_wall_only() {
    let r = run(
        json!([{"id": "a", "shape": {"type": "hollow_box", "inner": [30, 30, 20], "wall": 2},
        "cut": [{"shape": {"type": "screw_hole", "size": "M3", "through": true}, "place": {"on": "front", "offset": [0, 0, 5]}}]}]),
        json!({}),
    );
    assert!(r.ok && r.warnings.is_empty(), "{}", r.to_text());
    let full = run(
        json!([{"id": "a", "shape": {"type": "hollow_box", "inner": [30, 30, 20], "wall": 2}}]),
        json!({}),
    );
    let removed = full.parts[0].volume_mm3 - r.parts[0].volume_mm3;
    // one 3.4 mm hole through a 2 mm wall (+0.5 overshoot into the cavity)
    assert!(removed > 15.0 && removed < 22.0, "removed {removed}");
}

#[test]
fn split_that_misses_and_thin_seams_are_errors() {
    let tray = json!([{"id": "t", "shape": {"type": "hollow_box", "outer": [200, 60, 20], "wall": 1.6, "floor": 1.6}}]);
    let r = run(
        tray.clone(),
        json!({"splits": [{"part": "t", "axis": "x", "at": 150}]}),
    );
    assert!(has(&r, "split_misses", "splits[0].at"));
    let r = run(
        tray,
        json!({"splits": [{"part": "t", "axis": "x", "at": 0, "joint": {"type": "dovetail"}}]}),
    );
    assert!(
        has(&r, "joint_no_room", "splits[0].joint"),
        "{}",
        r.to_text()
    );
}

#[test]
fn magnet_and_pin_joints_build() {
    let block = json!([{"id": "b", "shape": {"type": "box", "size": [120, 40, 30]}}]);
    for joint in [
        json!({"type": "magnet", "count": 2}),
        json!({"type": "pin", "count": 3}),
    ] {
        let r = run(
            block.clone(),
            json!({"splits": [{"part": "b", "axis": "x", "at": 10, "names": ["l", "r"], "joint": joint}]}),
        );
        assert!(r.ok, "{}", r.to_text());
        assert!(r.parts.iter().any(|p| p.id == "l") && r.parts.iter().any(|p| p.id == "r"));
    }
}

#[test]
fn lid_must_fit_a_hollow_box() {
    let r = run(
        json!([
            {"id": "a", "shape": {"type": "box", "size": [20, 20, 10]}},
            {"id": "lid", "shape": {"type": "lid", "fits": "a"}}
        ]),
        json!({}),
    );
    assert!(has(&r, "bad_ref", "parts[1].shape.fits"));
}
