//! Rust/WASM port of the libavoid connector-routing library.
//!
//! * [`avoid`]: the faithful port of upstream C++ libavoid.
//! * [`AvoidRouter`]: the wasm-bindgen string-id adapter used by the
//!   JavaScript package (`index.cjs`).

pub mod avoid;
mod constants;
mod json;
mod wasm_api;
mod wasm_failure;

pub use constants::*;
pub use wasm_api::{curved_polyline, AvoidRouter};
