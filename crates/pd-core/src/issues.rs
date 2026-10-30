//! Errors and warnings, written to be read and fixed by an LLM.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `unknown_field`.
    pub code: &'static str,
    /// JSON path of the offending value, e.g. `parts[1].shape.size`.
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Parameter values under which the problem appears, e.g. `wall = 5 (max)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

impl Issue {
    pub fn error(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            severity: Severity::Error,
            code,
            path: path.into(),
            message: message.into(),
            hint: None,
            when: None,
        }
    }
    pub fn warning(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Issue {
            severity: Severity::Warning,
            code,
            path: path.into(),
            message: message.into(),
            hint: None,
            when: None,
        }
    }
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn hint_opt(mut self, hint: Option<String>) -> Self {
        if hint.is_some() {
            self.hint = hint;
        }
        self
    }
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// One-line human/LLM readable form.
    pub fn to_line(&self) -> String {
        let sev = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
        };
        let mut s = format!(
            "{sev} [{}] {}: {}",
            self.code,
            if self.path.is_empty() {
                "(design)"
            } else {
                &self.path
            },
            self.message
        );
        if let Some(w) = &self.when {
            s.push_str(&format!(" (when {w})"));
        }
        if let Some(h) = &self.hint {
            s.push_str(&format!("\n    fix: {h}"));
        }
        s
    }
}

/// Levenshtein distance between two strings.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The closest candidate to `name`, if it is plausibly a typo.
pub fn suggest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let lname = name.to_lowercase();
    let candidates: Vec<&str> = candidates.into_iter().collect();
    const SYNONYMS: &[(&str, &str)] = &[
        ("diameter", "d"),
        ("dia", "d"),
        ("outer_diameter", "d"),
        ("od", "d"),
        ("radius", "r"),
        ("height", "h"),
        ("inner_diameter", "d_inner"),
        ("thickness", "wall"),
        ("dimensions", "size"),
        ("dims", "size"),
        ("position", "place"),
        ("translate", "place"),
        ("colour", "color"),
    ];
    if let Some((_, t)) = SYNONYMS
        .iter()
        .find(|(s, t)| *s == lname && candidates.contains(t))
    {
        return Some(t.to_string());
    }
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let lc = c.to_lowercase();
        let d = if lc == lname {
            0
        } else {
            edit_distance(&lname, &lc)
        };
        let contains =
            lc.len() >= 3 && lname.len() >= 3 && (lc.contains(&lname) || lname.contains(&lc));
        let limit = (name.len().max(c.len()) / 3).max(2);
        if d <= limit || (contains && name.len() >= 3) {
            let score = if contains { d.min(1) } else { d };
            if best.map_or(true, |(bd, _)| score < bd) {
                best = Some((score, c));
            }
        }
    }
    best.map(|(_, c)| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions() {
        assert_eq!(
            suggest("siez", ["size", "wall", "radius"]),
            Some("size".into())
        );
        assert_eq!(suggest("diameter", ["d", "r", "h"]), Some("d".into()));
        assert_eq!(suggest("width", ["d", "h"]), None);
        assert_eq!(
            suggest("inner_size", ["inner", "outer", "wall"]),
            Some("inner".into())
        );
        assert_eq!(suggest("zzz", ["size", "wall"]), None);
    }
}
