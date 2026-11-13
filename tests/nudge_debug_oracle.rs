//! Differential test of upstream's `NUDGE_DEBUG` / `NUDGE_DEBUG_SVG` output
//! (here the `nudge-debug` / `nudge-debug-svg` features) against the real
//! C++.
//!
//! `SVG_MODE=nudge-debug tools/oracle/svg_oracle.sh` records stdout and
//! stderr of every upstream fixture and audit scene that prints nothing
//! itself (`tests/golden-svg/nudge-debug-manifest.txt`).  Each script runs
//! here in a child process (this test binary re-executed for `child`); its
//! stdout between two markers and its stderr (minus the test harness's own
//! `golden-bump`/`golden-malloc` notes) must be byte-identical to the C++,
//! except that upstream prints a `ConnRef` pointer in `Pos: %lX, ...` (the
//! port prints the connector's creation serial there), which is masked as
//! `Pos: <ptr>,` on both sides.
//!
//! `cargo test --release --features nudge-debug,nudge-debug-svg --test nudge_debug_oracle`

#![cfg(all(feature = "nudge-debug", feature = "nudge-debug-svg"))]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;
#[path = "common/sha256.rs"]
mod sha;

use std::collections::BTreeMap;
use std::process::Command;

const CHILD_ENV: &str = "NUDGE_DEBUG_ORACLE_CHILD";
const BEGIN: &str = "\n@@NUDGE-BEGIN@@\n";
const END: &str = "\n@@NUDGE-END@@\n";

/// Runs one script in this process (the child side).
#[test]
fn child() {
    if let Ok(script) = std::env::var(CHILD_ENV) {
        use std::io::Write;
        print!("{}", BEGIN);
        fixture_ops::run_fixture(&script);
        print!("{}", END);
        std::io::stdout().flush().unwrap();
    }
}

fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    let path = format!("{}/tests/golden-svg/nudge-debug-manifest.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let mut out: BTreeMap<String, BTreeMap<String, (String, usize)>> = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks[0] == "script" {
            cur = Some(toks[1].to_string());
            out.entry(toks[1].to_string()).or_default();
            continue;
        }
        let script = cur.as_ref().expect("manifest entry before any script line");
        out.get_mut(script)
            .unwrap()
            .insert(toks[0].to_string(), (toks[1].to_string(), toks[2].parse().unwrap()));
    }
    out
}

/// `Pos: <hex>,` (a pointer upstream, a serial in the port) as `Pos: <ptr>,`.
fn mask_pointers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("Pos: ") {
            if let Some(comma) = rest.find(',') {
                if comma > 0 && rest[..comma].bytes().all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c)) {
                    out.push_str("Pos: <ptr>");
                    out.push_str(&rest[comma..]);
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    out
}

fn check_script(script: &str, expected: &BTreeMap<String, (String, usize)>) -> Result<usize, String> {
    let dir = format!("{}/target/nudge-debug-oracle", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, script)
        .current_dir(&dir)
        .output()
        .unwrap();
    if !out.status.success() {
        return Err(format!("{}: child failed:\n{}", script, String::from_utf8_lossy(&out.stderr)));
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let (Some(b), Some(e)) = (raw.find(BEGIN), raw.rfind(END)) else {
        return Err(format!("{}: markers missing in the child's stdout", script));
    };
    let stdout = mask_pointers(&raw[b + BEGIN.len()..e]);
    // The test harness's own notes are not part of the library's output.
    let stderr: String = String::from_utf8_lossy(&out.stderr)
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("golden-bump ") && !l.starts_with("golden-malloc "))
        .collect();
    std::fs::write(format!("{}/{}.stdout", dir, script), &stdout).unwrap();
    std::fs::write(format!("{}/{}.stderr", dir, script), &stderr).unwrap();
    for (name, text) in [("stdout", &stdout), ("stderr", &stderr)] {
        let (hash, bytes) = &expected[name];
        if sha::sha256(text.as_bytes()) != *hash {
            return Err(format!(
                "{}: {} differs from the C++ ({} bytes, C++ {}); see {}/{}.{}",
                script,
                name,
                text.len(),
                bytes,
                dir,
                script,
                name
            ));
        }
    }
    Ok(2)
}

#[test]
fn nudge_debug_output_matches_cpp() {
    if std::env::var_os(CHILD_ENV).is_some() {
        return;
    }
    let manifest = read_manifest();
    let slow = std::env::var_os("LIBAVOID_SLOW_FIXTURES").is_some_and(|v| !v.is_empty() && v != "0");
    let mut failures = Vec::new();
    let mut compared = 0;
    for (script, expected) in manifest.iter() {
        if !slow && ["performance01", "slowrouting", "unsatisfiableRangeAssertion"].contains(&script.as_str()) {
            continue;
        }
        match check_script(script, expected) {
            Ok(n) => compared += n,
            Err(m) => failures.push(m),
        }
    }
    eprintln!(
        "nudge debug oracle: {} scripts, {} outputs compared, {} scripts differ",
        manifest.len(),
        compared,
        failures.len()
    );
    assert!(failures.is_empty(), "NUDGE_DEBUG output differs from the C++:\n{}", failures.join("\n"));
}
