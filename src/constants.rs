//! Flat numeric constants of the upstream libavoid public API, mirrored by
//! the frozen objects exported from `index.cjs` (`RouterFlag`, `ConnDir`,
//! `RoutingParameter`, ...).  The values are those of the upstream enums;
//! `tests` below checks them against `crate::avoid`.

// upstream: router.h enum RouterFlag
pub const POLY_LINE_ROUTING: u32 = 1;
pub const ORTHOGONAL_ROUTING: u32 = 2;

// upstream: connector.h enum ConnType
pub const CONN_TYPE_NONE: u32 = 0;
pub const CONN_TYPE_POLY_LINE: u32 = 1;
pub const CONN_TYPE_ORTHOGONAL: u32 = 2;

// upstream: connend.h enum ConnDirFlag
pub const CONN_DIR_NONE: u32 = 0;
pub const CONN_DIR_UP: u32 = 1;
pub const CONN_DIR_DOWN: u32 = 2;
pub const CONN_DIR_LEFT: u32 = 4;
pub const CONN_DIR_RIGHT: u32 = 8;
pub const CONN_DIR_ALL: u32 = 15;

// upstream: connend.h enum ConnEndType
pub const CONN_END_POINT: u32 = 0;
pub const CONN_END_SHAPE_PIN: u32 = 1;
pub const CONN_END_JUNCTION: u32 = 2;
pub const CONN_END_EMPTY: u32 = 3;

// upstream: router.h runningTo / runningFrom / runningToAndFrom
pub const RUNNING_TO: u32 = 1;
pub const RUNNING_FROM: u32 = 2;
pub const RUNNING_TO_AND_FROM: u32 = RUNNING_TO | RUNNING_FROM;

// upstream: router.h enum RoutingParameter (typedef PenaltyType)
pub const SEGMENT_PENALTY: u32 = 0;
pub const ANGLE_PENALTY: u32 = 1;
pub const CROSSING_PENALTY: u32 = 2;
pub const CLUSTER_CROSSING_PENALTY: u32 = 3;
pub const FIXED_SHARED_PATH_PENALTY: u32 = 4;
pub const PORT_DIRECTION_PENALTY: u32 = 5;
pub const SHAPE_BUFFER_DISTANCE: u32 = 6;
pub const IDEAL_NUDGING_DISTANCE: u32 = 7;
pub const REVERSE_DIRECTION_PENALTY: u32 = 8;

// upstream: router.h zeroParamValue / chooseSensibleParamValue
pub const ZERO_PARAM_VALUE: f64 = 0.0;
pub const CHOOSE_SENSIBLE_PARAM_VALUE: f64 = -1.0;

// upstream: router.h enum RoutingOption
pub const NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES: u32 = 0;
pub const IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS: u32 = 1;
pub const PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS: u32 = 2;
pub const NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS: u32 = 3;
pub const PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP: u32 = 4;
pub const IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS: u32 = 5;
pub const NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT: u32 = 6;

// upstream: connectionpin.h ATTACH_POS_*
pub const ATTACH_POS_TOP: f64 = 0.0;
pub const ATTACH_POS_CENTRE: f64 = 0.5;
pub const ATTACH_POS_BOTTOM: f64 = 1.0;
pub const ATTACH_POS_LEFT: f64 = ATTACH_POS_TOP;
pub const ATTACH_POS_RIGHT: f64 = ATTACH_POS_BOTTOM;
pub const ATTACH_POS_MIN_OFFSET: f64 = 0.0;
pub const ATTACH_POS_MAX_OFFSET: f64 = -1.0;

// upstream: connectionpin.h CONNECTIONPIN_UNSET / CONNECTIONPIN_CENTRE
pub const CONNECTION_PIN_UNSET: u32 = i32::MAX as u32;
pub const CONNECTION_PIN_CENTRE: u32 = CONNECTION_PIN_UNSET - 1;

// upstream: shape.h enum ShapeTransformationType
pub const TRANSFORMATION_TYPE_CW90: u32 = 0;
pub const TRANSFORMATION_TYPE_CW180: u32 = 1;
pub const TRANSFORMATION_TYPE_CW270: u32 = 2;
pub const TRANSFORMATION_TYPE_FLIP_X: u32 = 3;
pub const TRANSFORMATION_TYPE_FLIP_Y: u32 = 4;

// upstream: geomtypes.h XDIM / YDIM (JunctionRef::preferOrthogonalDimension)
pub const XDIM: u32 = 0;
pub const YDIM: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoid;
    use crate::avoid::connector::ConnType;
    use crate::avoid::connend::ConnEndType;
    use crate::avoid::router::{RoutingOption, RoutingParameter};
    use crate::avoid::shape::ShapeTransformationType;

    #[test]
    fn constants_match_core() {
        assert_eq!(POLY_LINE_ROUTING, avoid::router::POLY_LINE_ROUTING);
        assert_eq!(ORTHOGONAL_ROUTING, avoid::router::ORTHOGONAL_ROUTING);
        assert_eq!(CONN_TYPE_NONE, ConnType::ConnTypeNone as u32);
        assert_eq!(CONN_TYPE_POLY_LINE, ConnType::ConnTypePolyLine as u32);
        assert_eq!(CONN_TYPE_ORTHOGONAL, ConnType::ConnTypeOrthogonal as u32);
        assert_eq!(CONN_DIR_NONE, avoid::connend::CONN_DIR_NONE);
        assert_eq!(CONN_DIR_UP, avoid::connend::CONN_DIR_UP);
        assert_eq!(CONN_DIR_DOWN, avoid::connend::CONN_DIR_DOWN);
        assert_eq!(CONN_DIR_LEFT, avoid::connend::CONN_DIR_LEFT);
        assert_eq!(CONN_DIR_RIGHT, avoid::connend::CONN_DIR_RIGHT);
        assert_eq!(CONN_DIR_ALL, avoid::connend::CONN_DIR_ALL);
        assert_eq!(CONN_END_POINT, ConnEndType::ConnEndPoint as u32);
        assert_eq!(CONN_END_SHAPE_PIN, ConnEndType::ConnEndShapePin as u32);
        assert_eq!(CONN_END_JUNCTION, ConnEndType::ConnEndJunction as u32);
        assert_eq!(CONN_END_EMPTY, ConnEndType::ConnEndEmpty as u32);
        assert_eq!(RUNNING_TO, avoid::router::RUNNING_TO);
        assert_eq!(RUNNING_FROM, avoid::router::RUNNING_FROM);
        assert_eq!(CHOOSE_SENSIBLE_PARAM_VALUE, avoid::router::CHOOSE_SENSIBLE_PARAM_VALUE);
        let params = [
            SEGMENT_PENALTY,
            ANGLE_PENALTY,
            CROSSING_PENALTY,
            CLUSTER_CROSSING_PENALTY,
            FIXED_SHARED_PATH_PENALTY,
            PORT_DIRECTION_PENALTY,
            SHAPE_BUFFER_DISTANCE,
            IDEAL_NUDGING_DISTANCE,
            REVERSE_DIRECTION_PENALTY,
        ];
        for (i, p) in RoutingParameter::ALL.iter().enumerate() {
            assert_eq!(params[i], *p as u32);
        }
        let options = [
            NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES,
            IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS,
            PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS,
            NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS,
            PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP,
            IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS,
            NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT,
        ];
        for (i, o) in RoutingOption::ALL.iter().enumerate() {
            assert_eq!(options[i], *o as u32);
        }
        assert_eq!(ATTACH_POS_CENTRE, avoid::connectionpin::ATTACH_POS_CENTRE);
        assert_eq!(ATTACH_POS_MAX_OFFSET, avoid::connectionpin::ATTACH_POS_MAX_OFFSET);
        assert_eq!(CONNECTION_PIN_UNSET, avoid::connectionpin::CONNECTIONPIN_UNSET);
        assert_eq!(CONNECTION_PIN_CENTRE, avoid::connectionpin::CONNECTIONPIN_CENTRE);
        assert_eq!(TRANSFORMATION_TYPE_CW90, ShapeTransformationType::TransformationTypeCW90 as u32);
        assert_eq!(TRANSFORMATION_TYPE_FLIP_Y, ShapeTransformationType::TransformationTypeFlipY as u32);
        assert_eq!(XDIM as usize, avoid::geomtypes::XDIM);
        assert_eq!(YDIM as usize, avoid::geomtypes::YDIM);
    }
}
