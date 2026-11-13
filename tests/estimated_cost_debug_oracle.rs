//! Differential test of upstream's `ESTIMATED_COST_DEBUG` output (here the
//! `estimated-cost-debug` feature: the A* search's cost targets, with
//! `printDirections()`, on stderr) against the real C++.
//!
//! `SVG_MODE=estimated-cost-debug tools/oracle/svg_oracle.sh` records the
//! stderr of every upstream fixture and audit scene
//! (`tests/golden-svg/estimated-cost-debug-manifest.txt`).  Each script
//! runs in a child process (this test binary re-executed for `child`)
//! whose stderr, minus the test harness's own `golden-bump` /
//! `golden-malloc` notes, must be byte-identical to the C++ (this includes
//! upstream's `err_printf` warnings, which both print there too).
//!
//! `cargo test --release --features estimated-cost-debug --test estimated_cost_debug_oracle`

#![cfg(feature = "estimated-cost-debug")]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;

use std::collections::BTreeMap;
use std::process::Command;

const CHILD_ENV: &str = "ESTIMATED_COST_DEBUG_ORACLE_CHILD";

/// Runs one script in this process (the child side).
#[test]
fn child() {
    if let Ok(script) = std::env::var(CHILD_ENV) {
        fixture_ops::run_fixture(&script);
    }
}

#[path = "common/sha256.rs"]
mod sha;

fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    let path = format!("{}/tests/golden-svg/estimated-cost-debug-manifest.txt", env!("CARGO_MANIFEST_DIR"));
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

fn check_script(script: &str, expected: &BTreeMap<String, (String, usize)>) -> Result<usize, String> {
    let dir = format!("{}/target/estimated-cost-debug-oracle", env!("CARGO_MANIFEST_DIR"));
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
    // The test harness's own notes are not part of the library's output.
    let stderr: String = String::from_utf8_lossy(&out.stderr)
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("golden-bump ") && !l.starts_with("golden-malloc "))
        .collect();
    let log = stderr.as_str();
    std::fs::write(format!("{}/{}.log", dir, script), log).unwrap();
    let (hash, bytes) = &expected["stderr"];
    if sha::sha256(log.as_bytes()) != *hash {
        return Err(format!(
            "{}: stderr differs from the C++ ({} bytes, C++ {}); see {}/{}.log",
            script,
            log.len(),
            bytes,
            dir,
            script
        ));
    }
    Ok(1)
}

#[test]
fn estimated_cost_debug_output_matches_cpp() {
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
        "estimated cost debug oracle: {} scripts, {} logs compared, {} scripts differ",
        manifest.len(),
        compared,
        failures.len()
    );
    assert!(failures.is_empty(), "ESTIMATED_COST_DEBUG output differs from the C++:\n{}", failures.join("\n"));
}
