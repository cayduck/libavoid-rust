//! Differential test of `db_printf` (upstream `LIBAVOID_DEBUG`, here the
//! `libavoid-debug` feature) against the real C++.
//!
//! `SVG_MODE=libavoid-debug tools/oracle/svg_oracle.sh` records upstream's
//! `db_printf()` output (sent to a file of its own, see there) for every
//! upstream fixture and audit scene
//! (`tests/golden-svg/libavoid-debug-manifest.txt`).  The port prints it to
//! stdout like upstream, so each script runs in a child process (this test
//! binary re-executed for `child`), whose stdout between two markers is
//! the `db_printf` output (the interpreter keeps what the fixtures print
//! themselves in a buffer) plus upstream's one plain `printf` ("Warning,
//! nonorthogonal edge.", removed before the comparison).  It must be
//! byte-identical to the C++.
//!
//! `cargo test --release --features libavoid-debug --test libavoid_debug_oracle`, and with
//! `--features libavoid-debug,pathdebug,astar-debug` against the C++ built
//! with `PATHDEBUG` and `ASTAR_DEBUG` too (`SVG_MODE=path-debug`).

#![cfg(feature = "libavoid-debug")]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;

use std::collections::BTreeMap;
use std::process::Command;

const CHILD_ENV: &str = "LIBAVOID_DEBUG_ORACLE_CHILD";
/// LIBAVOID_DEBUG alone, or with PATHDEBUG and ASTAR_DEBUG
/// (`SVG_MODE=path-debug`).
#[cfg(not(any(feature = "pathdebug", feature = "astar-debug")))]
const MANIFEST: &str = "libavoid-debug-manifest.txt";
#[cfg(all(feature = "pathdebug", feature = "astar-debug"))]
const MANIFEST: &str = "path-debug-manifest.txt";
#[cfg(all(any(feature = "pathdebug", feature = "astar-debug"), not(all(feature = "pathdebug", feature = "astar-debug"))))]
compile_error!("the db_printf oracle has goldens for libavoid-debug alone or with both pathdebug and astar-debug");
const BEGIN: &str = "\n@@DBPRINTF-BEGIN@@\n";
const END: &str = "\n@@DBPRINTF-END@@\n";

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

#[path = "common/sha256.rs"]
mod sha;

fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    let path = format!("{}/tests/golden-svg/{}", env!("CARGO_MANIFEST_DIR"), MANIFEST);
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
    let dir = format!("{}/target/libavoid-debug-oracle", env!("CARGO_MANIFEST_DIR"));
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
    let stdout = String::from_utf8_lossy(&out.stdout);
    let (Some(b), Some(e)) = (stdout.find(BEGIN), stdout.rfind(END)) else {
        return Err(format!("{}: markers missing in the child's stdout", script));
    };
    // Upstream's one plain printf to stdout (MinimumTerminalSpanningTree::
    // getOrthogonalEdgesFromVertex(), "Warning, nonorthogonal edge.") is
    // not db_printf output; the oracle's db_printf goes to its own file, so
    // that line is removed here.
    let log = stdout[b + BEGIN.len()..e].replace("Warning, nonorthogonal edge.\n", "");
    let log = log.as_str();
    std::fs::write(format!("{}/{}.log", dir, script), log).unwrap();
    let (hash, bytes) = &expected["dbprintf.log"];
    if sha::sha256(log.as_bytes()) != *hash {
        return Err(format!(
            "{}: db_printf output differs from the C++ ({} bytes, C++ {}); see {}/{}.log",
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
fn db_printf_output_matches_cpp() {
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
        "db_printf oracle: {} scripts, {} logs compared, {} scripts differ",
        manifest.len(),
        compared,
        failures.len()
    );
    assert!(failures.is_empty(), "db_printf output differs from the C++:\n{}", failures.join("\n"));
}
