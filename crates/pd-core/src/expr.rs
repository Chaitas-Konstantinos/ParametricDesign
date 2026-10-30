//! A small, safe expression language used for parameter-driven values.
//!
//! Supports numbers, strings, booleans, identifiers (params and derived
//! values), arithmetic, comparisons, logic, `cond ? a : b` and a fixed set of
//! math functions. Angles are in degrees.

use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Str(_) => "text",
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Value> {
        match v {
            serde_json::Value::Number(n) => n.as_f64().map(Value::Num),
            serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
            serde_json::Value::String(s) => Some(Value::Str(s.clone())),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Num(n) => serde_json::json!(n),
            Value::Bool(b) => serde_json::json!(b),
            Value::Str(s) => serde_json::json!(s),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Num(n) => write!(f, "{}", fmt_num(*n)),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "\"{s}\""),
        }
    }
}

pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Lit(Value),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExprError {
    pub message: String,
    pub hint: Option<String>,
}

impl ExprError {
    fn new(message: impl Into<String>) -> Self {
        ExprError {
            message: message.into(),
            hint: None,
        }
    }
    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

pub const FUNCTIONS: &[(&str, &str)] = &[
    ("min(a, b, ...)", "smallest value"),
    ("max(a, b, ...)", "largest value"),
    ("clamp(x, lo, hi)", "x limited to [lo, hi]"),
    ("abs(x)", "absolute value"),
    ("round(x)", "nearest integer"),
    ("floor(x)", "round down"),
    ("ceil(x)", "round up"),
    ("sqrt(x)", "square root"),
    ("pow(a, b)", "a to the power b (same as a ^ b)"),
    ("sin(deg) cos(deg) tan(deg)", "trigonometry in degrees"),
    (
        "asin(x) acos(x) atan(x) atan2(y, x)",
        "inverse trigonometry, result in degrees",
    ),
    (
        "if(cond, a, b)",
        "a when cond is true, otherwise b (same as cond ? a : b)",
    ),
    ("inch(x)", "x inches in mm (x * 25.4)"),
];

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
    Question,
    Colon,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, ExprError> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let save = i;
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                if i < chars.len() && chars[i].is_ascii_digit() {
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    i = save;
                }
            }
            let s: String = chars[start..i].iter().collect();
            let n: f64 = s
                .parse()
                .map_err(|_| ExprError::new(format!("invalid number '{s}'")))?;
            out.push(Tok::Num(n));
            if i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '_') {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let unit: String = chars[start..i].iter().collect();
                return Err(ExprError::new(format!("unexpected '{unit}' right after the number {s}"))
                    .hint("All lengths are millimetres: write plain numbers like 12.5. Use inch(2) for inches; write 2*x for multiplication."));
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != quote {
                i += 1;
            }
            if i >= chars.len() {
                return Err(ExprError::new("unterminated text literal"));
            }
            out.push(Tok::Str(chars[start..i].iter().collect()));
            i += 1;
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        let op2 = match two.as_str() {
            "==" => Some("=="),
            "!=" => Some("!="),
            "<=" => Some("<="),
            ">=" => Some(">="),
            "&&" => Some("&&"),
            "||" => Some("||"),
            "**" => Some("^"),
            _ => None,
        };
        if let Some(op) = op2 {
            out.push(Tok::Op(op));
            i += 2;
            continue;
        }
        let tok = match c {
            '+' => Tok::Op("+"),
            '-' => Tok::Op("-"),
            '*' => Tok::Op("*"),
            '/' => Tok::Op("/"),
            '%' => Tok::Op("%"),
            '^' => Tok::Op("^"),
            '<' => Tok::Op("<"),
            '>' => Tok::Op(">"),
            '!' => Tok::Op("!"),
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '?' => Tok::Question,
            ':' => Tok::Colon,
            '=' => {
                return Err(ExprError::new("single '=' is not allowed in expressions")
                    .hint("Use '==' to compare. Expressions compute a value; they cannot assign."))
            }
            _ => return Err(ExprError::new(format!("unexpected character '{c}'"))),
        };
        out.push(tok);
        i += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Parser (precedence climbing)
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn expect(&mut self, t: Tok, what: &str) -> Result<(), ExprError> {
        match self.next() {
            Some(ref got) if *got == t => Ok(()),
            _ => Err(ExprError::new(format!("expected {what}"))),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ExprError> {
        let cond = self.parse_bin(0)?;
        if self.peek() == Some(&Tok::Question) {
            self.next();
            let a = self.parse_expr()?;
            self.expect(Tok::Colon, "':' in 'cond ? a : b'")?;
            let b = self.parse_expr()?;
            return Ok(Expr::Cond(Box::new(cond), Box::new(a), Box::new(b)));
        }
        Ok(cond)
    }

    fn bin_op(&self) -> Option<(BinOp, u8)> {
        match self.peek()? {
            Tok::Op(op) => Some(match *op {
                "||" => (BinOp::Or, 1),
                "&&" => (BinOp::And, 2),
                "==" => (BinOp::Eq, 3),
                "!=" => (BinOp::Ne, 3),
                "<" => (BinOp::Lt, 4),
                "<=" => (BinOp::Le, 4),
                ">" => (BinOp::Gt, 4),
                ">=" => (BinOp::Ge, 4),
                "+" => (BinOp::Add, 5),
                "-" => (BinOp::Sub, 5),
                "*" => (BinOp::Mul, 6),
                "/" => (BinOp::Div, 6),
                "%" => (BinOp::Rem, 6),
                _ => return None,
            }),
            _ => None,
        }
    }

    fn parse_bin(&mut self, min_prec: u8) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_unary()?;
        while let Some((op, prec)) = self.bin_op() {
            if prec < min_prec {
                break;
            }
            self.next();
            let rhs = self.parse_bin(prec + 1)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ExprError> {
        match self.peek() {
            Some(Tok::Op("-")) => {
                self.next();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            Some(Tok::Op("+")) => {
                self.next();
                self.parse_unary()
            }
            Some(Tok::Op("!")) => {
                self.next();
                Ok(Expr::Not(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_pow(),
        }
    }

    fn parse_pow(&mut self) -> Result<Expr, ExprError> {
        let base = self.parse_primary()?;
        if self.peek() == Some(&Tok::Op("^")) {
            self.next();
            let exp = self.parse_unary()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<Expr, ExprError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Lit(Value::Num(n))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(Value::Str(s))),
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek() == Some(&Tok::Comma) {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(Tok::RParen, "')' to close the function call")?;
                    return Ok(Expr::Call(name, args));
                }
                Ok(match name.as_str() {
                    "true" => Expr::Lit(Value::Bool(true)),
                    "false" => Expr::Lit(Value::Bool(false)),
                    "pi" | "PI" => Expr::Lit(Value::Num(std::f64::consts::PI)),
                    _ => Expr::Var(name),
                })
            }
            Some(Tok::LParen) => {
                let e = self.parse_expr()?;
                self.expect(Tok::RParen, "')'")?;
                Ok(e)
            }
            Some(t) => Err(ExprError::new(format!("unexpected {}", tok_desc(&t)))),
            None => Err(ExprError::new("expression ended unexpectedly")),
        }
    }
}

fn tok_desc(t: &Tok) -> String {
    match t {
        Tok::Num(n) => format!("number {n}"),
        Tok::Str(s) => format!("text \"{s}\""),
        Tok::Ident(s) => format!("name '{s}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::LParen => "'('".into(),
        Tok::RParen => "')'".into(),
        Tok::Comma => "','".into(),
        Tok::Question => "'?'".into(),
        Tok::Colon => "':'".into(),
    }
}

pub fn parse(src: &str) -> Result<Expr, ExprError> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err(ExprError::new("empty expression"));
    }
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_expr()?;
    if p.pos < p.toks.len() {
        return Err(ExprError::new(format!(
            "unexpected {} after the end of the expression",
            tok_desc(&p.toks[p.pos])
        )));
    }
    Ok(e)
}

impl Expr {
    /// Names of all variables referenced by this expression.
    pub fn vars(&self, out: &mut Vec<String>) {
        match self {
            Expr::Lit(_) => {}
            Expr::Var(v) => out.push(v.clone()),
            Expr::Neg(e) | Expr::Not(e) => e.vars(out),
            Expr::Bin(_, a, b) => {
                a.vars(out);
                b.vars(out);
            }
            Expr::Cond(c, a, b) => {
                c.vars(out);
                a.vars(out);
                b.vars(out);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.vars(out)),
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

pub type Env = HashMap<String, Value>;

fn num(v: &Value, ctx: &str) -> Result<f64, ExprError> {
    match v {
        Value::Num(n) => Ok(*n),
        other => Err(ExprError::new(format!(
            "{ctx} needs a number but got {} {other}",
            other.type_name()
        ))),
    }
}

fn truthy(v: &Value) -> Result<bool, ExprError> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Num(n) => Ok(*n != 0.0),
        Value::Str(s) => Err(ExprError::new(format!("text \"{s}\" used as a condition"))
            .hint("Compare it instead, e.g. style == \"round\"")),
    }
}

pub fn eval(e: &Expr, env: &Env) -> Result<Value, ExprError> {
    match e {
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Var(name) => env.get(name).cloned().ok_or_else(|| {
            let names: Vec<&str> = env.keys().map(|s| s.as_str()).collect();
            let mut err = ExprError::new(format!("unknown name '{name}'"));
            if let Some(s) = crate::issues::suggest(name, names.iter().copied()) {
                err = err.hint(format!(
                    "Did you mean '{s}'? Names must be declared in \"params\" or \"derived\"."
                ));
            } else {
                err = err
                    .hint("Declare it in \"params\" (user-adjustable) or \"derived\" (computed).");
            }
            err
        }),
        Expr::Neg(a) => Ok(Value::Num(-num(&eval(a, env)?, "'-'")?)),
        Expr::Not(a) => Ok(Value::Bool(!truthy(&eval(a, env)?)?)),
        Expr::Cond(c, a, b) => {
            if truthy(&eval(c, env)?)? {
                eval(a, env)
            } else {
                eval(b, env)
            }
        }
        Expr::Bin(op, a, b) => {
            let op = *op;
            if op == BinOp::And {
                return Ok(Value::Bool(
                    truthy(&eval(a, env)?)? && truthy(&eval(b, env)?)?,
                ));
            }
            if op == BinOp::Or {
                return Ok(Value::Bool(
                    truthy(&eval(a, env)?)? || truthy(&eval(b, env)?)?,
                ));
            }
            let va = eval(a, env)?;
            let vb = eval(b, env)?;
            match op {
                BinOp::Eq => return Ok(Value::Bool(values_eq(&va, &vb))),
                BinOp::Ne => return Ok(Value::Bool(!values_eq(&va, &vb))),
                _ => {}
            }
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Rem => "%",
                BinOp::Pow => "^",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                _ => "?",
            };
            let x = num(&va, &format!("'{sym}'"))?;
            let y = num(&vb, &format!("'{sym}'"))?;
            Ok(match op {
                BinOp::Add => Value::Num(x + y),
                BinOp::Sub => Value::Num(x - y),
                BinOp::Mul => Value::Num(x * y),
                BinOp::Div => {
                    if y == 0.0 {
                        return Err(ExprError::new("division by zero"));
                    }
                    Value::Num(x / y)
                }
                BinOp::Rem => {
                    if y == 0.0 {
                        return Err(ExprError::new("remainder by zero"));
                    }
                    Value::Num(x % y)
                }
                BinOp::Pow => Value::Num(x.powf(y)),
                BinOp::Lt => Value::Bool(x < y),
                BinOp::Le => Value::Bool(x <= y),
                BinOp::Gt => Value::Bool(x > y),
                BinOp::Ge => Value::Bool(x >= y),
                _ => unreachable!(),
            })
        }
        Expr::Call(name, args) => call(name, args, env),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

fn call(name: &str, args: &[Expr], env: &Env) -> Result<Value, ExprError> {
    if name == "if" {
        if args.len() != 3 {
            return Err(ExprError::new("if() takes 3 arguments: if(cond, a, b)"));
        }
        return if truthy(&eval(&args[0], env)?)? {
            eval(&args[1], env)
        } else {
            eval(&args[2], env)
        };
    }
    let vals: Vec<f64> = args
        .iter()
        .map(|a| eval(a, env).and_then(|v| num(&v, &format!("{name}()"))))
        .collect::<Result<_, _>>()?;
    let arity = |n: usize| -> Result<(), ExprError> {
        if vals.len() != n {
            Err(ExprError::new(format!(
                "{name}() takes {n} argument(s), got {}",
                vals.len()
            )))
        } else {
            Ok(())
        }
    };
    let deg = std::f64::consts::PI / 180.0;
    let r = match name {
        "min" | "max" => {
            if vals.is_empty() {
                return Err(ExprError::new(format!(
                    "{name}() needs at least one argument"
                )));
            }
            if name == "min" {
                vals.iter().cloned().fold(f64::INFINITY, f64::min)
            } else {
                vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            }
        }
        "clamp" => {
            arity(3)?;
            vals[0].max(vals[1]).min(vals[2])
        }
        "abs" => {
            arity(1)?;
            vals[0].abs()
        }
        "round" => {
            arity(1)?;
            vals[0].round()
        }
        "floor" => {
            arity(1)?;
            vals[0].floor()
        }
        "ceil" => {
            arity(1)?;
            vals[0].ceil()
        }
        "sqrt" => {
            arity(1)?;
            if vals[0] < 0.0 {
                return Err(ExprError::new("sqrt() of a negative number"));
            }
            vals[0].sqrt()
        }
        "pow" => {
            arity(2)?;
            vals[0].powf(vals[1])
        }
        "sin" => {
            arity(1)?;
            (vals[0] * deg).sin()
        }
        "cos" => {
            arity(1)?;
            (vals[0] * deg).cos()
        }
        "tan" => {
            arity(1)?;
            (vals[0] * deg).tan()
        }
        "asin" => {
            arity(1)?;
            vals[0].asin() / deg
        }
        "acos" => {
            arity(1)?;
            vals[0].acos() / deg
        }
        "atan" => {
            arity(1)?;
            vals[0].atan() / deg
        }
        "atan2" => {
            arity(2)?;
            vals[0].atan2(vals[1]) / deg
        }
        "inch" => {
            arity(1)?;
            vals[0] * 25.4
        }
        _ => {
            let known = [
                "min", "max", "clamp", "abs", "round", "floor", "ceil", "sqrt", "pow", "sin",
                "cos", "tan", "asin", "acos", "atan", "atan2", "if", "inch",
            ];
            let mut err = ExprError::new(format!("unknown function '{name}()'"));
            if let Some(s) = crate::issues::suggest(name, known.iter().copied()) {
                err = err.hint(format!("Did you mean '{s}()'?"));
            }
            return Err(err);
        }
    };
    if !r.is_finite() {
        return Err(ExprError::new(format!(
            "{name}() produced an invalid number"
        )));
    }
    Ok(Value::Num(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str, env: &Env) -> Value {
        eval(&parse(s).unwrap(), env).unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        let mut env = Env::new();
        env.insert("width".into(), Value::Num(40.0));
        env.insert("wall".into(), Value::Num(2.0));
        assert_eq!(ev("width + 2*wall", &env), Value::Num(44.0));
        assert_eq!(ev("-2^2", &env), Value::Num(-4.0));
        assert_eq!(ev("(1 + 2) * 3", &env), Value::Num(9.0));
        assert_eq!(ev("max(1, wall, 3) + min(5, 4)", &env), Value::Num(7.0));
        assert_eq!(ev("wall > 1 ? 10 : 20", &env), Value::Num(10.0));
        assert_eq!(ev("if(wall == 2, 1, 0)", &env), Value::Num(1.0));
        assert_eq!(ev("inch(1)", &env), Value::Num(25.4));
        assert_eq!(ev("1e1 + .5", &env), Value::Num(10.5));
    }

    #[test]
    fn strings_and_logic() {
        let mut env = Env::new();
        env.insert("style".into(), Value::Str("round".into()));
        assert_eq!(ev("style == \"round\" && !false", &env), Value::Bool(true));
        assert_eq!(ev("style != 'round' || 1 < 0", &env), Value::Bool(false));
    }

    #[test]
    fn helpful_errors() {
        let mut env = Env::new();
        env.insert("width".into(), Value::Num(1.0));
        let e = eval(&parse("widht * 2").unwrap(), &env).unwrap_err();
        assert!(e.hint.unwrap().contains("width"));
        assert!(parse("10mm").unwrap_err().hint.is_some());
        assert!(parse("a = 1").is_err());
        assert!(parse("1 +").is_err());
    }
}
