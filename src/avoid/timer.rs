//! Port of upstream `libavoid/timer.{h,cpp}`: the profiling timers.
//!
//! Upstream compiles all of this out unless `AVOID_PROFILE` is defined:
//! the `TIMER_*` macros are then empty and `Router` has no `timers` member.
//! Here the same holds for the `avoid-profile` Cargo feature (off by
//! default): without it the [`timer_start!`](crate::timer_start) family
//! expands to nothing, and only the `Timer` type itself is compiled.
//!
//! Differences from upstream:
//! * Upstream measures CPU time with `clock()` (`CLOCKS_PER_SEC` ticks).
//!   Rust's standard library has no CPU clock, so [`clock`] returns the
//!   wall-clock time since the first call, in microseconds
//!   (`CLOCKS_PER_SEC` = 1,000,000, glibc's value).  On wasm32 targets
//!   without a clock it returns 0.  Tallies and the `TIMER_VAR_*`
//!   variables are exact.
//! * `Router::timers` is a `RefCell<Timer>`, so that `Router::printInfo()`
//!   can print and reset the timers through `&Router`.

use std::fmt;

/// Upstream `typedef unsigned long long int bigclock_t`.
// upstream: timer.h typedef bigclock_t
pub type BigclockT = u64;

/// `clock_t` (a signed long on glibc).
pub type ClockT = i64;

/// `CLOCKS_PER_SEC` of [`clock`].
pub const CLOCKS_PER_SEC: f64 = 1_000_000.0;

/// The timers.
// upstream: timer.h enum TimerIndex
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimerIndex {
    TmAdd = 0,
    TmDel,
    TmMov,
    TmPth,
    TmOrthogGraph,
    TmOrthogRoute,
    TmOrthogCentre,
    TmOrthogNudge,
    TmHyperedgeForest,
    TmHyperedgeMTST,
    TmHyperedgeImprove,
    TmHyperedgeAlt,
    TmCount,
}

/// Number of timers (`tmCount`).
pub const TM_COUNT: usize = TimerIndex::TmCount as usize;

impl TimerIndex {
    /// The timer with index `i` (`(TimerIndex) i`).
    pub fn from_index(i: usize) -> TimerIndex {
        use TimerIndex::*;
        const ALL: [TimerIndex; TM_COUNT + 1] = [
            TmAdd,
            TmDel,
            TmMov,
            TmPth,
            TmOrthogGraph,
            TmOrthogRoute,
            TmOrthogCentre,
            TmOrthogNudge,
            TmHyperedgeForest,
            TmHyperedgeMTST,
            TmHyperedgeImprove,
            TmHyperedgeAlt,
            TmCount,
        ];
        ALL[i.min(TM_COUNT)]
    }
}

// upstream: timer.h timerStart
pub const TIMER_START: bool = true;
// upstream: timer.h timerDelay
pub const TIMER_DELAY: bool = false;

// upstream: timer.h TIMER_VARIABLES_COUNT
pub const TIMER_VARIABLES_COUNT: usize = 2;

/// Stand-in for C `clock()`: microseconds of wall-clock time since the
/// first call (see the module docs).
#[cfg(not(target_arch = "wasm32"))]
pub fn clock() -> ClockT {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_micros() as ClockT
}

/// Stand-in for C `clock()` (no clock on this target).
#[cfg(target_arch = "wasm32")]
pub fn clock() -> ClockT {
    0
}

// upstream: timer.cpp timerNames
static TIMER_NAMES: [&str; TM_COUNT] = [
    "Adds",
    "Dels",
    "Movs",
    "Pths",
    "OrthogGraph",
    "OrthogRoute",
    "OrthogCentre",
    "OrthogNudge",
    "HyperedgeForest",
    "HyperedgeMTST",
    "HyperedgeImprove",
    "HyperedgeAlt",
];

// upstream: timer.cpp toMsec (macro)
fn to_msec(tot: f64) -> BigclockT {
    (tot / (CLOCKS_PER_SEC / 1000.0)) as BigclockT
}

// upstream: timer.cpp toAvg (macro)
fn to_avg(tot: BigclockT, cnt: i32) -> f64 {
    if cnt > 0 {
        (tot as f64) / (cnt as f64)
    } else {
        0.0
    }
}

// upstream: timer.h class Timer
#[derive(Clone, Debug)]
pub struct Timer {
    m_start_time: [ClockT; TM_COUNT],
    m_total_time: [BigclockT; TM_COUNT],
    m_tally: [i32; TM_COUNT],
    m_max_time: [ClockT; TM_COUNT],
    m_variables: [[u32; TIMER_VARIABLES_COUNT]; TM_COUNT],

    m_is_running: bool,
    m_type: TimerIndex,
    m_last_type: TimerIndex,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::new()
    }
}

impl Timer {
    // upstream: timer.cpp Timer::Timer()
    pub fn new() -> Timer {
        let mut t = Timer {
            m_start_time: [0; TM_COUNT],
            m_total_time: [0; TM_COUNT],
            m_tally: [0; TM_COUNT],
            m_max_time: [0; TM_COUNT],
            m_variables: [[0; TIMER_VARIABLES_COUNT]; TM_COUNT],
            m_is_running: false,
            m_type: TimerIndex::TmCount,
            m_last_type: TimerIndex::TmCount,
        };
        t.reset();
        t
    }

    // upstream: timer.cpp Timer::reset()
    pub fn reset(&mut self) {
        for i in 0..TM_COUNT {
            self.m_total_time[i] = 0;
            self.m_tally[i] = 0;
            self.m_max_time[i] = 0;
            for j in 0..TIMER_VARIABLES_COUNT {
                self.m_variables[i][j] = 0;
            }
        }
        self.m_is_running = false;
        self.m_type = TimerIndex::TmCount;
        self.m_last_type = TimerIndex::TmCount;
    }

    // upstream: timer.cpp Timer::initialise()
    pub fn initialise(&mut self, t: TimerIndex) {
        debug_assert!(t != TimerIndex::TmCount);
        debug_assert!(self.m_type == TimerIndex::TmCount);

        self.m_type = t;
    }

    // upstream: timer.cpp Timer::start()
    pub fn start(&mut self) {
        debug_assert!(!self.m_is_running);
        // MISUSE: upstream indexes with tmCount (out of bounds) when
        // start() is called without initialise(); the port ignores it.
        if self.m_type == TimerIndex::TmCount {
            return;
        }
        self.m_start_time[self.m_type as usize] = clock(); // CPU time
        self.m_is_running = true;
    }

    // upstream: timer.cpp Timer::stop()
    pub fn stop(&mut self) {
        debug_assert!(self.m_is_running);
        let stop_time = clock(); // CPU time
        self.m_is_running = false;
        if self.m_type == TimerIndex::TmCount {
            return;
        }
        let ty = self.m_type as usize;

        let time_diff: BigclockT = if stop_time < self.m_start_time[ty] {
            // Uh-oh, the clock value has wrapped around.
            //
            let real_stop_time = (stop_time as BigclockT).wrapping_add(u64::MAX).wrapping_add(1);
            real_stop_time.wrapping_sub(self.m_start_time[ty] as BigclockT)
        } else {
            (stop_time - self.m_start_time[ty]) as BigclockT
        };

        debug_assert!(time_diff < i64::MAX as BigclockT);

        self.m_total_time[ty] = self.m_total_time[ty].wrapping_add(time_diff);
        self.m_tally[ty] += 1;
        if (time_diff as ClockT) > self.m_max_time[ty] {
            self.m_max_time[ty] = time_diff as ClockT;
        }
        self.m_last_type = self.m_type;

        self.m_type = TimerIndex::TmCount;
    }

    // upstream: timer.cpp Timer::printAll()
    pub fn print_all(&self, fp: &mut dyn fmt::Write) -> fmt::Result {
        for i in 0..TM_COUNT {
            write!(fp, "{}:  ", TIMER_NAMES[i])?;
            self.print(TimerIndex::from_index(i), fp)?;
        }
        fp.write_str("\n")
    }

    // upstream: timer.cpp Timer::printHyperedgePaper()
    pub fn print_hyperedge_paper(&self, fp: &mut dyn fmt::Write) -> fmt::Result {
        use TimerIndex::*;
        let t = &self.m_total_time;
        write!(
            fp,
            "{:5} & {:5} & {:5} & {:5}\n\n",
            to_msec(t[TmOrthogGraph as usize] as f64) as i64,
            to_msec(t[TmHyperedgeForest as usize].wrapping_add(t[TmHyperedgeMTST as usize]) as f64) as i64,
            to_msec(t[TmHyperedgeAlt as usize] as f64) as i64,
            to_msec(t[TmHyperedgeImprove as usize] as f64) as i64
        )
    }

    // upstream: timer.cpp Timer::print()
    pub fn print(&self, t: TimerIndex, fp: &mut dyn fmt::Write) -> fmt::Result {
        let t = t as usize;
        if t >= TM_COUNT {
            return Ok(());
        }
        let avg: BigclockT = to_msec(to_avg(self.m_total_time[t], self.m_tally[t]));
        let max: ClockT = to_msec(self.m_max_time[t] as f64) as ClockT;
        write!(
            fp,
            "{} {} {} {}",
            to_msec(self.m_total_time[t] as f64) as i64,
            self.m_tally[t],
            avg as i64,
            max
        )?;

        for j in 0..TIMER_VARIABLES_COUNT {
            if self.m_variables[t][j] > 0 {
                write!(fp, ", {}: {}", j, self.m_variables[t][j])?;
            }
        }
        fp.write_str("\n")
    }

    // upstream: timer.cpp Timer::varIncrement()
    pub fn var_increment(&mut self, i: usize, val: u32) {
        debug_assert!(i < TIMER_VARIABLES_COUNT);

        if self.m_is_running {
            let v = &mut self.m_variables[self.m_type as usize][i];
            *v = v.wrapping_add(val);
        }
    }

    // upstream: timer.cpp Timer::varMax()
    pub fn var_max(&mut self, i: usize, val: u32) {
        debug_assert!(i < TIMER_VARIABLES_COUNT);

        if self.m_is_running {
            let v = &mut self.m_variables[self.m_type as usize][i];
            *v = (*v).max(val);
        }
    }

    /// The number of times timer `t` was stopped (port addition, for tests).
    pub fn tally(&self, t: TimerIndex) -> i32 {
        self.m_tally.get(t as usize).copied().unwrap_or(0)
    }

    /// Timer variable `i` of timer `t` (port addition, for tests).
    pub fn variable(&self, t: TimerIndex, i: usize) -> u32 {
        self.m_variables.get(t as usize).and_then(|v| v.get(i)).copied().unwrap_or(0)
    }
}

/// `TIMER_START(r, t)`: `r->timers.initialise(t); r->timers.start()` with
/// the `avoid-profile` feature, nothing otherwise.
// upstream: timer.h TIMER_START
#[macro_export]
#[doc(hidden)]
macro_rules! timer_start {
    ($r:expr, $t:expr) => {
        #[cfg(feature = "avoid-profile")]
        {
            let mut timers = $r.timers.borrow_mut();
            timers.initialise($t);
            timers.start();
        }
    };
}

/// `TIMER_STOP(r)`.
// upstream: timer.h TIMER_STOP
#[macro_export]
#[doc(hidden)]
macro_rules! timer_stop {
    ($r:expr) => {
        #[cfg(feature = "avoid-profile")]
        {
            $r.timers.borrow_mut().stop();
        }
    };
}

/// `TIMER_VAR_ADD(r, n, v)`.
// upstream: timer.h TIMER_VAR_ADD
#[macro_export]
#[doc(hidden)]
macro_rules! timer_var_add {
    ($r:expr, $n:expr, $v:expr) => {
        #[cfg(feature = "avoid-profile")]
        {
            $r.timers.borrow_mut().var_increment($n, $v);
        }
    };
}

/// `TIMER_VAR_MAX(r, n, v)`.
// upstream: timer.h TIMER_VAR_MAX
#[macro_export]
#[doc(hidden)]
macro_rules! timer_var_max {
    ($r:expr, $n:expr, $v:expr) => {
        #[cfg(feature = "avoid-profile")]
        {
            $r.timers.borrow_mut().var_max($n, $v);
        }
    };
}

pub use crate::{timer_start, timer_stop, timer_var_add, timer_var_max};
