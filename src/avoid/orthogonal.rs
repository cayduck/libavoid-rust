//! Faithful port of upstream `libavoid/orthogonal.{h,cpp}`.  NOT YET
//! PORTED: the functions below are the interface the router calls; their
//! bodies are `todo!` stubs to be filled in without changing the
//! signatures.

#![allow(dead_code)]

use crate::avoid::router::Router;

// upstream: orthogonal.cpp generateStaticOrthogonalVisGraph()
pub fn generate_static_orthogonal_vis_graph(router: &mut Router) {
    let _ = router;
    todo!("upstream: orthogonal.cpp generateStaticOrthogonalVisGraph — not yet ported")
}

// upstream: orthogonal.cpp improveOrthogonalRoutes()
pub fn improve_orthogonal_routes(router: &mut Router) {
    let _ = router;
    todo!("upstream: orthogonal.cpp improveOrthogonalRoutes — not yet ported")
}
