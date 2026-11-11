//! Faithful port of upstream `libavoid/makepath.{h,cpp}`.  NOT YET PORTED:
//! `AStarPath` below is the interface `ConnRef::generatePath()` calls; the
//! bodies are `todo!` stubs to be filled in without changing the
//! signatures (fields may be added freely).

#![allow(dead_code)]
#![allow(clippy::new_without_default)]

use crate::avoid::arena::{ConnIdx, VertIdx};
use crate::avoid::router::Router;

// upstream: makepath.h class AStarPath
#[derive(Debug)]
pub struct AStarPath {}

impl AStarPath {
    // upstream: makepath.cpp AStarPath::AStarPath()
    pub fn new() -> AStarPath {
        todo!("upstream: makepath.cpp AStarPath::AStarPath — not yet ported")
    }

    /// Routes `line_ref` from `src` to `tar` (`start` is the rubber-band
    /// restart vertex, may be None), leaving the result in the vertices'
    /// `path_next` links.
    // upstream: makepath.cpp AStarPath::search()
    pub fn search(
        &mut self,
        router: &mut Router,
        line_ref: ConnIdx,
        src: VertIdx,
        tar: VertIdx,
        start: Option<VertIdx>,
    ) {
        let _ = (router, line_ref, src, tar, start);
        todo!("upstream: makepath.cpp AStarPath::search — not yet ported")
    }
}
