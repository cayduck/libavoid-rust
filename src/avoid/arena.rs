//! Arena storage and `Copy` index handles replacing upstream raw pointers.
//!
//! This module has no upstream counterpart.  See the data-model notes at the
//! top of `router.rs` for how the arenas are meant to be used.
//!
//! * An [`Arena<I, T>`] owns objects of type `T`, addressed by handle `I`.
//! * Allocation reuses freed slots (LIFO free list).  **Slot indices carry no
//!   meaning**: nothing may order, hash-iterate or tie-break by a handle's
//!   index.  Where upstream compares pointers (`std::set<T*>`, `a < b` on
//!   pointers, `std::min` of pointers, ...) order by the object's creation
//!   `serial` instead (see `Router::next_serial`) and mark the site with a
//!   `// PTR-ORDER:` comment.
//! * Handles are generation-checked: a handle to a freed object stays dead
//!   even after its slot is reused.  Indexing with a dead handle panics (the
//!   moral equivalent of a use-after-free in the C++); code that can be
//!   reached with a handle that may legitimately be dead (see the
//!   "Dangling handles" section of `router.rs`) checks `contains`/`get`.

#![allow(dead_code)]

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// Test hook (not upstream): when set, arenas never reuse freed slots.
/// Behaviour must not depend on it (slot indices carry no meaning); the
/// end-to-end fuzzer (`tests/e2e_fuzz.rs`, `E2E_NO_SLOT_REUSE=1`) replays
/// scenes both ways to catch slot indices leaking into behaviour.
pub static NO_SLOT_REUSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Conversion between a typed handle and a (slot index, generation) pair.
///
/// Handles are generation-checked in all builds: freeing a slot bumps its
/// generation, so a stale handle never aliases an object later allocated in
/// the same slot -- every access through it is detected (`contains`/`get`
/// return `false`/`None`, indexing panics) instead of silently reading the
/// new occupant.
pub trait ArenaIndex: Copy + Eq + std::fmt::Debug {
    fn from_parts(index: usize, generation: u32) -> Self;
    fn index(self) -> usize;
    fn generation(self) -> u32;
}

/// Defines a generation-checked `Copy` handle usable with [`Arena`].
#[macro_export]
macro_rules! avoid_define_handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name {
            index: u32,
            generation: u32,
        }

        impl $crate::avoid::arena::ArenaIndex for $name {
            #[inline]
            fn from_parts(index: usize, generation: u32) -> Self {
                $name { index: index as u32, generation }
            }
            #[inline]
            fn index(self) -> usize {
                self.index as usize
            }
            #[inline]
            fn generation(self) -> u32 {
                self.generation
            }
        }

        impl $name {
            /// Slot index (for diagnostics only: carries no meaning).
            #[inline]
            pub fn slot(self) -> u32 {
                self.index
            }
            /// A handle that names no object of any arena (for tests).
            #[inline]
            pub fn dangling() -> Self {
                $name { index: u32::MAX, generation: u32::MAX }
            }
        }
    };
}

/// Defines a plain `Copy` index (not arena-backed; a `Vec` index).
#[macro_export]
macro_rules! avoid_define_index {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name(pub u32);
    };
}

avoid_define_handle!(
    /// Handle of a `VertInf` (upstream `VertInf *`).
    VertIdx
);
avoid_define_handle!(
    /// Handle of an `EdgeInf` (upstream `EdgeInf *`).
    EdgeIdx
);
avoid_define_handle!(
    /// Handle of a tree-root cell (upstream `VertInf **` allocated by
    /// `VertInf::makeTreeRootPointer()`).
    TreeRootPtr
);
avoid_define_handle!(
    /// Handle of an `Obstacle` (`ShapeRef`/`JunctionRef`), upstream
    /// `Obstacle *`.  Defined here so that `ReferencingPolygon` can refer to
    /// obstacle polygons; the obstacle arena itself is added by the agent
    /// porting obstacle.cpp.
    ObstacleIdx
);
avoid_define_handle!(
    /// Handle of a `ConnRef`, upstream `ConnRef *`.  The connector arena is
    /// added by the agent porting connector.cpp.
    ConnIdx
);
avoid_define_index!(
    /// Handle of an A* search node (upstream `ANode *`, makepath.cpp).
    ANodeIdx
);
avoid_define_index!(
    /// Handle of a reroute flag owned by `ConnRerouteFlagDelegate`
    /// (upstream `bool *` pointing into its `m_mapping` list).
    RerouteFlagIdx
);

avoid_define_handle!(
    /// Handle of a `ShapeConnectionPin` (upstream `ShapeConnectionPin *`).
    PinIdx
);
avoid_define_handle!(
    /// Handle of a `ClusterRef` (upstream `ClusterRef *`).
    ClusterIdx
);

/// An object handle paired with the creation serial of the object it names.
///
/// Used as the element of upstream pointer-ordered containers
/// (`std::set<T *>`, `std::map<T *, ..>`): ordering and equality use the
/// `serial` only, which is unique per heap object, so iteration order is
/// creation order (see `// PTR-ORDER:` sites).
#[derive(Clone, Copy, Debug)]
pub struct PtrKey<I> {
    pub serial: u64,
    pub idx: I,
}

impl<I> PtrKey<I> {
    #[inline]
    pub fn new(serial: u64, idx: I) -> Self {
        PtrKey { serial, idx }
    }
}

impl<I> PartialEq for PtrKey<I> {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
    }
}
impl<I> Eq for PtrKey<I> {}
impl<I> PartialOrd for PtrKey<I> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<I> Ord for PtrKey<I> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.serial.cmp(&other.serial)
    }
}

/// Slot arena with free-list reuse and generation-checked handles.
#[derive(Clone, Debug)]
pub struct Arena<I: ArenaIndex, T> {
    slots: Vec<(u32, Option<T>)>,
    free: Vec<u32>,
    live: usize,
    _marker: PhantomData<I>,
}

impl<I: ArenaIndex, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cold]
#[inline(never)]
fn dead_handle(index: usize, generation: u32) -> ! {
    panic!("arena: access to freed or invalid handle {index} (generation {generation})");
}

impl<I: ArenaIndex, T> Arena<I, T> {
    pub fn new() -> Self {
        Arena {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            _marker: PhantomData,
        }
    }

    /// Stores `value` and returns its handle (upstream `new T(...)`).
    pub fn alloc(&mut self, value: T) -> I {
        self.live += 1;
        let reuse = !NO_SLOT_REUSE.load(std::sync::atomic::Ordering::Relaxed);
        if let Some(slot) = if reuse { self.free.pop() } else { None } {
            let s = &mut self.slots[slot as usize];
            debug_assert!(s.1.is_none());
            s.1 = Some(value);
            I::from_parts(slot as usize, s.0)
        } else {
            self.slots.push((0, Some(value)));
            I::from_parts(self.slots.len() - 1, 0)
        }
    }

    #[inline]
    fn slot(&self, handle: I) -> Option<&T> {
        match self.slots.get(handle.index()) {
            Some((g, Some(v))) if *g == handle.generation() => Some(v),
            _ => None,
        }
    }

    #[inline]
    fn slot_mut(&mut self, handle: I) -> Option<&mut T> {
        match self.slots.get_mut(handle.index()) {
            Some((g, Some(v))) if *g == handle.generation() => Some(v),
            _ => None,
        }
    }

    /// Removes and returns the object (upstream `delete ptr`).  The slot's
    /// generation is bumped so that every outstanding handle to it is dead.
    pub fn free(&mut self, handle: I) -> T {
        let index = handle.index();
        if self.slot(handle).is_none() {
            dead_handle(index, handle.generation());
        }
        let s = &mut self.slots[index];
        let value = s.1.take().unwrap();
        s.0 = s.0.wrapping_add(1);
        // A slot whose generation wrapped is retired rather than reused, so
        // that a 2^32-times-stale handle can never alias.
        if s.0 != 0 {
            self.free.push(index as u32);
        }
        self.live -= 1;
        value
    }

    /// Whether `handle` names a live object of this arena.
    #[inline]
    pub fn contains(&self, handle: I) -> bool {
        self.slot(handle).is_some()
    }

    #[inline]
    pub fn get(&self, handle: I) -> Option<&T> {
        self.slot(handle)
    }

    #[inline]
    pub fn get_mut(&mut self, handle: I) -> Option<&mut T> {
        self.slot_mut(handle)
    }

    /// Number of live objects.
    #[inline]
    pub fn len(&self) -> usize {
        self.live
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Iterates live objects in **slot order**.  Slot order is meaningless
    /// (see module docs); only use this where the order cannot matter
    /// (e.g. assertions, statistics).
    pub fn iter_unordered(&self) -> impl Iterator<Item = (I, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, (g, s))| s.as_ref().map(|t| (I::from_parts(i, *g), t)))
    }
}

impl<I: ArenaIndex, T> Index<I> for Arena<I, T> {
    type Output = T;
    #[inline]
    fn index(&self, handle: I) -> &T {
        match self.slot(handle) {
            Some(value) => value,
            None => dead_handle(handle.index(), handle.generation()),
        }
    }
}

impl<I: ArenaIndex, T> IndexMut<I> for Arena<I, T> {
    #[inline]
    fn index_mut(&mut self, handle: I) -> &mut T {
        match self.slot_mut(handle) {
            Some(value) => value,
            None => dead_handle(handle.index(), handle.generation()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_reuse() {
        let mut a: Arena<VertIdx, i32> = Arena::new();
        let x = a.alloc(1);
        let y = a.alloc(2);
        assert_eq!(a[x], 1);
        assert_eq!(a[y], 2);
        assert_eq!(a.free(x), 1);
        assert!(!a.contains(x));
        let z = a.alloc(3);
        assert_eq!(z.index(), x.index()); // slot reused ...
        assert_ne!(z, x); // ... under a new generation
        assert!(!a.contains(x));
        assert!(a.get(x).is_none());
        assert_eq!(a[z], 3);
        assert_eq!(a.len(), 2);
    }

    #[test]
    #[should_panic]
    fn stale_handle_to_reused_slot_panics() {
        let mut a: Arena<EdgeIdx, i32> = Arena::new();
        let x = a.alloc(1);
        a.free(x);
        let _y = a.alloc(2);
        let _ = a[x];
    }

    #[test]
    #[should_panic]
    fn freed_handle_panics() {
        let mut a: Arena<EdgeIdx, i32> = Arena::new();
        let x = a.alloc(1);
        a.free(x);
        let _ = a[x];
    }
}
