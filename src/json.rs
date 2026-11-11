//! Minimal JSON serialisation helpers for the wasm-bindgen wrapper.
//!
//! The raw wasm API returns JSON strings (parsed by `index.cjs`); the core
//! has no serde dependency, so the few shapes needed are written by hand.

use crate::avoid::geomtypes::{Box, Point};

/// A JSON number (`null` for non-finite values, which JSON cannot carry).
pub(crate) fn number(value: f64) -> String {
    if value.is_finite() {
        // Rust's shortest round-trip formatting is valid JSON for finite
        // values (`1e21` -> "1000000000000000000000", `-0` -> "-0").
        value.to_string()
    } else {
        "null".to_string()
    }
}

/// A JSON string literal.
pub(crate) fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 || ch == '\u{2028}' || ch == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", ch as u32))
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// `null` or a JSON string literal.
pub(crate) fn opt_string(value: Option<&str>) -> String {
    match value {
        Some(v) => string(v),
        None => "null".to_string(),
    }
}

/// `{"x":..,"y":..}`
pub(crate) fn point(p: &Point) -> String {
    format!("{{\"x\":{},\"y\":{}}}", number(p.x), number(p.y))
}

/// `[{"x":..,"y":..}, ...]`
pub(crate) fn points(ps: &[Point]) -> String {
    let items: Vec<String> = ps.iter().map(point).collect();
    format!("[{}]", items.join(","))
}

/// `{"left":..,"top":..,"right":..,"bottom":..}` from an upstream `Box`.
pub(crate) fn bbox(b: &Box) -> String {
    format!(
        "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
        number(b.min.x),
        number(b.min.y),
        number(b.max.x),
        number(b.max.y)
    )
}

/// `["a","b",...]`
pub(crate) fn strings<S: AsRef<str>>(values: &[S]) -> String {
    let items: Vec<String> = values.iter().map(|v| string(v.as_ref())).collect();
    format!("[{}]", items.join(","))
}

/// A JSON object from already-serialised `(key, value)` pairs.
pub(crate) fn object(fields: &[(&str, String)]) -> String {
    let items: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{}", string(k), v))
        .collect();
    format!("{{{}}}", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_numbers() {
        assert_eq!(string("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
        assert_eq!(number(f64::NAN), "null");
        assert_eq!(number(f64::INFINITY), "null");
        assert_eq!(number(1.5), "1.5");
        assert_eq!(number(1e21), "1000000000000000000000");
        assert_eq!(points(&[Point::new(1.0, 2.0)]), "[{\"x\":1,\"y\":2}]");
        assert_eq!(strings(&["a", "b"]), "[\"a\",\"b\"]");
        assert_eq!(object(&[("k", number(1.0))]), "{\"k\":1}");
    }
}
