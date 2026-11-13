//! Differential test of upstream's hyperedge debug output, built only with
//! `--features hyperedge-debug,major-hyperedge-improvement-debug`:
//!
//! * `HyperedgeImprover::outputHyperedgesToSVG()` (upstream
//!   `HYPEREDGE_DEBUG`), which writes `DEBUG/hyperedges-NNNNN.svg` (via
//!   `HyperedgeTreeNode::outputEdgesExcept()` /
//!   `HyperedgeTreeEdge::outputNodesExcept()`) relative to the working
//!   directory during hyperedge improvement;
//! * the `MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG` topology reports on stderr.
//!
//! `SVG_MODE=hyperedge-debug tools/oracle/svg_oracle.sh` records both from
//! the C++ for every upstream fixture and audit scene that writes such
//! files (`tests/golden-svg/hyperedge-debug-manifest.txt`).  Because the
//! files land in the working directory and the reports on the process's
//! stderr, each script runs in a child process (this test binary
//! re-executed for `child`, in its own directory under
//! `target/hyperedge-debug-oracle/`); its `DEBUG/` files and its stderr
//! (minus the test harness's own `golden-bump`/`golden-malloc` notes) must
//! be byte-identical to the C++.
//!
//! `cargo test --release --features hyperedge-debug,major-hyperedge-improvement-debug --test hyperedge_debug_oracle`

#![cfg(all(feature = "hyperedge-debug", feature = "major-hyperedge-improvement-debug"))]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;

use std::collections::BTreeMap;
use std::process::Command;

const CHILD_ENV: &str = "HYPEREDGE_DEBUG_ORACLE_CHILD";

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
    let path = format!("{}/tests/golden-svg/hyperedge-debug-manifest.txt", env!("CARGO_MANIFEST_DIR"));
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
    let dir = format!("{}/target/hyperedge-debug-oracle/{}", env!("CARGO_MANIFEST_DIR"), script);
    let debug_dir = format!("{}/DEBUG", dir);
    std::fs::create_dir_all(&debug_dir).unwrap();
    for e in std::fs::read_dir(&debug_dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "svg") {
            std::fs::remove_file(p).unwrap();
        }
    }
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, script)
        .current_dir(&dir)
        .output()
        .unwrap();
    if !out.status.success() {
        return Err(format!("{}: child failed:\n{}", script, String::from_utf8_lossy(&out.stdout)));
    }
    // The test harness's own notes are not part of the library's output.
    let stderr: String = String::from_utf8_lossy(&out.stderr)
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("golden-bump ") && !l.starts_with("golden-malloc "))
        .collect();
    std::fs::write(format!("{}/stderr", dir), &stderr).unwrap();

    let mut got: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for e in std::fs::read_dir(&debug_dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "svg") {
            got.insert(p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap());
        }
    }
    got.insert("stderr".to_string(), stderr.into_bytes());
    let got_names: Vec<&String> = got.keys().collect();
    let want_names: Vec<&String> = expected.keys().collect();
    if got_names != want_names {
        return Err(format!("{}: the port wrote {:?}, the C++ {:?}", script, got_names, want_names));
    }
    for (file, (hash, bytes)) in expected.iter() {
        if sha::sha256(&got[file]) != *hash {
            return Err(format!(
                "{}: {} differs from the C++ ({} bytes, C++ {}); see {}",
                script,
                file,
                got[file].len(),
                bytes,
                dir
            ));
        }
    }
    Ok(expected.len())
}

#[test]
fn hyperedge_debug_output_matches_cpp() {
    if std::env::var_os(CHILD_ENV).is_some() {
        return;
    }
    let manifest = read_manifest();
    let mut failures = Vec::new();
    let mut compared = 0;
    for (script, expected) in manifest.iter() {
        match check_script(script, expected) {
            Ok(n) => compared += n,
            Err(m) => failures.push(m),
        }
    }
    eprintln!(
        "hyperedge debug oracle: {} scripts, {} files compared, {} scripts differ",
        manifest.len(),
        compared,
        failures.len()
    );
    assert!(failures.is_empty(), "hyperedge debug output differs from the C++:\n{}", failures.join("\n"));
}
