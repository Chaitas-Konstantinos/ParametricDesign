//! Turning params + derived values into an environment, and reading typed
//! component fields under that environment.

use crate::design::{join, literal_color, Design, FieldVal, ParamKind, ShapeDef, Val};
use crate::expr::{self, fmt_num, Env, Expr, Value};
use crate::issues::{suggest, Issue};
use std::collections::{BTreeMap, HashMap};

/// Parameter overrides from the UI / tool call: name -> value.
pub type Overrides = HashMap<String, Value>;

pub fn eval_val(v: &Val, env: &Env, path: &str) -> Result<Value, Issue> {
    match v {
        Val::Lit(x) => Ok(x.clone()),
        Val::Expr { expr, src } => expr::eval(expr, env).map_err(|e| {
            Issue::error("eval_error", path, format!("\"{src}\": {}", e.message)).hint_opt(e.hint)
        }),
    }
}

pub fn eval_num(v: &Val, env: &Env, path: &str) -> Result<f64, Issue> {
    match eval_val(v, env, path)? {
        Value::Num(n) if n.is_finite() => Ok(n),
        Value::Num(_) => Err(Issue::error(
            "eval_error",
            path,
            "value is not a finite number",
        )),
        other => Err(Issue::error(
            "wrong_type",
            path,
            format!("expected a number, got {} {other}", other.type_name()),
        )),
    }
}

pub fn eval_bool(v: &Val, env: &Env, path: &str) -> Result<bool, Issue> {
    match eval_val(v, env, path)? {
        Value::Bool(b) => Ok(b),
        Value::Num(n) => Ok(n != 0.0),
        other => Err(Issue::error(
            "wrong_type",
            path,
            format!("expected true/false, got {} {other}", other.type_name()),
        )),
    }
}

pub fn eval_vec3(v: &[Val; 3], env: &Env, path: &str) -> Result<[f64; 3], Issue> {
    Ok([
        eval_num(&v[0], env, &format!("{path}[0]"))?,
        eval_num(&v[1], env, &format!("{path}[1]"))?,
        eval_num(&v[2], env, &format!("{path}[2]"))?,
    ])
}

/// Build the evaluation environment: params (with overrides applied and
/// clamped to their ranges) followed by derived values in order.
pub fn build_env(design: &Design, overrides: &Overrides) -> (Env, Vec<Issue>) {
    let mut env = Env::new();
    let mut issues = Vec::new();
    for p in &design.params {
        let mut v = overrides
            .get(&p.name)
            .cloned()
            .unwrap_or_else(|| p.value.clone());
        match (&p.kind, &v) {
            (ParamKind::Number | ParamKind::Integer, Value::Num(n)) => {
                let mut n = *n;
                if let Some(lo) = p.min {
                    n = n.max(lo);
                }
                if let Some(hi) = p.max {
                    n = n.min(hi);
                }
                if p.kind == ParamKind::Integer {
                    n = n.round();
                }
                v = Value::Num(n);
            }
            (ParamKind::Number | ParamKind::Integer, _)
            | (ParamKind::Boolean, Value::Num(_) | Value::Str(_)) => {
                v = p.value.clone();
            }
            (ParamKind::Choice, _) if !p.options.contains(&v) => v = p.value.clone(),
            (ParamKind::Color, Value::Str(s)) => {
                v = Value::Str(literal_color(s).unwrap_or_else(|| s.clone()))
            }
            _ => {}
        }
        env.insert(p.name.clone(), v);
    }
    for (name, val, path) in &design.derived {
        match eval_val(val, &env, path) {
            Ok(v) => {
                env.insert(name.clone(), v);
            }
            Err(e) => {
                let defined_later = match val {
                    Val::Expr { expr, .. } => {
                        let mut vars = Vec::new();
                        expr.vars(&mut vars);
                        let later: Vec<&String> = vars
                            .iter()
                            .filter(|v| {
                                !env.contains_key(*v)
                                    && design.derived.iter().any(|(n, _, _)| n == *v)
                            })
                            .collect();
                        later.first().map(|s| s.to_string())
                    }
                    _ => None,
                };
                let e = match defined_later {
                    Some(n) => e.hint(format!("\"{n}\" is defined later in \"derived\"; derived values are computed top to bottom, so move \"{n}\" above \"{name}\".")),
                    None => e,
                };
                issues.push(e);
            }
        }
    }
    (env, issues)
}

/// Typed access to a shape's / joint's fields.
pub struct Fields<'a> {
    pub fields: &'a BTreeMap<String, FieldVal>,
    pub path: &'a str,
    pub env: &'a Env,
}

impl<'a> Fields<'a> {
    pub fn of(shape: &'a ShapeDef, env: &'a Env) -> Self {
        Fields {
            fields: &shape.fields,
            path: &shape.path,
            env,
        }
    }

    fn p(&self, name: &str) -> String {
        join(self.path, name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    fn one(&self, name: &str) -> Option<&'a Val> {
        match self.fields.get(name) {
            Some(FieldVal::One(v)) => Some(v),
            _ => None,
        }
    }

    pub fn opt_num(&self, name: &str) -> Result<Option<f64>, Issue> {
        match self.one(name) {
            Some(v) => eval_num(v, self.env, &self.p(name)).map(Some),
            None => Ok(None),
        }
    }

    pub fn num(&self, name: &str, default: f64) -> Result<f64, Issue> {
        Ok(self.opt_num(name)?.unwrap_or(default))
    }

    /// A strictly positive length.
    pub fn len(&self, name: &str) -> Result<f64, Issue> {
        let v = self.opt_num(name)?.ok_or_else(|| {
            Issue::error("missing_field", self.path, format!("missing \"{name}\""))
        })?;
        positive(v, &self.p(name), name)
    }

    /// A positive length that defaults when absent.
    pub fn len_or(&self, name: &str, default: f64) -> Result<f64, Issue> {
        if self.has(name) {
            self.len(name)
        } else {
            Ok(default)
        }
    }

    pub fn opt_len(&self, name: &str) -> Result<Option<f64>, Issue> {
        match self.opt_num(name)? {
            Some(v) => positive(v, &self.p(name), name).map(Some),
            None => Ok(None),
        }
    }

    pub fn non_negative(&self, name: &str, default: f64) -> Result<f64, Issue> {
        let v = self.num(name, default)?;
        if v < 0.0 {
            return Err(Issue::error(
                "bad_value",
                self.p(name),
                format!("\"{name}\" is {} but cannot be negative", fmt_num(v)),
            ));
        }
        Ok(v)
    }

    pub fn int(&self, name: &str, default: i64) -> Result<i64, Issue> {
        Ok(self.num(name, default as f64)?.round() as i64)
    }

    pub fn bool(&self, name: &str, default: bool) -> Result<bool, Issue> {
        match self.one(name) {
            Some(v) => eval_bool(v, self.env, &self.p(name)),
            None => Ok(default),
        }
    }

    pub fn text(&self, name: &str, default: &str, options: &[&str]) -> Result<String, Issue> {
        let Some(v) = self.one(name) else {
            return Ok(default.to_string());
        };
        let path = self.p(name);
        let value = match v {
            Val::Expr {
                expr: Expr::Var(var),
                ..
            } if !self.env.contains_key(var) => {
                let hint =
                    suggest(var, options.iter().copied()).map(|s| format!("Did you mean \"{s}\"?"));
                return Err(Issue::error(
                    "bad_option",
                    path,
                    format!(
                        "\"{var}\" is not a valid option (use one of {})",
                        options.join(", ")
                    ),
                )
                .hint_opt(hint));
            }
            _ => eval_val(v, self.env, &path)?,
        };
        match value {
            Value::Str(s) if options.is_empty() || options.contains(&s.as_str()) => Ok(s),
            other => Err(Issue::error(
                "bad_option",
                path,
                format!("{other} is not one of {}", options.join(", ")),
            )),
        }
    }

    fn list(&self, name: &str) -> Option<&'a Vec<FieldVal>> {
        match self.fields.get(name) {
            Some(FieldVal::List(l)) => Some(l),
            _ => None,
        }
    }

    pub fn opt_nums(&self, name: &str) -> Result<Option<Vec<f64>>, Issue> {
        let Some(l) = self.list(name) else {
            return Ok(None);
        };
        let mut out = Vec::new();
        for (i, it) in l.iter().enumerate() {
            match it {
                FieldVal::One(v) => {
                    out.push(eval_num(v, self.env, &format!("{}[{i}]", self.p(name)))?)
                }
                FieldVal::List(_) => {
                    return Err(Issue::error(
                        "wrong_type",
                        format!("{}[{i}]", self.p(name)),
                        "expected a number",
                    ))
                }
            }
        }
        Ok(Some(out))
    }

    pub fn vec3(&self, name: &str) -> Result<Option<[f64; 3]>, Issue> {
        Ok(self.opt_nums(name)?.map(|v| [v[0], v[1], v[2]]))
    }

    /// A size vector whose components must all be positive.
    pub fn size3(&self, name: &str) -> Result<Option<[f64; 3]>, Issue> {
        let Some(v) = self.vec3(name)? else {
            return Ok(None);
        };
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            positive(
                v[i],
                &format!("{}[{i}]", self.p(name)),
                &format!("{name} {axis}"),
            )?;
        }
        Ok(Some(v))
    }

    pub fn points(&self, name: &str) -> Result<Option<Vec<[f64; 2]>>, Issue> {
        let Some(l) = self.list(name) else {
            return Ok(None);
        };
        let mut out = Vec::new();
        for (i, it) in l.iter().enumerate() {
            let p = format!("{}[{i}]", self.p(name));
            match it {
                FieldVal::List(xy) if xy.len() == 2 => {
                    let mut c = [0.0; 2];
                    for k in 0..2 {
                        match &xy[k] {
                            FieldVal::One(v) => c[k] = eval_num(v, self.env, &format!("{p}[{k}]"))?,
                            _ => return Err(Issue::error("wrong_type", &p, "expected [x, y]")),
                        }
                    }
                    out.push(c);
                }
                _ => return Err(Issue::error("wrong_type", &p, "expected [x, y]")),
            }
        }
        Ok(Some(out))
    }

    pub fn part_ref(&self, name: &str) -> Option<String> {
        match self.one(name) {
            Some(Val::Lit(Value::Str(s))) => Some(s.clone()),
            _ => None,
        }
    }
}

fn positive(v: f64, path: &str, name: &str) -> Result<f64, Issue> {
    if v <= 0.0 {
        return Err(Issue::error("bad_value", path, format!("\"{name}\" is {} but must be greater than 0", fmt_num(v)))
            .hint("Check the expression: sizes must stay positive for every allowed param value (e.g. use max(1, ...))."));
    }
    Ok(v)
}
