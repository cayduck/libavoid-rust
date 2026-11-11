//! Faithful port of upstream `libavoid/actioninfo.{h,cpp}`.
//!
//! * `void *objPtr` is an [`ActionObj`] (the handle of the shape/junction,
//!   connector or pin).  Equality (`operator==`) compares the handle, as
//!   upstream compares the pointer.
//! * `operator<` needs `conn()->id()`, `obstacle()->id()` or (for
//!   `ConnectionPinChange`) the pointer value.  Object ids never change
//!   after construction, so the id is captured when the `ActionInfo` is
//!   created (`obj_id`), together with the object's creation serial
//!   (`obj_serial`, the `// PTR-ORDER:` stand-in for the pointer value).
//! * `ActionInfoList` (`std::list<ActionInfo>`) is a `Vec<ActionInfo>`;
//!   `std::list::sort()` is reproduced with `libstdcxx_list_sort`.

#![allow(dead_code)]
// Keep upstream statement shapes.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::print_with_newline,
    clippy::field_reassign_with_default,
    clippy::assign_op_pattern,
    clippy::explicit_counter_loop,
    clippy::if_same_then_else,
    clippy::question_mark,
    clippy::neg_multiply,
    clippy::needless_late_init,
    clippy::uninlined_format_args
)]

use crate::avoid::arena::{ConnIdx, ObstacleIdx, PinIdx};
use crate::avoid::connend::ConnEnd;
use crate::avoid::geomtypes::{Point, Polygon};
use crate::avoid::junction::JunctionRef;
use crate::avoid::obstacle::ObstacleKind;
use crate::avoid::router::Router;
use crate::avoid::shape::ShapeRef;

// upstream: actioninfo.h enum ActionType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionType {
    ShapeMove = 0,
    ShapeAdd = 1,
    ShapeRemove = 2,
    JunctionMove = 3,
    JunctionAdd = 4,
    JunctionRemove = 5,
    ConnChange = 6,
    ConnectionPinChange = 7,
}

/// The object an action refers to (upstream `void *objPtr`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionObj {
    Obstacle(ObstacleIdx),
    Conn(ConnIdx),
    Pin(PinIdx),
}

// upstream: actioninfo.h typedef std::list<std::pair<unsigned int, ConnEnd> > ConnUpdateList
pub type ConnUpdateList = Vec<(u32, ConnEnd)>;

// upstream: actioninfo.h class ActionInfo
#[derive(Clone, Debug)]
pub struct ActionInfo {
    pub type_: ActionType,
    pub obj_ptr: ActionObj,
    /// `obstacle()->id()` / `conn()->id()` captured at construction
    /// (0 for pins).
    pub obj_id: u32,
    /// Creation serial of the object (`// PTR-ORDER:` pointer value).
    pub obj_serial: u64,
    pub new_poly: Polygon,
    pub new_position: Point,
    /// Uninitialised upstream for non-move actions; false here.
    pub first_move: bool,
    pub conns: ConnUpdateList,
}

impl ActionInfo {
    fn base(type_: ActionType, obj_ptr: ActionObj, obj_id: u32, obj_serial: u64) -> ActionInfo {
        ActionInfo {
            type_,
            obj_ptr,
            obj_id,
            obj_serial,
            new_poly: Polygon::new(),
            new_position: Point::default(),
            first_move: false,
            conns: Vec::new(),
        }
    }

    fn obstacle_info(router: &Router, o: ObstacleIdx) -> (u32, u64) {
        let ob = &router.obstacles[o];
        (ob.m_id, ob.serial)
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, ShapeRef *, const Polygon&, bool)
    pub fn new_shape_move(router: &Router, t: ActionType, s: ShapeRef, p: &Polygon, f_m: bool) -> ActionInfo {
        let (id, serial) = ActionInfo::obstacle_info(router, s.0);
        let mut a = ActionInfo::base(t, ActionObj::Obstacle(s.0), id, serial);
        a.new_poly = p.clone();
        a.first_move = f_m;
        debug_assert!(a.type_ == ActionType::ShapeMove);
        a
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, ShapeRef *)
    pub fn new_shape(router: &Router, t: ActionType, s: ShapeRef) -> ActionInfo {
        let (id, serial) = ActionInfo::obstacle_info(router, s.0);
        let a = ActionInfo::base(t, ActionObj::Obstacle(s.0), id, serial);
        debug_assert!(
            (t == ActionType::ShapeAdd) || (t == ActionType::ShapeRemove) || (t == ActionType::ShapeMove)
        );
        a
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, JunctionRef *, const Point&)
    pub fn new_junction_move(router: &Router, t: ActionType, j: JunctionRef, p: &Point) -> ActionInfo {
        let (id, serial) = ActionInfo::obstacle_info(router, j.0);
        let mut a = ActionInfo::base(t, ActionObj::Obstacle(j.0), id, serial);
        a.new_position = *p;
        debug_assert!(t == ActionType::JunctionMove);
        a
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, JunctionRef *)
    pub fn new_junction(router: &Router, t: ActionType, j: JunctionRef) -> ActionInfo {
        let (id, serial) = ActionInfo::obstacle_info(router, j.0);
        let a = ActionInfo::base(t, ActionObj::Obstacle(j.0), id, serial);
        debug_assert!(
            (t == ActionType::JunctionAdd)
                || (t == ActionType::JunctionRemove)
                || (t == ActionType::JunctionMove)
        );
        a
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, ConnRef *)
    pub fn new_conn(router: &Router, t: ActionType, c: ConnIdx) -> ActionInfo {
        let conn = &router.conns[c];
        let a = ActionInfo::base(t, ActionObj::Conn(c), conn.m_id, conn.serial);
        debug_assert!(t == ActionType::ConnChange);
        a
    }

    // upstream: actioninfo.cpp ActionInfo::ActionInfo(ActionType, ShapeConnectionPin *)
    pub fn new_pin(router: &Router, t: ActionType, p: PinIdx) -> ActionInfo {
        let a = ActionInfo::base(t, ActionObj::Pin(p), 0, router.pins[p].serial);
        debug_assert!(t == ActionType::ConnectionPinChange);
        a
    }

    // upstream: actioninfo.cpp ActionInfo::obstacle()
    pub fn obstacle(&self) -> ObstacleIdx {
        debug_assert!(matches!(
            self.type_,
            ActionType::ShapeMove
                | ActionType::ShapeAdd
                | ActionType::ShapeRemove
                | ActionType::JunctionMove
                | ActionType::JunctionAdd
                | ActionType::JunctionRemove
        ));
        match self.obj_ptr {
            ActionObj::Obstacle(o) => o,
            _ => panic!("ActionInfo::obstacle(): not an obstacle action"),
        }
    }

    // upstream: actioninfo.cpp ActionInfo::shape()
    pub fn shape(&self, router: &Router) -> Option<ShapeRef> {
        let o = self.obstacle();
        match router.obstacles[o].kind {
            ObstacleKind::Shape => Some(ShapeRef(o)),
            ObstacleKind::Junction(_) => None,
        }
    }

    // upstream: actioninfo.cpp ActionInfo::conn()
    pub fn conn(&self) -> ConnIdx {
        debug_assert!(self.type_ == ActionType::ConnChange);
        match self.obj_ptr {
            ActionObj::Conn(c) => c,
            _ => panic!("ActionInfo::conn(): not a connector action"),
        }
    }

    // upstream: actioninfo.cpp ActionInfo::junction()
    pub fn junction(&self, router: &Router) -> Option<JunctionRef> {
        let o = self.obstacle();
        match router.obstacles[o].kind {
            ObstacleKind::Junction(_) => Some(JunctionRef(o)),
            ObstacleKind::Shape => None,
        }
    }

    // upstream: actioninfo.cpp ActionInfo::addConnEndUpdate()
    pub fn add_conn_end_update(&mut self, type_: u32, conn_end: &ConnEnd, is_conn_pin_move_update: bool) {
        let mut already_exists = false;
        for conn in self.conns.iter_mut() {
            // Look for an existing queued change to the same end.
            if conn.0 == type_ {
                // Found a queued change to the same endpoint of the
                // connector. If this is a pin change as a result of a
                // shape move, then leave the user created update
                // that was found (since it may be moving the connection
                // to connect to a different shape/pin.  But if this is a
                // user change, then overwrite the previous change.
                already_exists = true;
                if !is_conn_pin_move_update {
                    // Overwrite the queued change with this one.
                    conn.1 = conn_end.clone();
                }
                break;
            }
        }

        if !already_exists {
            // Matching change not found, so add this one.
            self.conns.push((type_, conn_end.clone()));
        }
    }

    // upstream: actioninfo.cpp ActionInfo::operator==()
    #[inline]
    pub fn equals(&self, rhs: &ActionInfo) -> bool {
        (self.type_ == rhs.type_) && (self.obj_ptr == rhs.obj_ptr)
    }

    // upstream: actioninfo.cpp ActionInfo::operator<()
    pub fn less_than(&self, rhs: &ActionInfo) -> bool {
        if self.type_ != rhs.type_ {
            return self.type_ < rhs.type_;
        }

        if self.type_ == ActionType::ConnChange {
            self.obj_id < rhs.obj_id
        } else if self.type_ == ActionType::ConnectionPinChange {
            // NOTE Comparing pointers may not preserve the order of
            //      objects, but the order of Connection Pins is not
            //      used so this is not an issue here.
            // PTR-ORDER: pointer comparison replaced by creation serial.
            self.obj_serial < rhs.obj_serial
        } else {
            self.obj_id < rhs.obj_id
        }
    }
}

// upstream: actioninfo.h typedef std::list<ActionInfo> ActionInfoList
pub type ActionInfoList = Vec<ActionInfo>;

/// `std::find(list.begin(), list.end(), info)` (uses `operator==`).
pub fn find_action(list: &ActionInfoList, info: &ActionInfo) -> Option<usize> {
    list.iter().position(|a| a.equals(info))
}
