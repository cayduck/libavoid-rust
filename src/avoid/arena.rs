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
//! * Accessing a freed handle panics (it is the moral equivalent of a
//!   use-after-free in the C++).

#![allow(dead_code)]

use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// Conversion between a typed handle and a raw slot index.
pub trait ArenaIndex: Copy + Eq + std::fmt::Debug {
    fn from_index(index: usize) -> Self;
    fn index(self) -> usize;
}

/// Defines a `Copy` newtype handle usable with [`Arena`].
#[macro_export]
macro_rules! avoid_define_handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name(pub u32);

        impl $crate::avoid::arena::ArenaIndex for $name {
            #[inline]
            fn from_index(index: usize) -> Self {
                $name(index as u32)
            }
            #[inline]
            fn index(self) -> usize {
                self.0 as usize
            }
        }
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
avoid_define_handle!(
    /// Handle of an A* search node (upstream `ANode *`, makepath.cpp).
    ANodeIdx
);
avoid_define_handle!(
    /// Handle of a reroute flag owned by `ConnRerouteFlagDelegate`
    /// (upstream `bool *` pointing into its `m_mapping` list).
    RerouteFlagIdx
);

/// Slot arena with free-list reuse.
#[derive(Clone, Debug)]
pub struct Arena<I: ArenaIndex, T> {
    slots: Vec<Option<T>>,
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
fn dead_handle(index: usize) -> ! {
    panic!("arena: access to freed or invalid handle {index}");
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
        if let Some(slot) = self.free.pop() {
            debug_assert!(self.slots[slot as usize].is_none());
            self.slots[slot as usize] = Some(value);
            I::from_index(slot as usize)
        } else {
            self.slots.push(Some(value));
            I::from_index(self.slots.len() - 1)
        }
    }

    /// Removes and returns the object (upstream `delete ptr`).
    pub fn free(&mut self, handle: I) -> T {
        let index = handle.index();
        match self.slots.get_mut(index).and_then(Option::take) {
            Some(value) => {
                self.free.push(index as u32);
                self.live -= 1;
                value
            }
            None => dead_handle(index),
        }
    }

    #[inline]
    pub fn contains(&self, handle: I) -> bool {
        matches!(self.slots.get(handle.index()), Some(Some(_)))
    }

    #[inline]
    pub fn get(&self, handle: I) -> Option<&T> {
        self.slots.get(handle.index()).and_then(Option::as_ref)
    }

    #[inline]
    pub fn get_mut(&mut self, handle: I) -> Option<&mut T> {
        self.slots.get_mut(handle.index()).and_then(Option::as_mut)
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
            .filter_map(|(i, s)| s.as_ref().map(|t| (I::from_index(i), t)))
    }
}

impl<I: ArenaIndex, T> Index<I> for Arena<I, T> {
    type Output = T;
    #[inline]
    fn index(&self, handle: I) -> &T {
        let index = handle.index();
        match self.slots.get(index) {
            Some(Some(value)) => value,
            _ => dead_handle(index),
        }
    }
}

impl<I: ArenaIndex, T> IndexMut<I> for Arena<I, T> {
    #[inline]
    fn index_mut(&mut self, handle: I) -> &mut T {
        let index = handle.index();
        match self.slots.get_mut(index) {
            Some(Some(value)) => value,
            _ => dead_handle(index),
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
        assert_eq!(z, x); // slot reused
        assert_eq!(a[z], 3);
        assert_eq!(a.len(), 2);
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
