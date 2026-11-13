//! Differential test of the `AVOID_PROFILE` timers (the `avoid-profile`
//! feature) against the real C++.
//!
//! `SVG_MODE=profile tools/oracle/svg_oracle.sh` builds upstream with
//! `-DAVOID_PROFILE` and records, at every diagram dump of every upstream
//! fixture and audit scene, each timer's tally and its two `TIMER_VAR_*`
//! variables (`tests/golden-svg/profile-manifest.txt`; the measured times
//! are not reproducible and are left out).  This test takes the same
//! values from `Router::timers` at the same points and requires them to be
//! identical: the `TIMER_START`/`TIMER_STOP` pairs ran as often, and the
//! A* step and queue counters (`TIMER_VAR_ADD` in
//! `AStarPathPrivate::search()`) add up to the same numbers.
//!
//! `cargo test --release --features avoid-profile --test timer_oracle`

#![cfg(feature = "avoid-profile")]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;
#[path = "common/sha256.rs"]
mod sha;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use libavoid_rust::avoid::router::{Router, ORTHOGONAL_ROUTING};

const SLOW: [&str; 4] = ["performance01", "slowrouting", "reallyslowrouting", "unsatisfiableRangeAssertion"];

fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    let path = format!("{}/tests/golden-svg/profile-manifest.txt", env!("CARGO_MANIFEST_DIR"));
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
        out.get_mut(cur.as_ref().unwrap())
            .unwrap()
            .insert(toks[0].to_string(), (toks[1].to_string(), toks[2].parse().unwrap()));
    }
    out
}

fn check_script(script: &str, expected: &BTreeMap<String, (String, usize)>) -> Result<usize, String> {
    let (_, mut got) = fixture_ops::run_fixture_capturing_svg(script);
    got.retain(|k, _| k.ends_with("-timers.txt"));
    let got_names: Vec<&String> = got.keys().collect();
    let want_names: Vec<&String> = expected.keys().collect();
    if got_names != want_names {
        return Err(format!("{}: the port wrote {:?}, the C++ {:?}", script, got_names, want_names));
    }
    for (file, (hash, _)) in expected.iter() {
        if sha::sha256(got[file].as_bytes()) != *hash {
            return Err(format!("{}: {} differs from the C++; the port's:\n{}", script, file, got[file]));
        }
    }
    Ok(expected.len())
}

#[test]
fn timer_tallies_match_cpp() {
    let manifest = read_manifest();
    let slow = std::env::var_os("LIBAVOID_SLOW_FIXTURES").is_some_and(|v| !v.is_empty() && v != "0");
    let scripts: Vec<(&String, &BTreeMap<String, (String, usize)>)> = manifest
        .iter()
        .filter(|(s, _)| (slow || !SLOW.contains(&s.as_str())) && (!cfg!(debug_assertions) || s.starts_with("audit_0")))
        .collect();
    let next = AtomicUsize::new(0);
    let compared = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= scripts.len() {
                    break;
                }
                let (script, expected) = scripts[i];
                match std::panic::catch_unwind(|| check_script(script, expected)) {
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
        "timer oracle: {} scripts, {} dumps compared, {} scripts differ",
        scripts.len(),
        compared.load(Ordering::SeqCst),
        failures.len()
    );
    assert!(failures.is_empty(), "timer tallies differ from the C++:\n{}", failures.join("\n"));
}

/// `Router::printInfo()` prints the timer table (upstream format) and
/// resets the timers.
#[test]
fn print_info_prints_and_resets_timers() {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    r.process_transaction();
    let info = r.print_info();
    assert!(info.contains("\nAdds:  0 0 0 0\n"), "{}", info);
    assert!(info.contains("\nOrthogGraph:  "), "{}", info);
    assert!(info.ends_with("HyperedgeAlt:  0 0 0 0\n\n"), "{}", info);
    let tally = r.timers.borrow().tally(libavoid_rust::avoid::timer::TimerIndex::TmOrthogGraph);
    assert_eq!(tally, 0, "printInfo() resets the timers");
}
