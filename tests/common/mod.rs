//! Golden-fixture harness shared by the fixture tests.
//!
//! `tests/golden/<name>.txt` files were produced by the real C++ libavoid
//! (`tools/oracle/build.sh`): every `router->outputDiagram("output/NAME")`
//! writes `NAME.txt` (with `%.17g` coordinates) and the Router destructor
//! writes `<fixture>-final.txt`.  `Router::output_diagram_text()` produces
//! the same format on the Rust side.
//!
//! Usage from a test file:
//!
//! ```ignore
//! mod common;
//! use common::*;
//! // at an upstream `router->outputDiagram("output/foo")` call:
//! assert_matches_golden(&mut router, "foo");
//! // just before `delete router`:
//! assert_matches_golden(&mut router, "foo-final");
//! ```
//!
//! Comparison: the texts are split into lines and whitespace / `=`
//! separated tokens.  Non-numeric tokens (and the line/token structure)
//! must match exactly; numeric tokens are compared as `f64` and pass if
//! they differ by at most [`GOLDEN_TOLERANCE`].  The maximum absolute
//! difference is reported; a note is printed (stderr) when the result is
//! not bit-identical.

#![allow(dead_code)]

use libavoid_rust::avoid::router::Router;

/// Maximum accepted absolute coordinate difference.
pub const GOLDEN_TOLERANCE: f64 = 1e-9;

/// Result of a successful comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoldenComparison {
    /// Maximum absolute difference over all numeric tokens.
    pub max_abs_diff: f64,
    /// True if the texts are byte-identical.
    pub bit_identical: bool,
}

/// Path of `tests/golden/<name>.txt`.
pub fn golden_path(name: &str) -> String {
    format!("{}/tests/golden/{}.txt", env!("CARGO_MANIFEST_DIR"), name)
}

/// Reads `tests/golden/<name>.<ext>` (panics with a readable message if it
/// does not exist).
pub fn read_golden_file(name: &str, ext: &str) -> String {
    let path = format!("{}/tests/golden/{}.{}", env!("CARGO_MANIFEST_DIR"), name, ext);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read golden file {}: {}", path, e))
}

/// Splits a diagram text into blank-line separated blocks, keeping the
/// starting line number (1-based) of each.
fn blocks(text: &str) -> Vec<(usize, Vec<&str>)> {
    let mut out: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut start = 1;
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                out.push((start, std::mem::take(&mut cur)));
            }
            start = i + 2;
        } else {
            if cur.is_empty() {
                start = i + 1;
            }
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        out.push((start, cur));
    }
    out
}

fn tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || c == '=')
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_num(t: &str) -> Option<f64> {
    // Only accept things that look like C numbers (avoid treating words
    // such as "inf"/"nan" in identifiers specially: they are compared as
    // text unless both sides parse).
    t.parse::<f64>().ok()
}

/// Compares a diagram text against a golden text.  On failure returns a
/// readable description (first differing block, both versions).
pub fn compare_diagram_text(ours: &str, golden: &str) -> Result<GoldenComparison, String> {
    let bit_identical = ours == golden;
    let ob = blocks(ours);
    let gb = blocks(golden);
    let mut max_abs_diff: f64 = 0.0;

    let describe = |idx: usize, why: String| -> String {
        let g = gb.get(idx);
        let o = ob.get(idx);
        let fmt_block = |b: Option<&(usize, Vec<&str>)>| match b {
            None => "    <missing>".to_string(),
            Some((line, lines)) => {
                let mut s = format!("    (from line {})\n", line);
                for l in lines {
                    s.push_str("    ");
                    s.push_str(l);
                    s.push('\n');
                }
                s
            }
        };
        format!(
            "diagram mismatch in block #{}: {}\n--- golden (C++):\n{}--- ours (Rust):\n{}",
            idx + 1,
            why,
            fmt_block(g),
            fmt_block(o)
        )
    };

    for i in 0..ob.len().max(gb.len()) {
        let (o, g) = match (ob.get(i), gb.get(i)) {
            (Some(o), Some(g)) => (o, g),
            _ => return Err(describe(i, "different number of blocks".into())),
        };
        if o.1.len() != g.1.len() {
            return Err(describe(i, format!("{} lines vs golden {}", o.1.len(), g.1.len())));
        }
        for (li, (ol, gl)) in o.1.iter().zip(g.1.iter()).enumerate() {
            let ot = tokens(ol);
            let gt = tokens(gl);
            if ot.len() != gt.len() {
                return Err(describe(i, format!("line {} has different token count", li + 1)));
            }
            for (a, b) in ot.iter().zip(gt.iter()) {
                if a == b {
                    continue;
                }
                match (parse_num(a), parse_num(b)) {
                    (Some(x), Some(y)) => {
                        let d = (x - y).abs();
                        if d.is_nan() || d > GOLDEN_TOLERANCE {
                            return Err(describe(
                                i,
                                format!("line {}: value {} vs golden {} (|diff| = {:e})", li + 1, a, b, d),
                            ));
                        }
                        if d > max_abs_diff {
                            max_abs_diff = d;
                        }
                    }
                    _ => {
                        return Err(describe(i, format!("line {}: token `{}` vs golden `{}`", li + 1, a, b)));
                    }
                }
            }
        }
    }
    Ok(GoldenComparison {
        max_abs_diff,
        bit_identical,
    })
}

/// Compares `text` with `tests/golden/<name>.txt`; panics on mismatch.
pub fn assert_text_matches_golden(text: &str, name: &str) -> GoldenComparison {
    let golden = read_golden_file(name, "txt");
    match compare_diagram_text(text, &golden) {
        Ok(c) => {
            if !c.bit_identical {
                eprintln!(
                    "note: golden {} matched within tolerance but is not bit-identical (max |diff| = {:e})",
                    name, c.max_abs_diff
                );
            }
            c
        }
        Err(msg) => panic!("golden {}: {}", name, msg),
    }
}

/// Compares the router's `outputDiagramText()` with
/// `tests/golden/<name>.txt`; panics on mismatch.
pub fn assert_matches_golden(router: &mut Router, name: &str) -> GoldenComparison {
    let text = router.output_diagram_text();
    assert_text_matches_golden(&text, name)
}

/// Upstream fixture exit code recorded in `tests/golden/<name>.exit`.
pub fn golden_exit_code(name: &str) -> i32 {
    read_golden_file(name, "exit").trim().parse().expect("exit code")
}

/// C `printf("%f", v)`.
pub fn printf_f(v: f64) -> String {
    format!("{:.6}", v)
}
