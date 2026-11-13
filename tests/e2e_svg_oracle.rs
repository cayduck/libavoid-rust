//! Differential test of the SVG debug output (`Router::outputInstanceToSVG()`
//! and `Router::outputDiagramSVG()`) after every transaction of the
//! committed end-to-end fuzz scenes, against the C++ fuzz driver.
//!
//! `tools/oracle/e2e_svg_oracle.sh` runs the scenes of
//! `tests/golden/e2e_fuzz.txt` on which upstream is well defined through the
//! NDEBUG C++ driver with `E2E_SVG=1`, which adds a `V <fnv> <bytes>` and a
//! `W <fnv> <bytes>` line (the two SVGs) to every transaction dump, and
//! stores per scene the sha256 of those lines
//! (`tests/golden-svg/e2e-svg.txt`).  This test replays each scene through
//! the Rust interpreter with the same lines enabled
//! (`e2e_fuzz_dsl::SVG_OUTPUT`) and requires identical hashes.  Debug builds
//! check a tenth of the scenes (`E2E_SVG_SUBSAMPLE=k` overrides,
//! `E2E_SVG_SCENE=name` selects one).

#![allow(clippy::uninlined_format_args)]

#[path = "e2e_fuzz_dsl/mod.rs"]
mod e2e_fuzz_dsl;
#[path = "common/sha256.rs"]
mod sha;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use e2e_fuzz_dsl::{parse_golden, run_scene, SVG_OUTPUT};

#[test]
fn e2e_svg_output_matches_cpp() {
    let golden = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/e2e_fuzz.txt"));
    let svg_golden = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden-svg/e2e-svg.txt"));
    let mut expected: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for line in svg_golden.lines().filter(|l| !l.starts_with('#')) {
        let t: Vec<&str> = line.split_whitespace().collect();
        expected.insert(t[0], (t[1], t[2].parse().unwrap()));
    }
    let only = std::env::var("E2E_SVG_SCENE").ok();
    let subsample: usize = std::env::var("E2E_SVG_SUBSAMPLE")
        .ok()
        .map(|v| v.parse().expect("E2E_SVG_SUBSAMPLE"))
        .unwrap_or(if cfg!(debug_assertions) { 10 } else { 1 });
    let scenes: Vec<_> = parse_golden(golden)
        .into_iter()
        .filter(|s| expected.contains_key(s.name.as_str()))
        .filter(|s| only.as_deref().is_none_or(|o| o == s.name))
        .enumerate()
        .filter(|(i, _)| only.is_some() || i % subsample == 0)
        .map(|(_, s)| s)
        .collect();
    assert!(!scenes.is_empty(), "no scenes selected");

    SVG_OUTPUT.store(true, Ordering::Relaxed);
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= scenes.len() {
                    break;
                }
                let scene = &scenes[i];
                let (hash, count) = expected[scene.name.as_str()];
                let dsl = scene.dsl.clone();
                match std::panic::catch_unwind(move || run_scene(&dsl, true)) {
                    Ok(out) => {
                        let svg: Vec<&String> = out.iter().filter(|l| l.starts_with("V ") || l.starts_with("W ")).collect();
                        let data: String = svg.iter().map(|l| format!("{}\n", l)).collect();
                        if svg.len() != count || sha::sha256(data.as_bytes()) != hash {
                            failures.lock().unwrap().push(format!(
                                "{}: SVG output differs from the C++ ({} V/W lines, C++ {})",
                                scene.name,
                                svg.len(),
                                count
                            ));
                        }
                    }
                    Err(_) => failures.lock().unwrap().push(format!("{}: panicked", scene.name)),
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    eprintln!("e2e svg oracle: {} scenes compared, {} differ", scenes.len(), failures.len());
    assert!(failures.is_empty(), "SVG output differs from the C++:\n{}", failures.join("\n"));
}
