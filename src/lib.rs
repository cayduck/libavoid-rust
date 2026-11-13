//! Rust/WASM port of the libavoid connector-routing library.
//!
//! * [`avoid`]: the faithful port of upstream C++ libavoid.
//! * [`RawRouter`] and the free functions of `wasm_api`: the low-level
//!   wasm-bindgen layer under the JavaScript API (`lib/adapter.cjs`
//!   implements upstream's classes on top of it).

pub mod avoid;
mod constants;
mod wasm_api;
mod wasm_failure;

pub use constants::*;
pub use wasm_api::RawRouter;
