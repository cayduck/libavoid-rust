//! Upstream hyperedge / junction fixtures
//! (`vendor/adaptagrams/cola/libavoid/tests/<name>.cpp`), compared with the
//! goldens written by the real C++ (`tests/golden/<name>*.txt`).
//!
//! The fixtures are translated statement by statement into the scene DSL
//! of the hyperedge oracle by `tools/oracle/hyperedge_fixture_scenes.py`
//! (output: `tools/oracle/hyperedge_fixtures.scenes`, scenes
//! `full-fixture-<name>`), which these tests replay with the shared DSL
//! interpreter (`tests/hyperedge_dsl`): every `processTransaction()` is a
//! real `Router::process_transaction()`, every
//! `outputDiagram("output/X")` an `assert_matches_golden(router, "X")`, and
//! the final router state is compared with `<name>-final` where upstream
//! reached `delete router`.  (Statements that only read results -- printf,
//! asserts, return values -- are not translated.)
//!
//! All of these route orthogonally, so they need `improve_orthogonal_routes`
//! (orthogonal nudging), which is not ported yet: they are #[ignore]d.  The
//! same scenes are verified *before* nudging against the C++ oracle by
//! `tests/hyperedge_oracle.rs` (scenes `fixture-<name>`).
//!
//! Note: `tests/golden/` was generated with glibc malloc; the bump-allocator
//! goldens (`tests/golden-bump/`, pointer order == allocation order, the
//! model of this port) are the intended target once available here.

#![allow(clippy::uninlined_format_args)]

mod common;

#[path = "hyperedge_dsl/mod.rs"]
mod hyperedge_dsl;

use libavoid_rust::avoid::router::Router;

fn scene_dsl(name: &str) -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/oracle/hyperedge_fixtures.scenes");
    let text = std::fs::read_to_string(path).expect("tools/oracle/hyperedge_fixtures.scenes");
    let header = format!("#scene full-fixture-{}", name);
    let mut lines = text.lines().skip_while(|l| *l != header);
    assert!(lines.next().is_some(), "fixture {} not translated", name);
    lines.take_while(|l| *l != "#endscene").map(|l| l.to_string()).collect()
}

fn check_diagram(router: &mut Router, name: &str) {
    common::assert_matches_golden_bump(router, name);
}

fn run_fixture(name: &str) {
    let dsl = scene_dsl(name);
    hyperedge_dsl::run_scene_with(&dsl, check_diagram);
}

macro_rules! fixture {
    ($test:ident, $name:expr) => {
        // upstream: tests/<name>.cpp
        #[test]
        fn $test() {
            run_fixture($name);
        }
    };
}

fixture!(hyperedge01, "hyperedge01");
fixture!(hyperedge02, "hyperedge02");
fixture!(hyperedge_loop1, "hyperedgeLoop1");
fixture!(hyperedge_rerouting01, "hyperedgeRerouting01");
fixture!(improve_hyperedge01, "improveHyperedge01");
fixture!(improve_hyperedge02, "improveHyperedge02");
fixture!(improve_hyperedge03, "improveHyperedge03");
fixture!(improve_hyperedge04, "improveHyperedge04");
fixture!(improve_hyperedge05, "improveHyperedge05");
fixture!(improve_hyperedge06, "improveHyperedge06");
fixture!(junction02, "junction02");
fixture!(junction03, "junction03");
fixture!(two_junctions, "2junctions");
fixture!(remove_junctions01, "removeJunctions01");
fixture!(tjunct, "tjunct");
