pub(crate) const EPS: f64 = 1e-9;
pub(crate) const ROUTING_PARAMETER_COUNT: usize = 9;
pub(crate) const ROUTING_OPTION_COUNT: usize = 7;

pub const POLY_LINE_ROUTING: u32 = 1;
pub const ORTHOGONAL_ROUTING: u32 = 2;

pub const CONN_TYPE_NONE: u32 = 0;
pub const CONN_TYPE_POLY_LINE: u32 = 1;
pub const CONN_TYPE_ORTHOGONAL: u32 = 2;

pub const CONN_DIR_NONE: u32 = 0;
pub const CONN_DIR_UP: u32 = 1;
pub const CONN_DIR_DOWN: u32 = 2;
pub const CONN_DIR_LEFT: u32 = 4;
pub const CONN_DIR_RIGHT: u32 = 8;
pub const CONN_DIR_ALL: u32 = 15;

pub const RUNNING_TO: u32 = 1;
pub const RUNNING_FROM: u32 = 2;
pub const RUNNING_TO_AND_FROM: u32 = RUNNING_TO | RUNNING_FROM;

pub const SEGMENT_PENALTY: u32 = 0;
pub const ANGLE_PENALTY: u32 = 1;
pub const CROSSING_PENALTY: u32 = 2;
pub const CLUSTER_CROSSING_PENALTY: u32 = 3;
pub const FIXED_SHARED_PATH_PENALTY: u32 = 4;
pub const PORT_DIRECTION_PENALTY: u32 = 5;
pub const SHAPE_BUFFER_DISTANCE: u32 = 6;
pub const IDEAL_NUDGING_DISTANCE: u32 = 7;
pub const REVERSE_DIRECTION_PENALTY: u32 = 8;

pub const NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES: u32 = 0;
pub const IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS: u32 = 1;
pub const PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS: u32 = 2;
pub const NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS: u32 = 3;
pub const PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP: u32 = 4;
pub const IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS: u32 = 5;
pub const NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT: u32 = 6;

pub const ATTACH_POS_TOP: f64 = 0.0;
pub const ATTACH_POS_CENTRE: f64 = 0.5;
pub const ATTACH_POS_BOTTOM: f64 = 1.0;
pub const ATTACH_POS_LEFT: f64 = ATTACH_POS_TOP;
pub const ATTACH_POS_RIGHT: f64 = ATTACH_POS_BOTTOM;
pub const ATTACH_POS_MIN_OFFSET: f64 = 0.0;
pub const ATTACH_POS_MAX_OFFSET: f64 = -1.0;

pub const CONNECTION_PIN_UNSET: u32 = i32::MAX as u32;
pub const CONNECTION_PIN_CENTRE: u32 = CONNECTION_PIN_UNSET - 1;

pub const TRANSFORMATION_TYPE_CW90: u32 = 0;
pub const TRANSFORMATION_TYPE_CW180: u32 = 1;
pub const TRANSFORMATION_TYPE_CW270: u32 = 2;
pub const TRANSFORMATION_TYPE_FLIP_X: u32 = 3;
pub const TRANSFORMATION_TYPE_FLIP_Y: u32 = 4;

pub(crate) fn sensible_routing_parameter(parameter: u32) -> f64 {
    match parameter {
        SEGMENT_PENALTY => 50.0,
        ANGLE_PENALTY => 50.0,
        CROSSING_PENALTY => 200.0,
        CLUSTER_CROSSING_PENALTY => 4000.0,
        FIXED_SHARED_PATH_PENALTY => 110.0,
        PORT_DIRECTION_PENALTY => 100.0,
        SHAPE_BUFFER_DISTANCE => 0.0,
        IDEAL_NUDGING_DISTANCE => 4.0,
        REVERSE_DIRECTION_PENALTY => 0.0,
        _ => 0.0,
    }
}

pub(crate) fn default_routing_parameter(parameter: u32) -> f64 {
    match parameter {
        SEGMENT_PENALTY => 10.0,
        CLUSTER_CROSSING_PENALTY => 4000.0,
        IDEAL_NUDGING_DISTANCE => 4.0,
        _ => 0.0,
    }
}

pub(crate) fn default_routing_options() -> [bool; ROUTING_OPTION_COUNT] {
    let mut options = [false; ROUTING_OPTION_COUNT];
    options[IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS as usize] = true;
    options[PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP as usize] = true;
    options[NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT as usize] = true;
    options
}
