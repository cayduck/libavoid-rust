//! Differential test of the SVG debug output against the real C++:
//! `Router::outputInstanceToSVG()` (the SVG with the embedded reproduction
//! code: `ShapeRef`/`JunctionRef`/`ConnRef`/`ConnEnd`/`ShapeConnectionPin`
//! `outputCode()`, clusters, routing parameters and options) and
//! `Router::outputDiagramSVG()`.
//!
//! `tools/oracle/svg_oracle.sh` runs every golden-mode upstream fixture
//! (`tests/fixtures/`) and every audit scene with goldens
//! (`tests/audit_scenes/`) through a patched copy of upstream that writes
//! both SVGs at every diagram dump, and records the sha256 and size of each
//! file in `tests/golden-svg/manifest.txt` (a few complete files are in
//! `tests/golden-svg/full/`).  This test runs the same op scripts through the
//! port (all the usual golden comparisons included), takes the SVGs at the
//! same points and requires byte-identical output.
//!
//! Environment:
//!  * `SVG_SCRIPT=name`  only that script
//!  * `SVG_DUMP=dir`     write the port's SVGs to `dir/<script>/` (for
//!    diffing against `target/svg-oracle/run/<script>/output/`)
//!  * `LIBAVOID_SLOW_FIXTURES=1` also the four slow fixtures (release
//!    builds recommended)
//!  * `SVG_SUBSAMPLE=k`  only every k-th script (default 4 in debug builds)

#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const SLOW: [&str; 4] = ["performance01", "slowrouting", "reallyslowrouting", "unsatisfiableRangeAssertion"];

#[path = "common/sha256.rs"]
mod sha;
use sha::sha256;

#[test]
fn sha256_known_answers() {
    assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let long = vec![b'a'; 1000];
    assert_eq!(sha256(&long), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
}

/// `script -> (file -> (sha256, bytes))`.
fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    // With HIGH_PRECISION_DEBUG upstream (`PREC` ".16"): the goldens of
    // `SVG_MODE=svg-high-precision tools/oracle/svg_oracle.sh`.
    let name = if cfg!(feature = "high-precision-debug") { "high-precision-manifest.txt" } else { "manifest.txt" };
    let path = format!("{}/tests/golden-svg/{}", env!("CARGO_MANIFEST_DIR"), name);
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

fn full_golden(file: &str) -> Option<String> {
    std::fs::read_to_string(format!("{}/tests/golden-svg/full/{}", env!("CARGO_MANIFEST_DIR"), file)).ok()
}

fn check_script(script: &str, expected: &BTreeMap<String, (String, usize)>) -> Result<usize, String> {
    let (_, mut svgs) = fixture_ops::run_fixture_capturing_svg(script);
    // (With the avoid-profile feature the capture also holds the timer
    // tallies, compared by tests/timer_oracle.rs.)
    svgs.retain(|k, _| k.ends_with(".svg"));
    if let Ok(dir) = std::env::var("SVG_DUMP") {
        let d = format!("{}/{}", dir, script);
        std::fs::create_dir_all(&d).unwrap();
        for (f, s) in svgs.iter() {
            std::fs::write(format!("{}/{}", d, f), s).unwrap();
        }
    }
    let got: Vec<&String> = svgs.keys().collect();
    let want: Vec<&String> = expected.keys().collect();
    if got != want {
        return Err(format!("{}: the port wrote {:?}, the C++ {:?}", script, got, want));
    }
    for (file, (hash, bytes)) in expected.iter() {
        let svg = &svgs[file];
        if sha256(svg.as_bytes()) != *hash {
            let mut msg = format!(
                "{}: {} differs from the C++ ({} bytes, C++ {} bytes)",
                script,
                file,
                svg.len(),
                bytes
            );
            if let Some(full) = full_golden(file).filter(|_| !cfg!(feature = "high-precision-debug")) {
                if let Some((i, (a, b))) = svg.lines().zip(full.lines()).enumerate().find(|(_, (a, b))| a != b) {
                    msg.push_str(&format!("\n  first difference at line {}:\n  rust: {}\n  c++:  {}", i + 1, a, b));
                }
            }
            return Err(msg);
        }
    }
    Ok(expected.len())
}

#[test]
fn svg_output_matches_cpp() {
    let manifest = read_manifest();
    let only = std::env::var("SVG_SCRIPT").ok();
    let slow = std::env::var_os("LIBAVOID_SLOW_FIXTURES").is_some_and(|v| !v.is_empty() && v != "0");
    let subsample: usize = std::env::var("SVG_SUBSAMPLE")
        .ok()
        .map(|v| v.parse().expect("SVG_SUBSAMPLE"))
        .unwrap_or(if cfg!(debug_assertions) { 4 } else { 1 });
    let scripts: Vec<(&String, &BTreeMap<String, (String, usize)>)> = manifest
        .iter()
        .filter(|(s, _)| match &only {
            Some(o) => *s == o,
            None => slow || !SLOW.contains(&s.as_str()),
        })
        .enumerate()
        .filter(|(i, _)| only.is_some() || i % subsample == 0)
        .map(|(_, e)| e)
        .collect();
    assert!(!scripts.is_empty(), "no scripts selected");

    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let compared = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= scripts.len() {
                    break;
                }
                let (script, expected) = scripts[i];
                let r = std::panic::catch_unwind(|| check_script(script, expected));
                match r {
                    Ok(Ok(n)) => {
                        compared.fetch_add(n, Ordering::SeqCst);
                    }
                    Ok(Err(m)) => failures.lock().unwrap().push(m),
                    Err(_) => failures.lock().unwrap().push(format!("{}: panicked", script)),
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    eprintln!(
        "svg oracle: {} scripts, {} SVG files compared, {} scripts differ",
        scripts.len(),
        compared.load(Ordering::SeqCst),
        failures.len()
    );
    assert!(failures.is_empty(), "SVG output differs from the C++:\n{}", failures.join("\n"));
}
