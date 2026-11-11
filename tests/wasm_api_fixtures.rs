//! Every upstream fixture scene (the op scripts of `tests/fixtures/`)
//! driven through the JS-facing string-id wrapper `AvoidRouter` instead of
//! the core API, with the same numeric object ids.  Each dump
//! (`outputDiagramText(true)`) and each connector's
//! `displayConnectorRoute()` JSON must match `tests/golden-bump/`; exit
//! codes and printed callback output must match upstream.  See
//! `tests/common/wrapper_ops.rs`.

#![allow(non_snake_case)]
#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/json.rs"]
mod json;
#[path = "common/wrapper_ops.rs"]
mod wrapper_ops;

fn run(kind: &str, name: &str) {
    let r = wrapper_ops::run_fixture_via_wrapper(name, true);
    match kind {
        "golden" => {
            assert!(r.golden_mode);
            assert!(r.outputs_checked > 0, "{}: nothing compared", name);
        }
        _ => assert!(!r.golden_mode),
    }
}

macro_rules! fixtures {
    ($( $kind:ident $test:ident $(= $file:literal)? ; )*) => {
        $(
            #[test]
            fn $test() {
                run(stringify!($kind), fixtures!(@file $test $($file)?));
            }
        )*
    };
    (@file $test:ident $file:literal) => { $file };
    (@file $test:ident) => { stringify!($test) };
}

fixtures! {
    golden _2junctions = "2junctions";
    golden buildOrthogonalChannelInfo1;
    golden checkpointNudging1;
    golden checkpointNudging2;
    golden checkpointNudging3;
    golden checkpoints01;
    golden checkpoints02;
    golden checkpoints03;
    golden complex;
    golden connectionpin01;
    golden connectionpin02;
    golden connectionpin03;
    golden connendmove;
    smoke corneroverlap01;
    golden endlessLoop01;
    golden example;
    golden finalSegmentNudging1;
    golden finalSegmentNudging2;
    golden finalSegmentNudging3;
    golden forwardFlowingConnectors01;
    golden freeFloatingDirection01;
    golden hola01;
    golden hyperedge01;
    golden hyperedge02;
    golden hyperedgeLoop1;
    golden hyperedgeRerouting01;
    golden improveHyperedge01;
    golden improveHyperedge02;
    golden improveHyperedge03;
    golden improveHyperedge04;
    golden improveHyperedge05;
    golden improveHyperedge06;
    golden infinity;
    golden inline;
    golden inlineOverlap09;
    golden inlineOverlap10;
    golden inlineOverlap11;
    golden inlineShapes;
    golden inlineoverlap01;
    golden inlineoverlap02;
    golden inlineoverlap03;
    golden inlineoverlap04;
    golden inlineoverlap05;
    golden inlineoverlap06;
    golden inlineoverlap07;
    golden inlineoverlap08;
    golden junction01;
    golden junction02;
    golden junction03;
    golden junction04;
    golden latesetup;
    golden lineSegWrapperCrash1;
    golden lineSegWrapperCrash2;
    golden lineSegWrapperCrash3;
    golden lineSegWrapperCrash4;
    golden lineSegWrapperCrash5;
    golden lineSegWrapperCrash6;
    golden lineSegWrapperCrash7;
    golden lineSegWrapperCrash8;
    golden multiconnact;
    golden node1;
    smoke nudgeCrossing01;
    golden nudgeintobug;
    golden nudgeold;
    golden nudgingSkipsCheckpoint01;
    golden nudgingSkipsCheckpoint02;
    golden orderassertion;
    golden orthordering01;
    golden orthordering02;
    golden overlappingRects;
    golden penaltyRerouting01;
    golden removeJunctions01;
    golden restrictedNudging;
    golden tjunct;
    golden treeRootCrash01;
    golden treeRootCrash02;
    golden validPaths01;
    golden validPaths02;
    golden vertlineassertion;
}
