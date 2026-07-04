use crate::geometry::Point;

pub(crate) fn points_to_json(points: &[Point]) -> String {
    let mut out = String::from("[");
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("{\"x\":");
        out.push_str(&number_to_json(point.x));
        out.push_str(",\"y\":");
        out.push_str(&number_to_json(point.y));
        out.push('}');
    }
    out.push(']');
    out
}

pub(crate) fn push_json_string(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
}

pub(crate) fn number_to_json(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}
