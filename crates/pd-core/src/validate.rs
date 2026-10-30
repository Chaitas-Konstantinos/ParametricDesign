//! Full validation: parse, build at default and at every param's extremes,
//! then run printability checks.

use crate::build::{build, Model};
use crate::checks::{check, CheckOptions};
use crate::design::{parse, Design, ParamKind};
use crate::eval::Overrides;
use crate::expr::{fmt_num, Value};
use crate::issues::{Issue, Severity};
use crate::math::Bbox;
use crate::mesh::Mesh;
use serde::Serialize;
use serde_json::Value as Json;

#[derive(Clone, Debug, Serialize)]
pub struct PartSummary {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub color: String,
    pub size: [f64; 3],
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub volume_mm3: f64,
    pub triangles: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParamSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub value: Json,
    pub default: Json,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub advanced: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Json>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub ok: bool,
    pub name: String,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
    pub parts: Vec<PartSummary>,
    pub params: Vec<ParamSummary>,
}

#[derive(Clone, Debug, Default)]
pub struct ValidateOptions {
    pub overrides: Overrides,
    /// Also build at each numeric param's min and max (and every choice).
    pub extremes: bool,
    /// Run the slower thin-wall / overhang checks.
    pub thorough: bool,
}

pub fn check_options(design: &Design, thorough: bool) -> CheckOptions {
    let mut o = CheckOptions {
        thorough,
        ..Default::default()
    };
    if let Some(n) = design.print.nozzle {
        o.nozzle = n;
    }
    if let Some(b) = design.print.bed {
        o.bed = b;
    }
    o.supports_ok = design.print.supports.unwrap_or(false);
    o
}

pub fn param_summaries(design: &Design, model: Option<&Model>) -> Vec<ParamSummary> {
    design
        .params
        .iter()
        .map(|p| ParamSummary {
            name: p.name.clone(),
            kind: p.kind.as_str(),
            value: model
                .and_then(|m| m.env.get(&p.name))
                .unwrap_or(&p.value)
                .to_json(),
            default: p.value.to_json(),
            min: p.min,
            max: p.max,
            step: p.step.or(if p.kind == ParamKind::Integer {
                Some(1.0)
            } else {
                None
            }),
            unit: p.unit.clone(),
            label: p.label.clone(),
            description: p.description.clone(),
            group: p.group.clone(),
            advanced: p.advanced,
            options: p.options.iter().map(|o| o.to_json()).collect(),
        })
        .collect()
}

pub fn part_summaries(model: &Model) -> Vec<PartSummary> {
    model
        .parts
        .iter()
        .map(|p| {
            let b = Bbox::of(&p.solid);
            let r = |v: f64| (v * 100.0).round() / 100.0;
            let ok = b.is_valid();
            PartSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                size: if ok { b.size().map(r) } else { [0.0; 3] },
                min: if ok { b.min.map(r) } else { [0.0; 3] },
                max: if ok { b.max.map(r) } else { [0.0; 3] },
                volume_mm3: r(p.solid.volume()),
                triangles: Mesh::of(&p.solid).triangle_count(),
            }
        })
        .collect()
}

/// Parse + build at the given values. Returns the design/model when usable.
pub fn analyze(
    json: &Json,
    overrides: &Overrides,
    thorough: bool,
) -> (Option<Design>, Option<Model>, Vec<Issue>) {
    let (design, mut issues) = parse(json);
    let Some(design) = design else {
        return (None, None, issues);
    };
    let model = build(&design, overrides);
    issues.extend(model.issues.iter().cloned());
    if !model.has_errors() {
        issues.extend(check(&model, &check_options(&design, thorough)));
    }
    (Some(design), Some(model), issues)
}

fn variants(design: &Design, base: &Overrides) -> Vec<(String, Overrides)> {
    let mut out = vec![];
    for p in &design.params {
        let mut vals: Vec<(Value, &str)> = vec![];
        match p.kind {
            ParamKind::Number | ParamKind::Integer => {
                if let Some(lo) = p.min {
                    vals.push((Value::Num(lo), "min"));
                }
                if let Some(hi) = p.max {
                    vals.push((Value::Num(hi), "max"));
                }
            }
            ParamKind::Boolean => {
                if let Value::Bool(b) = p.value {
                    vals.push((Value::Bool(!b), "other"));
                }
            }
            ParamKind::Choice => vals.extend(p.options.iter().map(|o| (o.clone(), "option"))),
            ParamKind::Color => {}
        }
        for (v, tag) in vals {
            let current = base.get(&p.name).unwrap_or(&p.value);
            if *current == v {
                continue;
            }
            let mut o = base.clone();
            o.insert(p.name.clone(), v.clone());
            let label = match tag {
                "min" | "max" => format!(
                    "{} = {} ({tag})",
                    p.name,
                    fmt_num(match v {
                        Value::Num(n) => n,
                        _ => 0.0,
                    })
                ),
                _ => format!("{} = {}", p.name, v),
            };
            out.push((label, o));
        }
    }
    out.truncate(48);
    out
}

pub fn validate(json: &Json, opts: &ValidateOptions) -> (Report, Option<Design>, Option<Model>) {
    let (design, model, mut issues) = analyze(json, &opts.overrides, opts.thorough);
    if let (Some(d), Some(m)) = (&design, &model) {
        if opts.extremes && !m.has_errors() {
            let seen: Vec<(&'static str, String)> =
                issues.iter().map(|i| (i.code, i.path.clone())).collect();
            let mut extra: Vec<Issue> = vec![];
            for (label, o) in variants(d, &opts.overrides) {
                let vm = build(d, &o);
                let mut found: Vec<Issue> = vm.issues.clone();
                if !vm.has_errors() {
                    found.extend(check(&vm, &check_options(d, false)));
                }
                for mut i in found.into_iter().filter(|i| i.severity == Severity::Error) {
                    if seen.iter().any(|(c, p)| *c == i.code && *p == i.path)
                        || extra.iter().any(|e| e.code == i.code && e.path == i.path)
                    {
                        continue;
                    }
                    i.when = Some(label.clone());
                    i.hint = Some(match i.hint {
                        Some(h) => format!("{h} This only happens at this param value: fix the formula or narrow the param's min/max."),
                        None => "This only happens at this param value: fix the formula or narrow the param's min/max.".into(),
                    });
                    extra.push(i);
                }
            }
            issues.extend(extra);
        }
    }
    let (errors, warnings): (Vec<Issue>, Vec<Issue>) =
        issues.into_iter().partition(|i| i.is_error());
    let report = Report {
        ok: errors.is_empty(),
        name: design.as_ref().map(|d| d.name.clone()).unwrap_or_default(),
        errors,
        warnings,
        parts: model.as_ref().map(part_summaries).unwrap_or_default(),
        params: design
            .as_ref()
            .map(|d| param_summaries(d, model.as_ref()))
            .unwrap_or_default(),
    };
    (report, design, model)
}

impl Report {
    /// Compact text for LLMs and terminals.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        if self.ok {
            s.push_str(&format!(
                "VALID: \"{}\" builds without errors ({} part(s), {} warning(s)).\n",
                self.name,
                self.parts.len(),
                self.warnings.len()
            ));
        } else {
            s.push_str(&format!(
                "NOT VALID: {} error(s), {} warning(s). Fix every error, then validate again.\n",
                self.errors.len(),
                self.warnings.len()
            ));
        }
        if !self.parts.is_empty() {
            s.push_str("\nParts (assembled position, mm):\n");
            for p in &self.parts {
                s.push_str(&format!(
                    "  - {}: {} x {} x {} (x {}..{}, y {}..{}, z {}..{}), {} cm³\n",
                    p.id,
                    fmt_num(p.size[0]),
                    fmt_num(p.size[1]),
                    fmt_num(p.size[2]),
                    fmt_num(p.min[0]),
                    fmt_num(p.max[0]),
                    fmt_num(p.min[1]),
                    fmt_num(p.max[1]),
                    fmt_num(p.min[2]),
                    fmt_num(p.max[2]),
                    fmt_num((p.volume_mm3 / 10.0).round() / 100.0)
                ));
            }
        }
        if !self.errors.is_empty() {
            s.push_str("\nErrors:\n");
            for i in &self.errors {
                s.push_str(&format!("  {}\n", i.to_line().replace('\n', "\n  ")));
            }
        }
        if !self.warnings.is_empty() {
            s.push_str("\nWarnings:\n");
            for i in &self.warnings {
                s.push_str(&format!("  {}\n", i.to_line().replace('\n', "\n  ")));
            }
        }
        s
    }
}
