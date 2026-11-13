//! Differential test of the `DebugHandler` calls (upstream `DEBUGHANDLER`,
//! here the `debug-handler` feature) against the real C++:
//!
//! `SVG_MODE=debug-handler tools/oracle/svg_oracle.sh` runs every
//! golden-mode upstream fixture and audit scene through upstream built with
//! `-DDEBUGHANDLER` and a recording handler
//! (`tools/oracle/debughandler_oracle.cpp`) and stores the sha256 of each
//! log in `tests/golden-svg/debug-handler-manifest.txt`.  This test registers
//! a handler that writes the same text (vertices as
//! `[objID,vn,props](x,y)`, routes and boxes by value, `%.17g`) and requires
//! identical logs.  Registering a handler also exercises
//! `MinimumTerminalSpanningTree::drawForest()`, which upstream only runs
//! with a handler and which creates orthogonal partner vertices; the
//! fixture comparisons (routes, junctions) run as usual, so the routing
//! results with a handler must match the C++ too.
//!
//! `cargo test --release --features debug-handler --test debughandler_oracle`
//! (`DH_SCRIPT=name` for one script; `DH_DUMP=dir` writes the port's logs.)

#![cfg(feature = "debug-handler")]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;
#[path = "common/sha256.rs"]
mod sha;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use libavoid_rust::avoid::arena::{ConnIdx, VertIdx};
use libavoid_rust::avoid::debughandler::DebugHandler;
use libavoid_rust::avoid::geomtypes::{Box as AvoidBox, Point, PolyLine};
use libavoid_rust::avoid::router::{format_g, Router};

const SLOW: [&str; 4] = ["performance01", "slowrouting", "reallyslowrouting", "unsatisfiableRangeAssertion"];

/// Writes what `tools/oracle/debughandler_oracle.cpp` writes.
struct Recorder(Rc<RefCell<String>>);

fn g17(v: f64) -> String {
    format_g(v, 17)
}

impl Recorder {
    fn vert(&self, router: &Router, v: VertIdx) {
        let vi = &router.vert_infs[v];
        let _ = write!(
            self.0.borrow_mut(),
            " [{},{},{}]({},{})",
            vi.id.obj_id,
            vi.id.vn,
            vi.id.props,
            g17(vi.point.x),
            g17(vi.point.y)
        );
    }
    fn points(&self, name: &str, ps: &[Point]) {
        let mut s = self.0.borrow_mut();
        let _ = write!(s, " {} {}", name, ps.len());
        for p in ps {
            let _ = write!(s, " ({},{})", g17(p.x), g17(p.y));
        }
    }
    fn push(&self, text: &str) {
        self.0.borrow_mut().push_str(text);
    }
}

impl DebugHandler for Recorder {
    fn update_obstacle_boxes(&mut self, _router: &Router, obstacles: Vec<AvoidBox>) {
        let mut s = self.0.borrow_mut();
        let _ = write!(s, "updateObstacleBoxes {}", obstacles.len());
        for b in obstacles.iter() {
            let _ = write!(s, " ({},{},{},{})", g17(b.min.x), g17(b.min.y), g17(b.max.x), g17(b.max.y));
        }
        s.push('\n');
    }
    fn update_connector_route(&mut self, router: &Router, conn: ConnIdx, index1: i32, index2: i32) {
        let c = &router.conns[conn];
        self.push(&format!("updateConnectorRoute {} {} {}", c.m_id, index1, index2));
        self.points("route", &c.m_route.ps);
        self.points("display", &c.m_display_route.ps);
        self.push("\n");
    }
    fn beginning_search_with_endpoints(&mut self, router: &Router, src: VertIdx, tar: VertIdx) {
        self.push("beginningSearchWithEndpoints");
        self.vert(router, src);
        self.vert(router, tar);
        self.push("\n");
    }
    fn update_current_search_path(&mut self, _router: &Router, current_path: PolyLine) {
        self.push("updateCurrentSearchPath");
        self.points("path", &current_path.ps);
        self.push("\n");
    }
    fn beginning_hyperedge_rerouting_with_endpoints(&mut self, router: &Router, endpoints: &[VertIdx]) {
        self.push(&format!("beginningHyperedgeReroutingWithEndpoints {}", endpoints.len()));
        for &v in endpoints {
            self.vert(router, v);
        }
        self.push("\n");
    }
    fn mtst_grow_forest_with_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx, should_wait: bool) {
        self.push("mtstGrowForestWithEdge");
        self.vert(router, u);
        self.vert(router, v);
        self.push(&format!(" {}\n", should_wait as i32));
    }
    fn mtst_potential_bridging_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx) {
        self.push("mtstPotentialBridgingEdge");
        self.vert(router, u);
        self.vert(router, v);
        self.push("\n");
    }
    fn mtst_commit_to_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx, is_bridge: bool) {
        self.push("mtstCommitToEdge");
        self.vert(router, u);
        self.vert(router, v);
        self.push(&format!(" {}\n", is_bridge as i32));
    }
}

fn read_manifest() -> BTreeMap<String, (String, usize)> {
    let path = format!("{}/tests/golden-svg/debug-handler-manifest.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let mut out = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks[0] == "script" {
            cur = Some(toks[1].to_string());
            continue;
        }
        assert_eq!(toks[0], "debughandler.log");
        out.insert(cur.clone().expect("entry before script"), (toks[1].to_string(), toks[2].parse().unwrap()));
    }
    out
}

fn check_script(script: &str, hash: &str, bytes: usize) -> Result<(), String> {
    let log = Rc::new(RefCell::new(String::new()));
    fixture_ops::run_fixture_with_debug_handler(script, Box::new(Recorder(log.clone())));
    let log = log.borrow();
    if let Ok(dir) = std::env::var("DH_DUMP") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(format!("{}/{}.log", dir, script), log.as_bytes()).unwrap();
    }
    if sha::sha256(log.as_bytes()) != hash {
        return Err(format!(
            "{}: debug handler log differs from the C++ ({} bytes, C++ {})",
            script,
            log.len(),
            bytes
        ));
    }
    Ok(())
}

#[test]
fn debug_handler_calls_match_cpp() {
    let manifest = read_manifest();
    let only = std::env::var("DH_SCRIPT").ok();
    let slow = std::env::var_os("LIBAVOID_SLOW_FIXTURES").is_some_and(|v| !v.is_empty() && v != "0");
    let scripts: Vec<(&String, &(String, usize))> = manifest
        .iter()
        .filter(|(s, _)| match &only {
            Some(o) => *s == o,
            None => (slow || !SLOW.contains(&s.as_str())) && (!cfg!(debug_assertions) || s.starts_with("audit_0")),
        })
        .collect();
    assert!(!scripts.is_empty(), "no scripts selected");
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= scripts.len() {
                    break;
                }
                let (script, (hash, bytes)) = scripts[i];
                match std::panic::catch_unwind(|| check_script(script, hash, *bytes)) {
                    Ok(Ok(())) => {}
                    Ok(Err(m)) => failures.lock().unwrap().push(m),
                    Err(_) => failures.lock().unwrap().push(format!("{}: panicked", script)),
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    eprintln!("debug handler oracle: {} scripts compared, {} differ", scripts.len(), failures.len());
    assert!(failures.is_empty(), "debug handler calls differ from the C++:\n{}", failures.join("\n"));
}
