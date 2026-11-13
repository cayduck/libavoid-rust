//! Diagnostics for internal failures of the wasm build (port addition, not
//! upstream).
//!
//! The release wasm is built with `panic = "abort"`: a Rust panic, a failed
//! allocation or a stack overflow traps the wasm instance, which JavaScript
//! sees as a `WebAssembly.RuntimeError` without a message.  This module
//! records what Rust knows about the failure *before* the trap, in a fixed
//! buffer that needs no allocation to write:
//!
//! * a panic hook (installed once, when the module is instantiated) records
//!   `panicked at <file>:<line>:<column>: <message>`;
//! * on wasm32 the global allocator wraps `System` and records
//!   `memory allocation of <n> bytes failed` when an allocation returns null
//!   (std then aborts without running the panic hook).
//!
//! The JS adapter (`lib/adapter.cjs`) reads the record with
//! `takeInternalErrorMessage()` after it has caught a trap and turns it
//! into an `AvoidInternalError`.  Source paths in the message are
//! crate-relative: the wasm build remaps absolute path prefixes
//! (`scripts/build-wasm.cjs`).
//!
//! The module also holds the test-only trap trigger used by the Node tests
//! (`RawRouter::__testPanic`, inert unless given [`TEST_PANIC_ARMING`]).

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::cell::UnsafeCell;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use wasm_bindgen::prelude::*;

const CAPACITY: usize = 1024;

/// Fixed-size record of the last failure.  wasm32 without atomics is
/// single-threaded; on native targets the record is only written by the
/// panic hook, which this crate never installs there.
struct Record {
    buf: UnsafeCell<[u8; CAPACITY]>,
    len: AtomicUsize,
}

unsafe impl Sync for Record {}

static RECORD: Record = Record { buf: UnsafeCell::new([0; CAPACITY]), len: AtomicUsize::new(0) };

/// `fmt::Write` into the record, truncating at a char boundary.
struct RecordWriter {
    len: usize,
}

impl Write for RecordWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // SAFETY: single-threaded (see `Record`); no reference to the buffer
        // outlives this call.
        let buf = unsafe { &mut *RECORD.buf.get() };
        let mut n = s.len().min(CAPACITY - self.len);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Overwrites the record (no allocation).
fn record(args: fmt::Arguments) {
    let mut w = RecordWriter { len: 0 };
    let _ = w.write_fmt(args);
    RECORD.len.store(w.len, Ordering::SeqCst);
}

/// Returns and clears the record ("" if nothing was recorded).
pub fn take_message() -> String {
    let len = RECORD.len.swap(0, Ordering::SeqCst);
    // SAFETY: see `RecordWriter::write_str`; `len` bytes were written as
    // whole UTF-8 characters.
    let bytes = unsafe { &(&*RECORD.buf.get())[..len] };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Installs the panic hook (idempotent).
pub fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let payload = info.payload();
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("Box<dyn Any>");
            match info.location() {
                Some(l) => record(format_args!("panicked at {}:{}:{}: {}", l.file(), l.line(), l.column(), message)),
                None => record(format_args!("panicked: {}", message)),
            }
        }));
    });
}

/// Runs when the wasm module is instantiated (both wasm-pack targets).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
fn __start() {
    install_panic_hook();
}

/// The record of the last internal failure (panic message and location, or
/// failed allocation size) and clears it; "" if there is none.  Called by
/// the JS adapter after a trap.
#[wasm_bindgen(js_name = takeInternalErrorMessage)]
pub fn take_internal_error_message() -> String {
    take_message()
}

#[cfg(target_arch = "wasm32")]
mod alloc {
    use std::alloc::{GlobalAlloc, Layout, System};

    /// `System` that records failed allocations before std aborts.
    struct RecordingAlloc;

    unsafe impl GlobalAlloc for RecordingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            check(System.alloc(layout), layout.size())
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            check(System.alloc_zeroed(layout), layout.size())
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            check(System.realloc(ptr, layout, new_size), new_size)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
    }

    #[inline]
    fn check(p: *mut u8, size: usize) -> *mut u8 {
        if p.is_null() {
            super::record(format_args!("memory allocation of {} bytes failed", size));
        }
        p
    }

    #[global_allocator]
    static ALLOC: RecordingAlloc = RecordingAlloc;
}

// ---------------------------------------------------------------------------
// Test-only trap trigger
// ---------------------------------------------------------------------------

/// The exact string `RawRouter::__testPanic` must be given to do anything.
/// It exists so that the shipped wasm can be tested as shipped while the
/// trigger stays inert for any accidental call.
pub const TEST_PANIC_ARMING: &str = "libavoid-rust: I want to trap this WebAssembly instance";

/// Kinds of `__testPanic`.
pub const TEST_PANIC_EXPLICIT: u32 = 1;
pub const TEST_PANIC_INDEX: u32 = 2;
pub const TEST_PANIC_STACK_OVERFLOW: u32 = 3;
pub const TEST_PANIC_OUT_OF_MEMORY: u32 = 4;

#[inline(never)]
#[allow(unconditional_recursion)]
pub(crate) fn recurse_forever(depth: u32) -> u32 {
    let frame = std::hint::black_box([depth; 32]);
    frame[(depth % 32) as usize].wrapping_add(recurse_forever(std::hint::black_box(depth + 1)))
}

/// Allocates (without touching) 1 GiB blocks until an allocation fails.
pub(crate) fn exhaust_memory() -> ! {
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    loop {
        blocks.push(Vec::with_capacity(1 << 30));
        std::hint::black_box(&blocks);
    }
}
