//! Port of upstream `libavoid/debug.h`: the `db_printf` / `err_printf`
//! diagnostics and the `PREC` precision of the SVG debug output, plus the
//! C `printf` number formatting the debug writers need.
//!
//! Upstream switches these with preprocessor defines; the port uses Cargo
//! features, all off by default:
//!
//! | upstream define        | Cargo feature          | effect |
//! |------------------------|------------------------|--------|
//! | `LIBAVOID_DEBUG`       | `libavoid-debug`       | `db_printf` writes to stdout (otherwise it compiles to nothing) |
//! | `HIGH_PRECISION_DEBUG` | `high-precision-debug` | `PREC` is `".16"` instead of `""` |
//!
//! `err_printf` always writes to stderr, as upstream does outside MSVC's
//! `USE_ATLTRACE` mode (which has no counterpart here).

use std::fmt;

/// Precision of numbers in libavoid SVG debug file.  Used in printf strings
/// (`"%" PREC "g"`), e.g., `"%.16g"`.  Upstream `PREC` is the string `""`
/// (plain `%g`, i.e. precision 6) unless `HIGH_PRECISION_DEBUG` is defined,
/// when it is `".16"`.
// upstream: debug.h PREC
#[cfg(not(feature = "high-precision-debug"))]
pub const PREC: usize = 6;
// upstream: debug.h PREC
#[cfg(feature = "high-precision-debug")]
pub const PREC: usize = 16;

/// `db_printf` is debugging output for verifying behaviour of the router.
/// Upstream it is a no-op unless `LIBAVOID_DEBUG` is defined, when it
/// `vfprintf`s to stdout.  Here it takes Rust format arguments (each call
/// site translates its C format string) and prints only with the
/// `libavoid-debug` feature; without it the arguments are type-checked but
/// the call compiles to nothing.
// upstream: debug.h #define db_printf (the MSVC USE_ATLTRACE alias of ATL::AtlTrace: no separate counterpart)
// upstream: debug.h db_printf(const char *, ...)
#[macro_export]
#[doc(hidden)]
macro_rules! db_printf {
    ($($arg:tt)*) => {
        if cfg!(feature = "libavoid-debug") {
            ::std::print!($($arg)*);
        }
    };
}
pub use crate::db_printf;

/// `err_printf` reports critical errors that mean something pretty bad has
/// happened; upstream always writes them to stderr.
// upstream: debug.h #define err_printf (the MSVC USE_ATLTRACE alias of ATL::AtlTrace: no separate counterpart)
// upstream: debug.h err_printf(const char *, ...)
#[macro_export]
#[doc(hidden)]
macro_rules! err_printf {
    ($($arg:tt)*) => {
        ::std::eprint!($($arg)*)
    };
}
pub use crate::err_printf;

/// C `printf("%.<prec>g", v)` as a `Display` value (glibc formatting; see
/// [`format_g`](crate::avoid::router::format_g)).
#[derive(Clone, Copy, Debug)]
pub struct CG(pub f64, pub usize);

impl fmt::Display for CG {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::avoid::router::format_g(self.0, self.1))
    }
}

/// C `printf("%g", v)`.
#[inline]
pub fn g(v: f64) -> CG {
    CG(v, 6)
}

/// C `printf("%" PREC "g", v)`.
#[inline]
pub fn pg(v: f64) -> CG {
    CG(v, PREC)
}

/// Adapts a `std::io::Write` (a file) to the `std::fmt::Write` the debug
/// writers take, keeping the first I/O error.
pub struct IoFmt<W: std::io::Write> {
    pub inner: W,
    pub error: Option<std::io::Error>,
}

impl<W: std::io::Write> IoFmt<W> {
    pub fn new(inner: W) -> Self {
        IoFmt { inner, error: None }
    }

    /// The first I/O error, if any.
    pub fn finish(mut self) -> std::io::Result<()> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        self.inner.flush()
    }
}

impl<W: std::io::Write> fmt::Write for IoFmt<W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        match self.inner.write_all(s.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.error = Some(e);
                Err(fmt::Error)
            }
        }
    }
}

/// `fopen(filename, "w")` + write + `fclose`: creates `filename` and writes
/// what `body` produces to it.  Upstream returns silently when `fopen`
/// fails; the port returns the error instead.  On targets without a file
/// system (`wasm32-unknown-unknown`) `std::fs` fails, so this returns an
/// error there; use the string forms instead.
pub fn write_file_with(
    filename: &str,
    body: impl FnOnce(&mut dyn fmt::Write) -> fmt::Result,
) -> std::io::Result<()> {
    let file = std::fs::File::create(filename)?;
    let mut w = IoFmt::new(std::io::BufWriter::new(file));
    let r = body(&mut w);
    if r.is_err() && w.error.is_none() {
        return Err(std::io::Error::other("formatting error"));
    }
    w.finish()
}
