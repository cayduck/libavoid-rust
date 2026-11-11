//! Faithful port of upstream `libavoid/visibility.{h,cpp}`.  NOT YET
//! PORTED: the functions below are the interface the object layer calls;
//! their bodies are `todo!` stubs to be filled in without changing their
//! signatures.

#![allow(dead_code)]

use crate::avoid::arena::{ObstacleIdx, VertIdx};
use crate::avoid::router::Router;

/// Upstream default for `gen_contains` is false.
// upstream: visibility.cpp vertexVisibility()
pub fn vertex_visibility(
    router: &mut Router,
    point: VertIdx,
    partner: Option<VertIdx>,
    known_new: bool,
    gen_contains: bool,
) {
    let _ = (router, point, partner, known_new, gen_contains);
    todo!("upstream: visibility.cpp vertexVisibility — not yet ported")
}

// upstream: visibility.cpp Obstacle::computeVisibilityNaive()
pub fn compute_visibility_naive(router: &mut Router, obstacle: ObstacleIdx) {
    let _ = (router, obstacle);
    todo!("upstream: visibility.cpp Obstacle::computeVisibilityNaive — not yet ported")
}

// upstream: visibility.cpp Obstacle::computeVisibilitySweep()
pub fn compute_visibility_sweep(router: &mut Router, obstacle: ObstacleIdx) {
    let _ = (router, obstacle);
    todo!("upstream: visibility.cpp Obstacle::computeVisibilitySweep — not yet ported")
}
