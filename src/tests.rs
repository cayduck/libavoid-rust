use crate::constants::*;
use crate::geometry::{
    angle, colinear, corner_side, euclidean_distance, in_poly, in_poly_gen, in_valid_region,
    manhattan_distance, point_on_line, projection, ray_intersect_point, rotational_angle,
    segment_intersect, segment_intersect_point, total_length, Point, Polygon, Rect, DONT_INTERSECT,
    DO_INTERSECT, PARALLEL, XDIM, YDIM,
};
use crate::graph::VisibilityGraph;
use crate::model::Endpoint;
use crate::orthogonal::{
    clear_segment, route_crossing_count, route_shared_path_count, Direction, OrthogonalObstacle,
    RoutingCostSettings,
};
use crate::router::{polyline_transition_cost, AvoidRouter};
use crate::scanline::{
    nearest_obstacle_limits, order_scan_events, ordered_axis_lines, ScanDimension, ScanEvent,
    ScanEventType, ScanObstacleLimits,
};
use crate::vertices::{
    VertId, VertInf, VertInfList, PROP_CONNECTION_PIN, PROP_CONN_CHECKPOINT, PROP_CONN_POINT,
    PROP_DUMMY_PIN_HELPER, PROP_ORTH_SHAPE_EDGE, VERT_ID_SRC, VERT_ID_TAR, VERT_ID_UNASSIGNED,
};
use crate::visibility::{build_static_visibility_graph, same_axis, StaticVisibilityGraph};

fn buffered_rect_obstacle(
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    offset: f64,
) -> OrthogonalObstacle {
    let bounds = Polygon::rectangle(left, top, right, bottom)
        .offset_bounding_box(offset)
        .unwrap()
        .to_rect();
    OrthogonalObstacle {
        polygon: bounds.to_polygon(),
        bounds,
    }
}

#[test]
fn routes_straight_when_unblocked() {
    let mut router = AvoidRouter::new(10.0);
    router.add_connector("a".to_string(), 0.0, 0.0, 100.0, 0.0);

    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );
}

#[test]
fn routes_around_rectangular_obstacle() {
    let mut router = AvoidRouter::new(10.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 120.0, 120.0);
    router.add_connector("a".to_string(), 0.0, 80.0, 200.0, 80.0);

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 0.0, y: 80.0 }));
    assert_eq!(route.last(), Some(&Point { x: 200.0, y: 80.0 }));
    assert!(route.len() > 2);
    for segment in route.windows(2) {
        assert!(clear_segment(
            segment[0],
            segment[1],
            &[buffered_rect_obstacle(40.0, 40.0, 120.0, 120.0, 10.0)]
        ));
    }
}

#[test]
fn crossing_penalty_changes_orthogonal_route_choice() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 0.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 0.0));
    router.add_connector("fixed".to_string(), 50.0, -10.0, 50.0, 10.0);
    assert!(router.set_connector_fixed_route("fixed".to_string(), vec![50.0, -10.0, 50.0, 10.0],));
    router.add_connector("candidate".to_string(), 0.0, 0.0, 100.0, 0.0);

    assert_eq!(
        router.route(&router.connectors[1]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );

    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1000.0));
    let avoiding = router.route(&router.connectors[1]);
    assert_eq!(avoiding.first(), Some(&Point { x: 0.0, y: 0.0 }));
    assert_eq!(avoiding.last(), Some(&Point { x: 100.0, y: 0.0 }));
    assert!(avoiding.len() > 2);
    for segment in avoiding.windows(2) {
        assert_eq!(
            route_crossing_count(
                segment[0],
                segment[1],
                &[vec![
                    Point { x: 50.0, y: -10.0 },
                    Point { x: 50.0, y: 10.0 }
                ]]
            ),
            0
        );
    }
}

#[test]
fn fixed_shared_path_penalty_changes_orthogonal_route_choice() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 0.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 0.0));
    assert!(router.set_routing_parameter(FIXED_SHARED_PATH_PENALTY, 0.0));
    router.add_connector("fixed".to_string(), 40.0, 0.0, 60.0, 20.0);
    assert!(router
        .set_connector_fixed_route("fixed".to_string(), vec![40.0, 0.0, 60.0, 0.0, 60.0, 20.0],));
    router.add_connector("candidate".to_string(), 0.0, 0.0, 100.0, 0.0);

    assert_eq!(
        router.route(&router.connectors[1]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );

    assert!(router.set_routing_parameter(FIXED_SHARED_PATH_PENALTY, 1000.0));
    let avoiding = router.route(&router.connectors[1]);
    let fixed_route = vec![
        Point { x: 40.0, y: 0.0 },
        Point { x: 60.0, y: 0.0 },
        Point { x: 60.0, y: 20.0 },
    ];
    assert_eq!(avoiding.first(), Some(&Point { x: 0.0, y: 0.0 }));
    assert_eq!(avoiding.last(), Some(&Point { x: 100.0, y: 0.0 }));
    assert!(avoiding.len() > 2);
    for segment in avoiding.windows(2) {
        assert_eq!(
            route_shared_path_count(segment[0], segment[1], &[fixed_route.clone()]),
            0
        );
    }
}

#[test]
fn upstream_penalty_rerouting01_has_no_crossings() {
    let mut router = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 10.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1e9));
    assert!(router.set_routing_parameter(CLUSTER_CROSSING_PENALTY, 4000.0));
    assert!(router.set_routing_parameter(FIXED_SHARED_PATH_PENALTY, 0.0));
    assert!(router.set_routing_parameter(PORT_DIRECTION_PENALTY, 0.0));
    assert!(router.set_routing_parameter(SHAPE_BUFFER_DISTANCE, 0.0));
    assert!(router.set_routing_parameter(IDEAL_NUDGING_DISTANCE, 4.0));
    assert!(router.set_routing_parameter(REVERSE_DIRECTION_PENALTY, 0.0));
    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES, false,));
    assert!(router.set_routing_option(IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS, true,));
    assert!(router.set_routing_option(PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS, false,));
    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS, false,));
    assert!(router.set_routing_option(PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP, true,));
    assert!(router.set_routing_option(
        IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS,
        false,
    ));
    assert!(router.set_routing_option(NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT, true,));

    for (id, left, top, right, bottom) in [
        ("shape-1a", 449.0, 491.0, 479.0, 521.0),
        ("shape-1b", 449.0, 583.0, 479.0, 613.0),
        ("shape-2", 354.0, 583.0, 384.0, 613.0),
        ("shape-3", 354.0, 354.0, 384.0, 384.0),
        ("shape-4", 252.5, 354.0, 282.5, 384.0),
        ("shape-5", 252.5, 583.0, 282.5, 613.0),
        ("shape-6", 151.0, 583.0, 181.0, 613.0),
        ("shape-7", 151.0, 130.0, 181.0, 160.0),
        ("shape-8", 520.0, 491.0, 550.0, 521.0),
        ("shape-9", 520.0, 354.0, 550.0, 384.0),
        ("shape-10", 633.0, 354.0, 663.0, 384.0),
        ("shape-11", 633.0, 583.0, 663.0, 613.0),
        ("shape-12", 741.0, 583.0, 771.0, 613.0),
        ("shape-13", 741.0, 130.0, 771.0, 160.0),
        ("shape-14", 459.495, 481.99, 468.505, 491.0),
        ("shape-15", 212.495, 481.99, 221.505, 491.0),
        ("shape-16", 701.99, 481.99, 711.0, 491.0),
    ] {
        router.add_shape(id.to_string(), left, top, right, bottom);
    }

    for (id, sx, sy, dx, dy) in [
        ("17", 464.0, 598.0, 369.0, 598.0),
        ("18", 267.5, 598.0, 166.0, 598.0),
        ("19", 369.0, 598.0, 369.0, 369.0),
        ("20", 464.0, 506.0, 464.0, 598.0),
        ("21", 756.0, 598.0, 756.0, 145.0),
        ("22", 369.0, 369.0, 267.5, 369.0),
        ("23", 166.0, 145.0, 756.0, 145.0),
        ("25", 535.0, 369.0, 535.0, 506.0),
        ("26", 648.0, 598.0, 648.0, 369.0),
        ("28", 648.0, 598.0, 756.0, 598.0),
        ("29", 166.0, 598.0, 166.0, 145.0),
        ("30", 648.0, 369.0, 535.0, 369.0),
        ("31", 267.5, 369.0, 267.5, 598.0),
        ("32", 535.0, 506.0, 464.0, 506.0),
    ] {
        router.add_connector(id.to_string(), sx, sy, dx, dy);
        assert!(router.set_connector_routing_type(id.to_string(), CONN_TYPE_ORTHOGONAL));
        assert!(router.set_connector_fixed_route(id.to_string(), vec![sx, sy, dx, dy]));
    }

    router.add_connector("24".to_string(), 464.0, 486.495, 706.495, 486.495);
    assert!(router.set_connector_routing_type("24".to_string(), CONN_TYPE_ORTHOGONAL));
    router.add_connector("27".to_string(), 464.0, 486.495, 217.0, 486.495);
    assert!(router.set_connector_routing_type("27".to_string(), CONN_TYPE_ORTHOGONAL));

    assert!(router.process_transaction());
    assert_eq!(router.exists_crossings(false), 0);
}

#[test]
fn upstream_forward_flowing_connectors01_keeps_connector6_short() {
    let mut router = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 10.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1000.0));
    assert!(router.set_routing_parameter(CLUSTER_CROSSING_PENALTY, 4000.0));
    assert!(router.set_routing_parameter(FIXED_SHARED_PATH_PENALTY, 0.0));
    assert!(router.set_routing_parameter(PORT_DIRECTION_PENALTY, 100.0));
    assert!(router.set_routing_parameter(SHAPE_BUFFER_DISTANCE, 1.0));
    assert!(router.set_routing_parameter(IDEAL_NUDGING_DISTANCE, 10.0));
    assert!(router.set_routing_parameter(REVERSE_DIRECTION_PENALTY, 500.0));
    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES, false,));
    assert!(router.set_routing_option(IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS, true,));
    assert!(router.set_routing_option(PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS, false,));
    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS, false,));
    assert!(router.set_routing_option(PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP, true,));
    assert!(router.set_routing_option(
        IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS,
        true,
    ));

    for (id, left, top, right, bottom) in [
        ("5", 501.0, 345.0, 421.0, 404.0),
        ("6", 94.0, 251.5, 12.0, 315.5),
        ("7", 634.366, 262.0, 416.366, 305.0),
        ("8", 324.0, 147.167, 236.0, 206.167),
        ("9", 816.0, 353.167, 735.0, 412.167),
        ("10", 981.0, 263.833, 828.0, 321.833),
        ("11", 1011.49, 361.833, 834.489, 419.833),
        ("12", 511.0, 155.333, 422.0, 214.333),
        ("13", 690.0, 66.0, 523.0, 124.0),
        ("14", 720.212, 164.0, 529.212, 222.0),
        ("15", 217.0, 336.833, 98.0, 395.833),
        ("16", 413.0, 147.167, 336.0, 206.167),
        ("17", 306.0, 336.833, 229.0, 395.833),
        ("18", 175.0, 139.0, 98.0, 198.0),
        ("19", 409.0, 399.333, 298.0, 458.333),
        ("20", 224.0, 40.0, 106.0, 99.0),
        ("21", 604.0, 345.0, 513.0, 404.0),
    ] {
        router.add_shape(id.to_string(), left, top, right, bottom);
    }

    for (shape, pin, class_id, x_offset, y_offset, directions) in [
        ("5", "5", 5, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("5", "6", 6, 0.0, 0.79096, CONN_DIR_LEFT),
        ("5", "7", 7, 0.0, 0.514124, CONN_DIR_LEFT),
        ("6", "8", 8, 1.0, 0.640625, CONN_DIR_RIGHT),
        ("6", "9", 9, 0.0, 0.640625, CONN_DIR_LEFT),
        ("7", "10", 10, 1.0, 0.709302, CONN_DIR_RIGHT),
        ("7", "11", 11, 0.0, 0.709302, CONN_DIR_LEFT),
        ("8", "12", 12, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("8", "13", 13, 0.0, 0.79096, CONN_DIR_LEFT),
        ("8", "14", 14, 0.0, 0.514124, CONN_DIR_LEFT),
        ("9", "15", 15, 1.0, 0.514124, CONN_DIR_RIGHT),
        ("9", "16", 16, 1.0, 0.79096, CONN_DIR_RIGHT),
        ("9", "17", 17, 0.0, 0.79096, CONN_DIR_LEFT),
        ("9", "18", 18, 0.0, 0.514124, CONN_DIR_LEFT),
        ("10", "19", 19, 0.0, 0.655172, CONN_DIR_LEFT),
        ("11", "20", 20, 0.0, 0.655172, CONN_DIR_LEFT),
        ("12", "21", 21, 1.0, 0.514124, CONN_DIR_RIGHT),
        ("12", "22", 22, 1.0, 0.79096, CONN_DIR_RIGHT),
        ("12", "23", 23, 0.0, 0.79096, CONN_DIR_LEFT),
        ("12", "24", 24, 0.0, 0.514124, CONN_DIR_LEFT),
        ("13", "25", 25, 0.0, 0.655172, CONN_DIR_LEFT),
        ("14", "26", 26, 0.0, 0.655172, CONN_DIR_LEFT),
        ("15", "27", 27, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("15", "28", 28, 0.0, 0.652542, CONN_DIR_LEFT),
        ("16", "29", 29, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("16", "30", 30, 0.0, 0.652542, CONN_DIR_LEFT),
        ("17", "31", 31, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("17", "32", 32, 0.0, 0.652542, CONN_DIR_LEFT),
        ("18", "33", 33, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("18", "34", 34, 0.0, 0.652542, CONN_DIR_LEFT),
        ("19", "35", 35, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("20", "36", 36, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("21", "37", 37, 1.0, 0.652542, CONN_DIR_RIGHT),
        ("21", "38", 38, 0.0, 0.652542, CONN_DIR_LEFT),
    ] {
        assert!(router.add_shape_connection_pin_with_options(
            shape.to_string(),
            pin.to_string(),
            class_id,
            x_offset,
            y_offset,
            true,
            0.0,
            directions,
            f64::NAN,
            false,
        ));
    }

    for (id, src_shape, src_class, dst_shape, dst_class) in [
        ("1", "5", 5, "21", 38),
        ("2", "6", 8, "18", 34),
        ("3", "6", 8, "15", 28),
        ("4", "6", 8, "12", 23),
        ("5", "6", 8, "7", 11),
        ("6", "7", 10, "9", 17),
        ("7", "8", 12, "16", 30),
        ("8", "9", 15, "10", 19),
        ("9", "9", 16, "11", 20),
        ("10", "12", 21, "13", 25),
        ("11", "12", 22, "14", 26),
        ("12", "15", 27, "17", 32),
        ("13", "16", 29, "12", 24),
        ("14", "17", 31, "5", 7),
        ("15", "18", 33, "8", 14),
        ("16", "19", 35, "5", 7),
        ("17", "20", 36, "8", 14),
        ("18", "21", 37, "9", 18),
    ] {
        router.add_connector(id.to_string(), 0.0, 0.0, 0.0, 0.0);
        assert!(router.set_connector_routing_type(id.to_string(), CONN_TYPE_ORTHOGONAL));
        assert!(router.set_connector_source_pin_class_endpoint(
            id.to_string(),
            src_shape.to_string(),
            src_class,
        ));
        assert!(router.set_connector_dest_pin_class_endpoint(
            id.to_string(),
            dst_shape.to_string(),
            dst_class,
        ));
    }

    assert!(router.process_transaction());
    let connector6 = router
        .connectors
        .iter()
        .find(|connector| connector.id == "6")
        .unwrap();
    assert_eq!(connector6.display_route.len(), 4);
}

#[test]
fn crossing_penalty_change_invalidates_and_transaction_reroutes_sequentially() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 0.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 0.0));
    router.add_connector("vertical".to_string(), 50.0, -10.0, 50.0, 10.0);
    router.add_connector("horizontal".to_string(), 0.0, 0.0, 100.0, 0.0);
    assert!(router.process_transaction());
    assert_eq!(
        router.connectors[0].display_route,
        vec![Point { x: 50.0, y: -10.0 }, Point { x: 50.0, y: 10.0 }]
    );
    assert_eq!(
        router.connectors[1].display_route,
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );

    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1000.0));
    assert!(router
        .connectors
        .iter()
        .all(|connector| connector.needs_reroute));
    assert!(router.process_transaction());

    let vertical = router.connectors[0].display_route.clone();
    let horizontal = &router.connectors[1].display_route;
    assert_eq!(vertical.len(), 2);
    assert_eq!(horizontal.first(), Some(&Point { x: 0.0, y: 0.0 }));
    assert_eq!(horizontal.last(), Some(&Point { x: 100.0, y: 0.0 }));
    assert!(horizontal.len() > 2);
    for segment in horizontal.windows(2) {
        assert_eq!(
            route_crossing_count(segment[0], segment[1], &[vertical.clone()]),
            0
        );
    }
}

#[test]
fn hate_crossings_connector_reroutes_after_non_hating_crossing_partner() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 0.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1000.0));
    router.add_connector("hates".to_string(), 0.0, 0.0, 100.0, 0.0);
    assert!(router.set_connector_hate_crossings("hates".to_string(), true));
    router.add_connector("normal".to_string(), 50.0, -10.0, 50.0, 10.0);

    assert!(router.process_transaction());

    let hates = router
        .connectors
        .iter()
        .find(|connector| connector.id == "hates")
        .unwrap()
        .display_route
        .clone();
    let normal = router
        .connectors
        .iter()
        .find(|connector| connector.id == "normal")
        .unwrap()
        .display_route
        .clone();
    assert_eq!(
        normal,
        vec![Point { x: 50.0, y: -10.0 }, Point { x: 50.0, y: 10.0 }]
    );
    assert_eq!(hates.first(), Some(&Point { x: 0.0, y: 0.0 }));
    assert_eq!(hates.last(), Some(&Point { x: 100.0, y: 0.0 }));
    assert!(hates.len() > 2);
    for segment in hates.windows(2) {
        assert_eq!(
            route_crossing_count(segment[0], segment[1], &[normal.clone()]),
            0
        );
    }
}

#[test]
fn clusters_are_scene_objects_and_constrain_outside_routes() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.add_cluster_rectangle("cluster".to_string(), 70.0, 70.0, 30.0, 30.0));
    assert!(!router.object_id_is_unused_public("cluster".to_string()));
    assert_eq!(
        router.cluster_bounding_box("cluster".to_string()),
        "{\"left\":30,\"top\":30,\"right\":70,\"bottom\":70}"
    );
    assert_eq!(
        router.cluster_polygon("cluster".to_string()),
        "[{\"x\":70,\"y\":30},{\"x\":70,\"y\":70},{\"x\":30,\"y\":70},{\"x\":30,\"y\":30}]"
    );
    assert_eq!(
        router.cluster_rectangular_polygon("cluster".to_string()),
        "[{\"x\":70,\"y\":30},{\"x\":70,\"y\":70},{\"x\":30,\"y\":70},{\"x\":30,\"y\":30}]"
    );
    assert_eq!(
        router.cluster_rectangular_polygon("missing".to_string()),
        "null"
    );
    assert!(!router.add_cluster_rectangle("invalid".to_string(), f64::NAN, 0.0, 1.0, 1.0));

    router.add_connector("outside".to_string(), 0.0, 50.0, 100.0, 50.0);
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 0.0, y: 50.0 }));
    assert_eq!(route.last(), Some(&Point { x: 100.0, y: 50.0 }));
    assert!(route.len() > 2);
    for segment in route.windows(2) {
        assert!(!router.clusters[0]
            .polygon
            .intersects_axis_segment(segment[0], segment[1]));
    }

    assert!(router.set_cluster_rectangle("cluster".to_string(), 60.0, 60.0, 40.0, 40.0));
    assert_eq!(
        router.cluster_bounding_box("cluster".to_string()),
        "{\"left\":30,\"top\":30,\"right\":70,\"bottom\":70}"
    );
    assert!(!router.connector_needs_repaint("outside".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.cluster_bounding_box("cluster".to_string()),
        "{\"left\":40,\"top\":40,\"right\":60,\"bottom\":60}"
    );
    assert!(router.connector_needs_repaint("outside".to_string()));
    let updated_route = router.display_connector_route("outside".to_string());
    assert_ne!(updated_route, "[{\"x\":0,\"y\":50},{\"x\":100,\"y\":50}]");
    assert!(router.remove_cluster("cluster".to_string()));
    assert!(!router.object_id_is_unused_public("cluster".to_string()));
    assert!(router.process_transaction());
    assert!(router.object_id_is_unused_public("cluster".to_string()));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 50.0 }, Point { x: 100.0, y: 50.0 }]
    );

    let mut contained = AvoidRouter::new(0.0);
    assert!(contained.add_cluster(
        "concave".to_string(),
        vec![
            0.0, 0.0, 100.0, 0.0, 100.0, 30.0, 30.0, 30.0, 30.0, 70.0, 100.0, 70.0, 100.0, 100.0,
            0.0, 100.0,
        ]
    ));
    assert_eq!(
        contained.cluster_rectangular_polygon("concave".to_string()),
        "[{\"x\":100,\"y\":0},{\"x\":100,\"y\":100},{\"x\":0,\"y\":100},{\"x\":0,\"y\":0}]"
    );
    contained.add_connector("inside".to_string(), 80.0, 20.0, 80.0, 80.0);
    let inside_route = contained.route(&contained.connectors[0]);
    assert!(inside_route.len() > 2);
    for segment in inside_route.windows(2) {
        let midpoint = Point {
            x: (segment[0].x + segment[1].x) / 2.0,
            y: (segment[0].y + segment[1].y) / 2.0,
        };
        assert!(contained.clusters[0].polygon.contains_point(midpoint, true));
    }
}

#[test]
fn cluster_inside_to_outside_routes_cross_boundary_explicitly() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.add_cluster(
        "cluster".to_string(),
        vec![0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0]
    ));
    router.add_connector("leaving".to_string(), 50.0, 50.0, 150.0, 50.0);

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 50.0, y: 50.0 }));
    assert_eq!(route.last(), Some(&Point { x: 150.0, y: 50.0 }));
    assert!(route.len() > 2);
    assert!(route
        .iter()
        .any(|point| *point == Point { x: 100.0, y: 50.0 }));
    assert!(route[1..]
        .iter()
        .any(|point| !router.clusters[0].polygon.contains_point(*point, false)));
}

#[test]
fn cluster_crossing_penalty_controls_cluster_boundary_routing() {
    // Matches the upstream router.h clusterCrossingPenalty contract: positive
    // values make cluster boundary crossings part of the route cost model.
    let mut router = AvoidRouter::new(0.0);
    assert!(router.add_cluster_rectangle("cluster".to_string(), 40.0, 40.0, 80.0, 80.0));
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);

    let avoiding_route = router.route(&router.connectors[0]);
    assert!(avoiding_route.len() > 2);
    for segment in avoiding_route.windows(2) {
        assert!(!router.clusters[0]
            .polygon
            .intersects_axis_segment(segment[0], segment[1]));
    }

    assert!(router.set_routing_parameter(CLUSTER_CROSSING_PENALTY, 0.0));
    let crossing_route = router.route(&router.connectors[0]);
    assert_eq!(
        crossing_route,
        vec![Point { x: 0.0, y: 60.0 }, Point { x: 120.0, y: 60.0 }]
    );
}

#[test]
fn polyline_connector_routes_around_rectangular_obstacle() {
    let mut router = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    router.set_routing_parameter(SHAPE_BUFFER_DISTANCE, 10.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 120.0, 120.0);
    router.add_connector("a".to_string(), 0.0, 80.0, 200.0, 80.0);
    assert!(router.set_connector_routing_type("a".to_string(), CONN_TYPE_POLY_LINE));

    let route = router.route(&router.connectors[0]);
    let obstacle = buffered_rect_obstacle(40.0, 40.0, 120.0, 120.0, 10.0);
    assert_eq!(route.first(), Some(&Point { x: 0.0, y: 80.0 }));
    assert_eq!(route.last(), Some(&Point { x: 200.0, y: 80.0 }));
    assert!(route.len() > 2);
    assert!(route
        .windows(2)
        .any(|segment| (segment[0].x - segment[1].x).abs() >= EPS
            && (segment[0].y - segment[1].y).abs() >= EPS));
    let graph = build_static_visibility_graph(
        &[obstacle.polygon.clone()],
        &[Point { x: 0.0, y: 80.0 }, Point { x: 200.0, y: 80.0 }],
    );
    for segment in route.windows(2) {
        assert!(!obstacle.polygon.intersects_segment(segment[0], segment[1]));
        let from = graph
            .vertices
            .iter()
            .find_map(|(index, vertex)| (vertex.point == segment[0]).then_some(index))
            .unwrap();
        let to = graph
            .vertices
            .iter()
            .find_map(|(index, vertex)| (vertex.point == segment[1]).then_some(index))
            .unwrap();
        assert!(graph
            .edge_between(from, to)
            .is_some_and(|edge| edge.visible));
    }
}

#[test]
fn polyline_endpoint_directions_filter_first_and_last_visibility_edges() {
    let mut router = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    router.set_routing_parameter(SHAPE_BUFFER_DISTANCE, 10.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 120.0, 120.0);
    router.add_connector_with_endpoint_directions(
        "edge".to_string(),
        0.0,
        80.0,
        CONN_DIR_UP,
        200.0,
        80.0,
        CONN_DIR_UP,
    );
    assert!(router.set_connector_routing_type("edge".to_string(), CONN_TYPE_POLY_LINE));

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 0.0, y: 80.0 }));
    assert_eq!(route.last(), Some(&Point { x: 200.0, y: 80.0 }));
    assert!(route.len() > 2);
    assert!(route[1].y > 80.0);
    assert!(route[route.len() - 2].y > 80.0);
    for segment in route.windows(2) {
        assert!(!buffered_rect_obstacle(40.0, 40.0, 120.0, 120.0, 10.0)
            .polygon
            .intersects_segment(segment[0], segment[1]));
    }
}

#[test]
fn polyline_transition_cost_uses_route_cost_parameters() {
    let settings = RoutingCostSettings {
        segment_penalty: 7.0,
        angle_penalty: 11.0,
        crossing_penalty: 13.0,
        fixed_shared_path_penalty: 17.0,
        reverse_direction_penalty: 19.0,
    };
    let src = Point { x: 0.0, y: 0.0 };
    let dst = Point { x: 100.0, y: 100.0 };

    assert_eq!(
        polyline_transition_cost(
            None,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            10.0,
            src,
            dst,
            &[],
            settings,
        ),
        10.0
    );
    let right_angle_scale = 5.0 * 6.0_f64.log10() / 10.5;
    let right_angle_cost = polyline_transition_cost(
        Some(Point { x: 0.0, y: 0.0 }),
        Point { x: 10.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
        10.0,
        src,
        dst,
        &[],
        settings,
    );
    assert!((right_angle_cost - (10.0 + 7.0 + (11.0 * right_angle_scale))).abs() < EPS);
    assert_eq!(
        polyline_transition_cost(
            Some(Point { x: 0.0, y: 0.0 }),
            Point { x: 10.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            10.0,
            src,
            dst,
            &[],
            settings,
        ),
        10.0
    );
    let double_back_scale = 10.0 * 11.0_f64.log10() / 10.5;
    let double_back_cost = polyline_transition_cost(
        Some(Point { x: 0.0, y: 0.0 }),
        Point { x: 10.0, y: 0.0 },
        Point { x: 0.0, y: 0.0 },
        10.0,
        Point { x: 0.0, y: 0.0 },
        Point { x: 0.0, y: 100.0 },
        &[],
        settings,
    );
    assert!((double_back_cost - (10.0 + (2.0 * 7.0) + (11.0 * double_back_scale))).abs() < EPS);
    assert_eq!(
        polyline_transition_cost(
            None,
            Point { x: 10.0, y: 0.0 },
            Point { x: 0.0, y: 0.0 },
            10.0,
            Point { x: 0.0, y: 0.0 },
            Point { x: 100.0, y: 0.0 },
            &[],
            settings,
        ),
        29.0
    );
    assert_eq!(
        polyline_transition_cost(
            None,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            10.0,
            src,
            dst,
            &[vec![Point { x: 5.0, y: -5.0 }, Point { x: 5.0, y: 5.0 }]],
            settings,
        ),
        23.0
    );
    assert_eq!(
        polyline_transition_cost(
            None,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            10.0,
            src,
            dst,
            &[vec![Point { x: 2.0, y: 0.0 }, Point { x: 8.0, y: 0.0 }]],
            RoutingCostSettings {
                crossing_penalty: 0.0,
                ..settings
            },
        ),
        27.0
    );
}

#[test]
fn route_all_returns_json_object() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("quote\"id".to_string(), 0.0, 0.0, 10.0, 0.0);

    assert_eq!(
        router.route_all(),
        "{\"quote\\\"id\":[{\"x\":0,\"y\":0},{\"x\":10,\"y\":0}]}"
    );
}

#[test]
fn route_all_uses_sequential_penalty_context_for_uncached_routes() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, 0.0));
    assert!(router.set_routing_parameter(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_parameter(CROSSING_PENALTY, 1000.0));
    router.add_connector("vertical".to_string(), 50.0, -10.0, 50.0, 10.0);
    router.add_connector("horizontal".to_string(), 0.0, 0.0, 100.0, 0.0);

    assert_eq!(
        router.route_all(),
        concat!(
            "{\"vertical\":[{\"x\":50,\"y\":-10},{\"x\":50,\"y\":10}],",
            "\"horizontal\":[{\"x\":0,\"y\":0},{\"x\":0,\"y\":10},{\"x\":100,\"y\":10},{\"x\":100,\"y\":0}]}"
        )
    );
}

#[test]
fn connector_repaint_state_follows_process_transaction_and_display_route() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(!router.connector_needs_repaint("edge".to_string()));

    assert!(router.process_transaction());
    assert!(router.connector_needs_repaint("edge".to_string()));
    assert_eq!(
        router.last_transaction_updated_connector_ids(),
        "[\"edge\"]"
    );
    assert_eq!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":0},{\"x\":10,\"y\":0}]"
    );
    assert!(!router.connector_needs_repaint("edge".to_string()));

    assert!(router.set_connector_dest_endpoint("edge".to_string(), 20.0, 0.0));
    assert!(!router.connector_needs_repaint("edge".to_string()));
    assert!(router.process_transaction());
    assert!(router.connector_needs_repaint("edge".to_string()));
    assert_eq!(
        router.last_transaction_updated_connector_ids(),
        "[\"edge\"]"
    );
    assert_eq!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":0},{\"x\":20,\"y\":0}]"
    );
    assert_eq!(
        router.display_connector_route("missing".to_string()),
        "null"
    );
    assert!(!router.process_transaction());
    assert_eq!(router.last_transaction_updated_connector_ids(), "[]");
}

#[test]
fn make_connector_path_invalid_marks_route_for_next_transaction() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(router.process_transaction());
    assert!(router.connector_needs_repaint("edge".to_string()));
    router.display_connector_route("edge".to_string());

    assert!(router.make_connector_path_invalid("edge".to_string()));
    assert!(!router.connector_needs_repaint("edge".to_string()));
    assert!(router.process_transaction());
    assert!(router.connector_needs_repaint("edge".to_string()));
    assert!(!router.make_connector_path_invalid("missing".to_string()));
}

#[test]
fn obstacle_lifecycle_invalidates_cached_connector_routes() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);
    assert!(router.process_transaction());
    assert_eq!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":60},{\"x\":120,\"y\":60}]"
    );
    assert!(!router.connector_needs_repaint("edge".to_string()));

    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    assert!(!router.connector_needs_repaint("edge".to_string()));
    assert!(router.process_transaction());
    assert!(router.connector_needs_repaint("edge".to_string()));
    let blocked_route = router.display_connector_route("edge".to_string());
    assert_ne!(blocked_route, "[{\"x\":0,\"y\":60},{\"x\":120,\"y\":60}]");

    assert!(router.remove_shape("box".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":60},{\"x\":120,\"y\":60}]"
    );

    router.add_junction("mid".to_string(), 60.0, 60.0);
    assert!(router.process_transaction());
    assert_ne!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":60},{\"x\":120,\"y\":60}]"
    );
    assert!(router.remove_junction("mid".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.display_connector_route("edge".to_string()),
        "[{\"x\":0,\"y\":60},{\"x\":120,\"y\":60}]"
    );
}

#[test]
fn connector_can_be_split_at_display_route_segment() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 100.0, 0.0);
    assert!(router.process_transaction());

    assert_eq!(
        router.split_connector_at_segment(
            "edge".to_string(),
            1,
            "junction".to_string(),
            "edge-tail".to_string()
        ),
        "{\"junctionId\":\"junction\",\"connectorId\":\"edge-tail\",\"position\":{\"x\":50,\"y\":0}}"
    );
    assert_eq!(router.connectors.len(), 2);
    assert_eq!(
        router.junction_position("junction".to_string()),
        "{\"x\":50,\"y\":0}"
    );
    assert_eq!(
        router.junction_preferred_orthogonal_dimension("junction".to_string()),
        0
    );
    assert_eq!(
        router.junction_connection_pin_ids("junction".to_string(), "centre".to_string()),
        format!(
            "{{\"objectId\":\"junction\",\"classId\":{}}}",
            CONNECTION_PIN_CENTRE
        )
    );
    assert_eq!(router.junctions[0].preferred_orthogonal_dimension, Some(0));
    assert!(matches!(
        &router.connectors[0].dst,
        Endpoint::Junction { junction_id } if junction_id == "junction"
    ));
    assert!(matches!(
        &router.connectors[1].src,
        Endpoint::Junction { junction_id } if junction_id == "junction"
    ));
    assert!(matches!(
        &router.connectors[1].dst,
        Endpoint::Point {
            point: Point { x: 100.0, y: 0.0 },
            directions: CONN_DIR_ALL
        }
    ));

    assert!(router.process_transaction());
    assert_eq!(
        router.route_connector("edge".to_string()),
        "[{\"x\":0,\"y\":0},{\"x\":50,\"y\":0}]"
    );
    assert_eq!(
        router.route_connector("edge-tail".to_string()),
        "[{\"x\":50,\"y\":0},{\"x\":100,\"y\":0}]"
    );
}

#[test]
fn connector_split_rejects_invalid_segments_and_reused_ids() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 100.0, 0.0);
    router.add_junction("used-junction".to_string(), 10.0, 10.0);
    router.add_connector("used-connector".to_string(), 0.0, 10.0, 100.0, 10.0);
    assert!(router.process_transaction());

    assert_eq!(
        router.split_connector_at_segment(
            "edge".to_string(),
            0,
            "new-junction".to_string(),
            "new-connector".to_string()
        ),
        "null"
    );
    assert_eq!(
        router.split_connector_at_segment(
            "edge".to_string(),
            2,
            "new-junction".to_string(),
            "new-connector".to_string()
        ),
        "null"
    );
    assert_eq!(
        router.split_connector_at_segment(
            "edge".to_string(),
            1,
            "used-junction".to_string(),
            "new-connector".to_string()
        ),
        "null"
    );
    assert_eq!(
        router.split_connector_at_segment(
            "edge".to_string(),
            1,
            "new-junction".to_string(),
            "used-connector".to_string()
        ),
        "null"
    );
}

#[test]
fn junction_preferred_orthogonal_dimension_can_be_set() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("j".to_string(), 0.0, 0.0);

    assert_eq!(
        router.junction_preferred_orthogonal_dimension("j".to_string()),
        -1
    );
    assert!(router.prefer_junction_orthogonal_dimension("j".to_string(), 1));
    assert_eq!(
        router.junction_preferred_orthogonal_dimension("j".to_string()),
        1
    );
    assert!(!router.prefer_junction_orthogonal_dimension("j".to_string(), 2));
    assert!(!router.prefer_junction_orthogonal_dimension("missing".to_string(), 0));
}

#[test]
fn junctions_have_upstream_default_centre_connection_pin() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 10.0, 20.0);
    router.add_junction("right".to_string(), 100.0, 20.0);

    assert_eq!(
        router.junction_connection_pin_position("left".to_string(), "centre".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert_eq!(
        router.junction_connection_pin_directions("left".to_string(), "centre".to_string()),
        CONN_DIR_ALL
    );
    assert_eq!(
        router.junction_connection_pin_ids("left".to_string(), "centre".to_string()),
        format!(
            "{{\"objectId\":\"left\",\"classId\":{}}}",
            CONNECTION_PIN_CENTRE
        )
    );
    assert!(!router.junction_connection_pin_exclusive("left".to_string(), "centre".to_string()));

    assert!(router.add_connector_between_junction_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        CONNECTION_PIN_CENTRE,
        "right".to_string(),
        CONNECTION_PIN_CENTRE,
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        format!(
            concat!(
                "{{\"source\":{{\"type\":\"junctionPin\",\"junctionId\":\"left\",",
                "\"pinId\":\"centre\",\"classId\":{},\"position\":{{\"x\":10,\"y\":20}},",
                "\"directions\":15}},\"destination\":{{\"type\":\"junctionPin\",",
                "\"junctionId\":\"right\",\"pinId\":\"centre\",\"classId\":{},",
                "\"position\":{{\"x\":100,\"y\":20}},\"directions\":15}}}}"
            ),
            CONNECTION_PIN_CENTRE, CONNECTION_PIN_CENTRE
        )
    );

    assert!(router.add_connector_to_junction(
        "plain-junction-edge".to_string(),
        0.0,
        20.0,
        "right".to_string()
    ));
    assert_eq!(
        router.connector_possible_dst_pin_points("plain-junction-edge".to_string()),
        "[{\"x\":100,\"y\":20}]"
    );
}

#[test]
fn junction_preferred_orthogonal_dimension_penalises_matching_pin_directions() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "east".to_string(),
        7,
        CONN_DIR_RIGHT,
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "north".to_string(),
        7,
        CONN_DIR_UP,
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "target".to_string(),
        8,
        CONN_DIR_LEFT,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        7,
        "right".to_string(),
        8,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"east\""));

    assert!(router.prefer_junction_orthogonal_dimension("left".to_string(), 1));
    assert_eq!(
        router.junction_connection_pin_connection_cost("left".to_string(), "east".to_string()),
        1.0
    );
    assert_eq!(
        router.junction_connection_pin_connection_cost("left".to_string(), "north".to_string()),
        0.0
    );
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"east\""));
    assert!(router.process_transaction());
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"north\""));

    router.set_transaction_use(false);
    assert!(router.prefer_junction_orthogonal_dimension("left".to_string(), 0));
    assert_eq!(
        router.junction_connection_pin_connection_cost("left".to_string(), "north".to_string()),
        1.0
    );
}

#[test]
fn junction_fixed_and_recommended_position_match_upstream_state() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("j".to_string(), 10.0, 20.0);

    assert_eq!(
        router.junction_recommended_position("j".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert!(router.set_junction_recommended_position("j".to_string(), 30.0, 40.0));
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert_eq!(
        router.junction_recommended_position("j".to_string()),
        "{\"x\":30,\"y\":40}"
    );
    assert!(!router.set_junction_recommended_position("missing".to_string(), 0.0, 0.0));

    assert!(!router.junction_position_fixed("j".to_string()));
    assert!(router.set_junction_position_fixed("j".to_string(), true));
    assert!(router.junction_position_fixed("j".to_string()));
    assert!(router.process_transaction());
    assert!(router.set_junction_position_fixed("j".to_string(), false));
    assert!(!router.junction_position_fixed("j".to_string()));
    assert!(router.process_transaction());
    assert!(!router.process_transaction());

    assert!(router.move_junction("j".to_string(), 50.0, 60.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.junction_recommended_position("j".to_string()),
        "{\"x\":50,\"y\":60}"
    );
}

#[test]
fn junction_connection_pins_support_class_endpoints_and_follow_junction_moves() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 10.0, 20.0);
    router.add_junction("right".to_string(), 100.0, 20.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "east".to_string(),
        1,
        CONN_DIR_RIGHT
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "west-a".to_string(),
        2,
        CONN_DIR_LEFT
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "west-b".to_string(),
        2,
        CONN_DIR_LEFT
    ));

    assert_eq!(
        router.junction_connection_pin_position("left".to_string(), "east".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert_eq!(
        router.junction_connection_pin_directions("left".to_string(), "east".to_string()),
        CONN_DIR_RIGHT
    );
    assert_eq!(
        router.junction_connection_pin_ids("left".to_string(), "east".to_string()),
        "{\"objectId\":\"left\",\"classId\":1}"
    );
    assert_eq!(
        router.junction_connection_pin_ids("left".to_string(), "missing".to_string()),
        "null"
    );
    assert!(router.add_connector_between_junction_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"junctionPin\",\"junctionId\":\"left\",\"pinId\":\"east\",",
            "\"classId\":1,\"position\":{\"x\":10,\"y\":20},\"directions\":8},",
            "\"destination\":{\"type\":\"junctionPin\",\"junctionId\":\"right\",\"pinId\":\"west-a\",",
            "\"classId\":2,\"position\":{\"x\":100,\"y\":20},\"directions\":4}}"
        )
    );
    assert_eq!(
        router.connector_possible_dst_pin_points("edge".to_string()),
        "[{\"x\":100,\"y\":20}]"
    );

    assert!(router.move_junction("right".to_string(), 120.0, 30.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.junction_connection_pin_position("right".to_string(), "west-a".to_string()),
        "{\"x\":120,\"y\":30}"
    );
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"junctionPin\",\"junctionId\":\"left\",\"pinId\":\"east\",",
            "\"classId\":1,\"position\":{\"x\":10,\"y\":20},\"directions\":8},",
            "\"destination\":{\"type\":\"junctionPin\",\"junctionId\":\"right\",\"pinId\":\"west-a\",",
            "\"classId\":2,\"position\":{\"x\":120,\"y\":30},\"directions\":4}}"
        )
    );

    assert!(router.remove_junction_connection_pin("right".to_string(), "west-a".to_string()));
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"west-b\""));
}

#[test]
fn junction_connection_pin_cost_and_exclusivity_drive_class_selection_transactionally() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "out".to_string(),
        1,
        CONN_DIR_RIGHT
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "expensive".to_string(),
        2,
        CONN_DIR_LEFT
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "cheap".to_string(),
        2,
        CONN_DIR_LEFT
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "spare-out".to_string(),
        3,
        CONN_DIR_RIGHT
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "spare-in".to_string(),
        4,
        CONN_DIR_LEFT
    ));

    assert_eq!(
        router.junction_connection_pin_connection_cost("right".to_string(), "cheap".to_string()),
        0.0
    );
    assert!(router.junction_connection_pin_exclusive("right".to_string(), "cheap".to_string()));
    assert!(router.set_junction_connection_pin_exclusive(
        "left".to_string(),
        "out".to_string(),
        false,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "right".to_string(),
        "expensive".to_string(),
        10.0,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "right".to_string(),
        "cheap".to_string(),
        1.0,
    ));
    assert!(!router.set_junction_connection_pin_connection_cost(
        "right".to_string(),
        "cheap".to_string(),
        -1.0,
    ));
    assert!(!router.set_junction_connection_pin_exclusive(
        "missing".to_string(),
        "cheap".to_string(),
        true,
    ));
    assert!(router
        .junction_connection_pin_connection_cost("right".to_string(), "missing".to_string())
        .is_nan());

    assert!(router.add_connector_between_junction_pin_classes(
        "first".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "junction-spare".to_string(),
        "left".to_string(),
        3,
        "right".to_string(),
        4,
    ));
    assert!(router
        .connector_endpoint_conn_ends("first".to_string())
        .contains("\"pinId\":\"cheap\""));
    assert!(router.process_transaction());
    router.display_connector_route("first".to_string());
    router.display_connector_route("junction-spare".to_string());
    assert!(!router.connector_needs_repaint("first".to_string()));
    assert!(!router.connector_needs_repaint("junction-spare".to_string()));

    assert!(router.set_junction_connection_pin_connection_cost(
        "right".to_string(),
        "cheap".to_string(),
        2.0,
    ));
    assert!(!router.connector_needs_repaint("first".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.last_transaction_updated_connector_ids(),
        "[\"first\"]"
    );
    assert!(router.connector_needs_repaint("first".to_string()));
    assert!(!router.connector_needs_repaint("junction-spare".to_string()));
    router.display_connector_route("first".to_string());

    assert!(router.set_junction_connection_pin_connection_cost(
        "right".to_string(),
        "expensive".to_string(),
        0.25,
    ));
    assert!(router.set_junction_connection_pin_exclusive(
        "right".to_string(),
        "expensive".to_string(),
        false,
    ));
    assert!(!router.junction_connection_pin_exclusive("right".to_string(), "expensive".to_string()));

    assert!(router.add_connector_between_junction_pin_classes(
        "second".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router
        .connector_endpoint_conn_ends("second".to_string())
        .contains("\"pinId\":\"expensive\""));
    assert!(router.add_connector_between_junction_pin_classes(
        "third".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router
        .connector_endpoint_conn_ends("third".to_string())
        .contains("\"pinId\":\"expensive\""));
}

#[test]
fn routes_between_shape_connection_pins() {
    let mut router = AvoidRouter::new(10.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 60.0, 60.0);
    router.add_shape("middle".to_string(), 90.0, -20.0, 150.0, 80.0);
    router.add_shape("right".to_string(), 180.0, 0.0, 240.0, 60.0);
    assert!(router.add_shape_connection_pin("left".to_string(), "east".to_string(), 60.0, 30.0));
    assert!(router.add_shape_connection_pin("right".to_string(), "west".to_string(), 180.0, 30.0));
    assert!(router.add_connector_between_pins(
        "edge".to_string(),
        "left".to_string(),
        "east".to_string(),
        "right".to_string(),
        "west".to_string()
    ));

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 60.0, y: 30.0 }));
    assert_eq!(route.last(), Some(&Point { x: 180.0, y: 30.0 }));
    assert!(route.len() > 2);
    for segment in route.windows(2) {
        assert!(clear_segment(
            segment[0],
            segment[1],
            &[buffered_rect_obstacle(90.0, -20.0, 150.0, 80.0, 10.0)]
        ));
    }

    assert_eq!(
        router.connector_possible_dst_pin_points("edge".to_string()),
        "[{\"x\":180,\"y\":30}]"
    );
    assert_eq!(
        router.connector_possible_dst_pin_points("missing".to_string()),
        "null"
    );
    router.add_connector("point-edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert_eq!(
        router.connector_possible_dst_pin_points("point-edge".to_string()),
        "[]"
    );
    assert!(router.move_shape_by("right".to_string(), 10.0, 5.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.connector_possible_dst_pin_points("edge".to_string()),
        "[{\"x\":190,\"y\":35}]"
    );
}

#[test]
fn moving_shape_moves_attached_pin_endpoint() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 0.0, 0.0, 40.0, 40.0);
    assert!(router.add_shape_connection_pin("box".to_string(), "east".to_string(), 40.0, 20.0));
    assert!(router.add_connector_between_pins(
        "loop".to_string(),
        "box".to_string(),
        "east".to_string(),
        "box".to_string(),
        "east".to_string()
    ));
    assert!(router.move_shape("box".to_string(), 100.0, 50.0, 140.0, 90.0));
    assert!(router.process_transaction());

    let route = router.route(&router.connectors[0]);
    assert_eq!(
        route,
        vec![Point { x: 140.0, y: 70.0 }, Point { x: 140.0, y: 70.0 }]
    );
}

#[test]
fn shape_connection_pin_position_directions_cost_and_exclusivity_are_exposed() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 10.0, 20.0, 50.0, 80.0);
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "out".to_string(),
        1,
        ATTACH_POS_RIGHT,
        ATTACH_POS_CENTRE,
        true,
        2.0,
        CONN_DIR_RIGHT,
        7.5,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "auto-corner".to_string(),
        2,
        ATTACH_POS_LEFT,
        ATTACH_POS_TOP,
        true,
        2.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "absolute-offset".to_string(),
        3,
        0.0,
        10.0,
        false,
        0.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "absolute-max".to_string(),
        4,
        ATTACH_POS_MAX_OFFSET,
        ATTACH_POS_MAX_OFFSET,
        false,
        2.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));

    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "out".to_string()),
        "{\"x\":48,\"y\":50}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "out".to_string()),
        CONN_DIR_RIGHT
    );
    assert_eq!(
        router.shape_connection_pin_ids("box".to_string(), "out".to_string()),
        "{\"objectId\":\"box\",\"classId\":1}"
    );
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "auto-corner".to_string()),
        "{\"x\":12,\"y\":22}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "auto-corner".to_string()),
        CONN_DIR_LEFT | CONN_DIR_UP
    );
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "absolute-offset".to_string()),
        "{\"x\":10,\"y\":30}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "absolute-offset".to_string()),
        CONN_DIR_LEFT
    );
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "absolute-max".to_string()),
        "{\"x\":48,\"y\":78}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "absolute-max".to_string()),
        CONN_DIR_ALL
    );
    assert_eq!(
        router.shape_connection_pin_connection_cost("box".to_string(), "out".to_string()),
        7.5
    );
    assert!(router.shape_connection_pin_exclusive("box".to_string(), "out".to_string()));

    assert!(router.set_shape_connection_pin_connection_cost(
        "box".to_string(),
        "out".to_string(),
        0.25
    ));
    assert_eq!(
        router.shape_connection_pin_connection_cost("box".to_string(), "out".to_string()),
        0.25
    );
    assert!(router.set_shape_connection_pin_exclusive("box".to_string(), "out".to_string(), false));
    assert!(!router.shape_connection_pin_exclusive("box".to_string(), "out".to_string()));

    assert!(!router.set_shape_connection_pin_connection_cost(
        "box".to_string(),
        "out".to_string(),
        -1.0
    ));
    assert!(!router.set_shape_connection_pin_exclusive(
        "missing".to_string(),
        "out".to_string(),
        true
    ));
    assert!(router.add_shape_connection_pin("box".to_string(), "specific".to_string(), 30.0, 40.0));
    assert_eq!(
        router.shape_connection_pin_ids("box".to_string(), "specific".to_string()),
        format!(
            "{{\"objectId\":\"box\",\"classId\":{}}}",
            CONNECTION_PIN_UNSET
        )
    );
    assert!(!router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "out".to_string(),
        0,
        ATTACH_POS_RIGHT,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_RIGHT,
        0.0,
        false,
    ));
    assert_eq!(
        router.shape_connection_pin_ids("box".to_string(), "out".to_string()),
        "{\"objectId\":\"box\",\"classId\":1}"
    );
    assert!(!router.add_shape_connection_pin(
        "box".to_string(),
        "nan-point".to_string(),
        f64::NAN,
        20.0
    ));
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "nan-point".to_string()),
        "null"
    );
    assert!(!router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "out".to_string(),
        1,
        f64::NAN,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_RIGHT,
        0.0,
        false,
    ));
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "out".to_string()),
        "{\"x\":48,\"y\":50}"
    );
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "missing".to_string()),
        "null"
    );
    assert_eq!(
        router.shape_connection_pin_ids("box".to_string(), "missing".to_string()),
        "null"
    );
    assert!(router
        .shape_connection_pin_connection_cost("box".to_string(), "missing".to_string())
        .is_nan());
}

#[test]
fn shape_connection_pin_mutation_invalidates_attached_connector_transactionally() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "out".to_string(),
        1,
        40.0,
        20.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "in".to_string(),
        2,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "spare-out".to_string(),
        3,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_ALL,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "spare-in".to_string(),
        4,
        0.0,
        30.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        1.0,
        true,
    ));
    assert!(router.add_connector_between_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router.add_connector_between_pin_classes(
        "spare-edge".to_string(),
        "left".to_string(),
        3,
        "right".to_string(),
        4,
    ));
    assert!(router.process_transaction());
    router.display_connector_route("edge".to_string());
    router.display_connector_route("spare-edge".to_string());
    assert!(!router.connector_needs_repaint("edge".to_string()));
    assert!(!router.connector_needs_repaint("spare-edge".to_string()));

    assert!(router.set_shape_connection_pin_connection_cost(
        "left".to_string(),
        "out".to_string(),
        2.0,
    ));
    assert!(router.set_shape_connection_pin_connection_cost(
        "left".to_string(),
        "out".to_string(),
        3.0,
    ));
    assert!(!router.connector_needs_repaint("edge".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.last_transaction_updated_connector_ids(),
        "[\"edge\"]"
    );
    assert!(router.connector_needs_repaint("edge".to_string()));
    assert!(!router.connector_needs_repaint("spare-edge".to_string()));
    router.display_connector_route("edge".to_string());

    assert!(router.set_shape_connection_pin_exclusive(
        "left".to_string(),
        "out".to_string(),
        false,
    ));
    assert!(router.process_transaction());
    assert_eq!(
        router.last_transaction_updated_connector_ids(),
        "[\"edge\"]"
    );
    assert!(router.connector_needs_repaint("edge".to_string()));
}

#[test]
fn shape_connection_pin_positions_can_be_transformed_like_upstream_shape_ref() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 10.0, 20.0, 50.0, 80.0);
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "right".to_string(),
        1,
        ATTACH_POS_RIGHT,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_connector_between_pins(
        "edge".to_string(),
        "box".to_string(),
        "right".to_string(),
        "box".to_string(),
        "right".to_string(),
    ));

    assert!(router
        .transform_shape_connection_pin_positions("box".to_string(), TRANSFORMATION_TYPE_CW90));
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "right".to_string()),
        "{\"x\":30,\"y\":80}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "right".to_string()),
        CONN_DIR_DOWN
    );
    assert!(router.process_transaction());
    assert_eq!(
        router.route_connector("edge".to_string()),
        "[{\"x\":30,\"y\":80},{\"x\":30,\"y\":80}]"
    );
    assert!(!router.transform_shape_connection_pin_positions("box".to_string(), 99));
    assert!(!router
        .transform_shape_connection_pin_positions("missing".to_string(), TRANSFORMATION_TYPE_CW90));
}

#[test]
fn absolute_shape_connection_pin_transforms_use_shape_dimensions() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 10.0, 20.0, 110.0, 70.0);
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "left".to_string(),
        1,
        0.0,
        10.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));

    assert!(router
        .transform_shape_connection_pin_positions("box".to_string(), TRANSFORMATION_TYPE_CW180));
    assert_eq!(
        router.shape_connection_pin_position("box".to_string(), "left".to_string()),
        "{\"x\":110,\"y\":60}"
    );
    assert_eq!(
        router.shape_connection_pin_directions("box".to_string(), "left".to_string()),
        CONN_DIR_RIGHT
    );
}

#[test]
fn removing_pin_removes_dependent_connectors() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 0.0, 0.0, 40.0, 40.0);
    assert!(router.add_shape_connection_pin("box".to_string(), "a".to_string(), 0.0, 20.0));
    assert!(router.add_shape_connection_pin("box".to_string(), "b".to_string(), 40.0, 20.0));
    assert!(router.add_connector_between_pins(
        "edge".to_string(),
        "box".to_string(),
        "a".to_string(),
        "box".to_string(),
        "b".to_string()
    ));

    assert!(router.remove_shape_connection_pin("box".to_string(), "a".to_string()));
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_eq!(router.route_connector("edge".to_string()), "null");

    router.set_transaction_use(false);
    assert!(router.add_shape_connection_pin("box".to_string(), "a".to_string(), 0.0, 20.0));
    assert!(router.add_connector_between_pins(
        "immediate".to_string(),
        "box".to_string(),
        "a".to_string(),
        "box".to_string(),
        "b".to_string()
    ));
    assert!(router.remove_shape_connection_pin("box".to_string(), "a".to_string()));
    assert_eq!(router.route_connector("immediate".to_string()), "null");
}

#[test]
fn removing_active_class_pin_reassigns_connector_to_available_shape_pin() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "first".to_string(),
        1,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "fallback".to_string(),
        1,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        5.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "in".to_string(),
        2,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(router.add_connector_between_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"first\""));

    assert!(router.remove_shape_connection_pin("left".to_string(), "first".to_string()));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"first\""));
    assert!(router.process_transaction());
    let ends = router.connector_endpoint_conn_ends("edge".to_string());
    assert!(ends.contains("\"pinId\":\"fallback\""));
    assert!(ends.contains("\"classId\":1"));
    assert_ne!(router.route_connector("edge".to_string()), "null");
}

#[test]
fn removing_active_class_pin_reassigns_connector_to_available_junction_pin() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "first".to_string(),
        1,
        CONN_DIR_RIGHT,
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "fallback".to_string(),
        1,
        CONN_DIR_RIGHT,
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "in".to_string(),
        2,
        CONN_DIR_LEFT,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "fallback".to_string(),
        5.0,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        1,
        "right".to_string(),
        2,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"first\""));

    assert!(router.remove_junction_connection_pin("left".to_string(), "first".to_string()));
    assert!(router.process_transaction());
    let ends = router.connector_endpoint_conn_ends("edge".to_string());
    assert!(ends.contains("\"pinId\":\"fallback\""));
    assert!(ends.contains("\"classId\":1"));
    assert_ne!(router.route_connector("edge".to_string()), "null");
}

#[test]
fn queued_pin_removal_drops_pending_endpoint_updates_for_that_pin() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 0.0, 0.0, 40.0, 40.0);
    assert!(router.add_shape_connection_pin("box".to_string(), "a".to_string(), 0.0, 20.0));
    assert!(router.add_shape_connection_pin("box".to_string(), "b".to_string(), 40.0, 20.0));
    router.add_connector("edge".to_string(), 100.0, 100.0, 120.0, 100.0);
    assert!(router.set_connector_source_pin_endpoint(
        "edge".to_string(),
        "box".to_string(),
        "a".to_string()
    ));
    assert!(router.remove_shape_connection_pin("box".to_string(), "a".to_string()));
    assert!(router.process_transaction());
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 100.0, y: 100.0 }));

    router.add_junction("j".to_string(), 20.0, 20.0);
    assert!(router.add_junction_connection_pin("j".to_string(), "p".to_string(), 1, CONN_DIR_ALL));
    assert!(router.set_connector_dest_junction_pin_endpoint(
        "edge".to_string(),
        "j".to_string(),
        "p".to_string()
    ));
    assert!(router.remove_junction_connection_pin("j".to_string(), "p".to_string()));
    assert!(router.process_transaction());
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.last(), Some(&Point { x: 120.0, y: 100.0 }));
}

#[test]
fn proportional_pin_tracks_shape_resize() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 0.0, 0.0, 100.0, 50.0);
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "centre".to_string(),
        1,
        ATTACH_POS_CENTRE,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));
    assert!(router.add_connector_between_pins(
        "self".to_string(),
        "box".to_string(),
        "centre".to_string(),
        "box".to_string(),
        "centre".to_string()
    ));
    assert!(router.move_shape("box".to_string(), 10.0, 20.0, 210.0, 120.0));
    assert!(router.process_transaction());

    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 110.0, y: 70.0 }, Point { x: 110.0, y: 70.0 }]
    );
}

#[test]
fn class_based_pin_selection_uses_cost_and_exclusivity() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "expensive".to_string(),
        10,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        50.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "cheap".to_string(),
        10,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target-a".to_string(),
        20,
        0.0,
        10.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target-b".to_string(),
        20,
        0.0,
        30.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));

    assert!(router.add_connector_between_pin_classes(
        "first".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "cheap"
    ));
    assert!(matches!(
        router.connectors[0].dst,
        Endpoint::ShapePin {
            pin_class_id: Some(20),
            ..
        }
    ));
    assert_eq!(
        router.connector_possible_dst_pin_points("first".to_string()),
        "[{\"x\":100,\"y\":30}]"
    );
    assert_eq!(
        router.connector_endpoint_conn_ends("first".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"left\",\"pinId\":\"cheap\",",
            "\"classId\":10,\"position\":{\"x\":40,\"y\":30},\"directions\":8},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"pinId\":\"target-a\",",
            "\"classId\":20,\"position\":{\"x\":100,\"y\":10},\"directions\":4}}"
        )
    );
    assert!(!router.add_connector_between_pin_classes(
        "invalid-zero".to_string(),
        "left".to_string(),
        0,
        "right".to_string(),
        20,
    ));
    assert!(!router.add_connector_between_pin_classes(
        "invalid-unset".to_string(),
        "left".to_string(),
        CONNECTION_PIN_UNSET,
        "right".to_string(),
        20,
    ));
    assert!(!router.set_connector_source_pin_class_endpoint(
        "first".to_string(),
        "left".to_string(),
        0,
    ));
    assert!(!router.set_connector_dest_pin_class_endpoint(
        "first".to_string(),
        "right".to_string(),
        CONNECTION_PIN_UNSET,
    ));
    assert!(router
        .connector_endpoint_conn_ends("first".to_string())
        .contains("\"pinId\":\"cheap\""));

    assert!(router.add_connector_between_pin_classes(
        "second".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(matches!(
        router.connectors[1].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "expensive"
    ));
}

#[test]
fn port_direction_penalty_affects_shape_pin_class_selection() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(PORT_DIRECTION_PENALTY, 0.0));
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "west".to_string(),
        10,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "east".to_string(),
        10,
        40.0,
        20.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        50.0,
        true,
    ));
    router.add_connector("edge".to_string(), 0.0, 20.0, 100.0, 20.0);

    assert!(router.set_connector_source_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        10,
    ));
    assert!(router.process_transaction());
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "west"
    ));

    assert!(router.set_routing_parameter(PORT_DIRECTION_PENALTY, 100.0));
    assert!(router.set_connector_source_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        10,
    ));
    assert!(router.process_transaction());
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "east"
    ));
}

#[test]
fn port_direction_penalty_uses_upstream_rotational_angle_ranges() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.set_routing_parameter(PORT_DIRECTION_PENALTY, 100.0));
    router.add_shape("box".to_string(), 0.0, 0.0, 40.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "up".to_string(),
        10,
        20.0,
        20.0,
        false,
        0.0,
        CONN_DIR_UP,
        0.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "box".to_string(),
        "down".to_string(),
        10,
        20.0,
        20.0,
        false,
        0.0,
        CONN_DIR_DOWN,
        50.0,
        true,
    ));
    router.add_connector("edge".to_string(), 20.0, 20.0, 20.0, 120.0);

    assert!(router.set_connector_source_pin_class_endpoint(
        "edge".to_string(),
        "box".to_string(),
        10,
    ));
    assert!(router.process_transaction());
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "down"
    ));
}

#[test]
fn pin_cost_and_exclusivity_mutation_affects_class_selection() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "a".to_string(),
        10,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        10.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "b".to_string(),
        10,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target".to_string(),
        20,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target-b".to_string(),
        20,
        0.0,
        30.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        1.0,
        true,
    ));

    assert!(router.set_shape_connection_pin_connection_cost(
        "left".to_string(),
        "a".to_string(),
        0.5
    ));
    assert!(router.set_shape_connection_pin_exclusive("left".to_string(), "a".to_string(), false));
    assert!(router.set_shape_connection_pin_exclusive(
        "right".to_string(),
        "target".to_string(),
        false
    ));
    assert!(router.add_connector_between_pin_classes(
        "first".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router.add_connector_between_pin_classes(
        "second".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "a"
    ));
    assert!(matches!(
        router.connectors[1].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "a"
    ));
}

#[test]
fn class_shape_pin_endpoint_reselects_after_pin_cost_changes() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "a".to_string(),
        10,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "b".to_string(),
        10,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        5.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target".to_string(),
        20,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target-b".to_string(),
        20,
        0.0,
        30.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        1.0,
        true,
    ));
    assert!(router.add_connector_between_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"a\""));

    assert!(router.set_shape_connection_pin_connection_cost(
        "left".to_string(),
        "b".to_string(),
        0.25,
    ));
    assert!(router.process_transaction());
    let ends = router.connector_endpoint_conn_ends("edge".to_string());
    assert!(ends.contains("\"pinId\":\"b\""));
    assert!(ends.contains("\"classId\":10"));
}

#[test]
fn adding_shape_pin_reselects_existing_class_endpoint() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "expensive".to_string(),
        10,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        5.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target".to_string(),
        20,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        false,
    ));
    assert!(router.add_connector_between_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"expensive\""));

    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "cheap".to_string(),
        10,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    let ends = router.connector_endpoint_conn_ends("edge".to_string());
    assert!(ends.contains("\"pinId\":\"cheap\""));
    assert!(ends.contains("\"classId\":10"));
    assert!(router.connectors[0].needs_reroute);
}

#[test]
fn missing_pin_class_connends_fall_back_to_anchor_positions_until_pins_exist() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_connector_between_pin_classes(
        "shape-edge".to_string(),
        "left".to_string(),
        77,
        "right".to_string(),
        88,
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("shape-edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"left\",\"classId\":77,",
            "\"position\":{\"x\":20,\"y\":20},\"directions\":15},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"classId\":88,",
            "\"position\":{\"x\":120,\"y\":20},\"directions\":15}}"
        )
    );
    assert_eq!(
        router.connector_possible_dst_pin_points("shape-edge".to_string()),
        "[]"
    );
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 20.0, y: 20.0 }, Point { x: 120.0, y: 20.0 }]
    );

    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "late-right".to_string(),
        88,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(matches!(
        router.connectors[0].dst,
        Endpoint::ShapePin {
            ref pin_id,
            pin_class_id: Some(88),
            ..
        } if pin_id == "late-right"
    ));

    router.add_junction("j-left".to_string(), 0.0, 100.0);
    router.add_junction("j-right".to_string(), 100.0, 100.0);
    assert!(router.add_connector_between_junction_pin_classes(
        "junction-edge".to_string(),
        "j-left".to_string(),
        7,
        "j-right".to_string(),
        8,
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("junction-edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"junctionPin\",\"junctionId\":\"j-left\",\"classId\":7,",
            "\"position\":{\"x\":0,\"y\":100},\"directions\":15},",
            "\"destination\":{\"type\":\"junctionPin\",\"junctionId\":\"j-right\",\"classId\":8,",
            "\"position\":{\"x\":100,\"y\":100},\"directions\":15}}"
        )
    );
    assert!(router.add_junction_connection_pin(
        "j-right".to_string(),
        "late-junction".to_string(),
        8,
        CONN_DIR_LEFT,
    ));
    assert!(matches!(
        router.connectors[1].dst,
        Endpoint::JunctionPin {
            ref pin_id,
            pin_class_id: Some(8),
            ..
        } if pin_id == "late-junction"
    ));
}

#[test]
fn class_junction_pin_endpoint_reselects_after_pin_cost_changes() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "a".to_string(),
        10,
        CONN_DIR_RIGHT,
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "b".to_string(),
        10,
        CONN_DIR_RIGHT,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "a".to_string(),
        1.0,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "b".to_string(),
        5.0,
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "target".to_string(),
        20,
        CONN_DIR_LEFT,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "edge".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"pinId\":\"a\""));

    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "b".to_string(),
        0.25,
    ));
    assert!(router.process_transaction());
    let ends = router.connector_endpoint_conn_ends("edge".to_string());
    assert!(ends.contains("\"pinId\":\"b\""));
    assert!(ends.contains("\"classId\":10"));
}

#[test]
fn class_shape_pin_endpoint_reselects_when_shared_pin_becomes_exclusive() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "shared".to_string(),
        10,
        40.0,
        10.0,
        false,
        0.0,
        CONN_DIR_ALL,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "fallback".to_string(),
        10,
        40.0,
        30.0,
        false,
        0.0,
        CONN_DIR_ALL,
        5.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target".to_string(),
        20,
        0.0,
        20.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target-b".to_string(),
        20,
        0.0,
        30.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        1.0,
        true,
    ));
    assert!(router.add_connector_between_pin_classes(
        "first".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router.add_connector_between_pin_classes(
        "second".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router
        .connector_endpoint_conn_ends("first".to_string())
        .contains("\"pinId\":\"shared\""));
    assert!(router
        .connector_endpoint_conn_ends("second".to_string())
        .contains("\"pinId\":\"shared\""));

    assert!(router.set_shape_connection_pin_exclusive(
        "left".to_string(),
        "shared".to_string(),
        true,
    ));
    assert!(router.process_transaction());
    let first = router.connector_endpoint_conn_ends("first".to_string());
    let second = router.connector_endpoint_conn_ends("second".to_string());
    assert!(
        (first.contains("\"pinId\":\"shared\"") && second.contains("\"pinId\":\"fallback\""))
            || (first.contains("\"pinId\":\"fallback\"")
                && second.contains("\"pinId\":\"shared\""))
    );
}

#[test]
fn class_junction_pin_endpoint_reselects_when_shared_pin_becomes_exclusive() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "shared".to_string(),
        10,
        CONN_DIR_RIGHT,
    ));
    assert!(router.set_junction_connection_pin_exclusive(
        "left".to_string(),
        "shared".to_string(),
        false,
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "fallback".to_string(),
        10,
        CONN_DIR_RIGHT,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "fallback".to_string(),
        5.0,
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "target".to_string(),
        20,
        CONN_DIR_LEFT,
    ));
    assert!(router.set_junction_connection_pin_exclusive(
        "right".to_string(),
        "target".to_string(),
        false,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "first".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router.add_connector_between_junction_pin_classes(
        "second".to_string(),
        "left".to_string(),
        10,
        "right".to_string(),
        20,
    ));
    assert!(router
        .connector_endpoint_conn_ends("first".to_string())
        .contains("\"pinId\":\"shared\""));
    assert!(router
        .connector_endpoint_conn_ends("second".to_string())
        .contains("\"pinId\":\"shared\""));

    assert!(router.set_junction_connection_pin_exclusive(
        "left".to_string(),
        "shared".to_string(),
        true,
    ));
    assert!(router.process_transaction());
    let first = router.connector_endpoint_conn_ends("first".to_string());
    let second = router.connector_endpoint_conn_ends("second".to_string());
    assert!(
        (first.contains("\"pinId\":\"shared\"") && second.contains("\"pinId\":\"fallback\""))
            || (first.contains("\"pinId\":\"fallback\"")
                && second.contains("\"pinId\":\"shared\""))
    );
}

#[test]
fn routing_parameters_follow_upstream_indices() {
    let mut router = AvoidRouter::new(0.0);
    assert_eq!(router.routing_parameter(SEGMENT_PENALTY), 10.0);
    assert_eq!(router.routing_parameter(ANGLE_PENALTY), 0.0);
    assert_eq!(router.routing_parameter(CROSSING_PENALTY), 0.0);
    assert_eq!(router.routing_parameter(CLUSTER_CROSSING_PENALTY), 4000.0);
    assert_eq!(router.routing_parameter(FIXED_SHARED_PATH_PENALTY), 0.0);
    assert_eq!(router.routing_parameter(PORT_DIRECTION_PENALTY), 0.0);
    assert_eq!(router.routing_parameter(IDEAL_NUDGING_DISTANCE), 4.0);
    assert!(router.set_routing_parameter(SHAPE_BUFFER_DISTANCE, 12.0));
    assert_eq!(router.routing_parameter(SHAPE_BUFFER_DISTANCE), 12.0);
    assert!(router.set_routing_parameter(SEGMENT_PENALTY, -1.0));
    assert_eq!(router.routing_parameter(SEGMENT_PENALTY), 50.0);
    assert!(router.set_routing_penalty(CROSSING_PENALTY, 200.0));
    assert_eq!(router.routing_parameter(CROSSING_PENALTY), 200.0);
    assert!(router.set_routing_penalty(PORT_DIRECTION_PENALTY, -1.0));
    assert_eq!(router.routing_parameter(PORT_DIRECTION_PENALTY), 100.0);
    assert!(!router.set_routing_penalty(99, 1.0));
    assert!(!router.set_routing_parameter(99, 1.0));
    assert!(router.process_transaction());
    assert!(!router.process_transaction());
}

#[test]
fn routing_options_follow_upstream_indices_and_defaults() {
    let mut router = AvoidRouter::new(0.0);

    assert!(!router.routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES));
    assert!(router.routing_option(IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS));
    assert!(!router.routing_option(PENALISE_ORTHOGONAL_SHARED_PATHS_AT_CONN_ENDS));
    assert!(!router.routing_option(NUDGE_ORTHOGONAL_TOUCHING_COLINEAR_SEGMENTS));
    assert!(router.routing_option(PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP));
    assert!(!router.routing_option(IMPROVE_HYPEREDGE_ROUTES_MOVING_ADDING_AND_DELETING_JUNCTIONS));
    assert!(router.routing_option(NUDGE_SHARED_PATHS_WITH_COMMON_END_POINT));

    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES, true));
    assert!(router.routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES));
    assert!(router.set_routing_option(PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP, false,));
    assert!(!router.routing_option(PERFORM_UNIFYING_NUDGING_PREPROCESSING_STEP));
    assert!(!router.set_routing_option(99, true));
    assert!(!router.routing_option(99));
    assert!(router.process_transaction());
    assert!(!router.process_transaction());
}

#[test]
fn valid_connector_type_follows_router_flags() {
    let orthogonal = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    assert_eq!(
        orthogonal.valid_conn_type(CONN_TYPE_ORTHOGONAL),
        CONN_TYPE_ORTHOGONAL
    );
    assert_eq!(
        orthogonal.valid_conn_type(CONN_TYPE_POLY_LINE),
        CONN_TYPE_ORTHOGONAL
    );
    assert_eq!(
        orthogonal.valid_conn_type(CONN_TYPE_NONE),
        CONN_TYPE_ORTHOGONAL
    );

    let polyline = AvoidRouter::with_flags(POLY_LINE_ROUTING);
    assert_eq!(
        polyline.valid_conn_type(CONN_TYPE_POLY_LINE),
        CONN_TYPE_POLY_LINE
    );
    assert_eq!(
        polyline.valid_conn_type(CONN_TYPE_ORTHOGONAL),
        CONN_TYPE_POLY_LINE
    );

    let both = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    assert_eq!(
        both.valid_conn_type(CONN_TYPE_ORTHOGONAL),
        CONN_TYPE_ORTHOGONAL
    );
    assert_eq!(both.valid_conn_type(CONN_TYPE_NONE), CONN_TYPE_POLY_LINE);

    let none = AvoidRouter::with_flags(0);
    assert_eq!(none.valid_conn_type(CONN_TYPE_ORTHOGONAL), CONN_TYPE_NONE);
}

#[test]
fn connector_routing_type_normalizes_and_hate_crossings_is_exposed() {
    let mut router = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);

    assert_eq!(
        router.connector_routing_type("edge".to_string()),
        CONN_TYPE_ORTHOGONAL
    );
    assert!(router.set_connector_routing_type("edge".to_string(), CONN_TYPE_POLY_LINE));
    assert_eq!(
        router.connector_routing_type("edge".to_string()),
        CONN_TYPE_ORTHOGONAL
    );
    assert!(router.set_connector_routing_type("edge".to_string(), CONN_TYPE_NONE));
    assert_eq!(
        router.connector_routing_type("edge".to_string()),
        CONN_TYPE_ORTHOGONAL
    );
    assert!(!router.set_connector_routing_type("edge".to_string(), 99));
    assert_eq!(
        router.connector_routing_type("missing".to_string()),
        CONN_TYPE_NONE
    );

    assert!(!router.connector_does_hate_crossings("edge".to_string()));
    assert!(router.set_connector_hate_crossings("edge".to_string(), true));
    assert!(router.connector_does_hate_crossings("edge".to_string()));
    assert!(router.set_connector_hate_crossings("edge".to_string(), false));
    assert!(!router.connector_does_hate_crossings("edge".to_string()));
    assert!(!router.set_connector_hate_crossings("missing".to_string(), true));
    assert!(!router.connector_does_hate_crossings("missing".to_string()));

    let mut typed = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    typed.add_connector("type-edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(typed.process_transaction());
    assert!(typed.connector_needs_repaint("type-edge".to_string()));
    typed.display_connector_route("type-edge".to_string());
    assert!(!typed.connector_needs_repaint("type-edge".to_string()));

    assert!(typed.set_connector_routing_type("type-edge".to_string(), CONN_TYPE_POLY_LINE));
    assert!(!typed.connector_needs_repaint("type-edge".to_string()));
    assert!(typed.process_transaction());
    assert!(typed.connector_needs_repaint("type-edge".to_string()));
    typed.display_connector_route("type-edge".to_string());

    assert!(typed.set_connector_routing_type("type-edge".to_string(), CONN_TYPE_POLY_LINE));
    assert!(!typed.process_transaction());

    typed.set_transaction_use(false);
    assert!(typed.set_connector_routing_type("type-edge".to_string(), CONN_TYPE_ORTHOGONAL));
    assert!(typed.connector_needs_repaint("type-edge".to_string()));
}

#[test]
fn object_id_shape_containment_position_and_attached_connector_queries_match_scene() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("peer".to_string(), 80.0, 0.0, 120.0, 40.0);
    router.add_junction("junction".to_string(), 100.0, 0.0);
    router.add_shape_connection_pin("box".to_string(), "pin".to_string(), 40.0, 20.0);
    router.add_shape_connection_pin("peer".to_string(), "pin".to_string(), 80.0, 20.0);
    assert!(router.add_connector_between_pins(
        "shape-edge".to_string(),
        "box".to_string(),
        "pin".to_string(),
        "box".to_string(),
        "pin".to_string(),
    ));
    assert!(router.add_connector_between_pins(
        "to-peer".to_string(),
        "box".to_string(),
        "pin".to_string(),
        "peer".to_string(),
        "pin".to_string(),
    ));
    assert!(router.add_connector_between_pins(
        "from-peer".to_string(),
        "peer".to_string(),
        "pin".to_string(),
        "box".to_string(),
        "pin".to_string(),
    ));
    assert!(router.add_connector_from_junction(
        "junction-edge".to_string(),
        "junction".to_string(),
        140.0,
        0.0,
    ));

    assert!(!router.object_id_is_unused_public("box".to_string()));
    assert!(!router.object_id_is_unused_public("junction".to_string()));
    assert!(!router.object_id_is_unused_public("shape-edge".to_string()));
    assert!(router.object_id_is_unused_public("unused".to_string()));
    assert_eq!(
        router.shape_position("box".to_string()),
        "{\"x\":20,\"y\":20}"
    );
    assert_eq!(router.shape_containing_point(10.0, 10.0), "\"box\"");
    assert_eq!(router.shape_containing_point(0.0, 10.0), "\"box\"");
    assert_eq!(router.shape_containing_point(50.0, 10.0), "null");
    assert_eq!(
        router.shape_attached_connectors("box".to_string()),
        "[\"shape-edge\",\"to-peer\",\"from-peer\"]"
    );
    assert_eq!(
        router.shape_attached_shapes("box".to_string(), RUNNING_TO_AND_FROM),
        "[\"box\",\"peer\"]"
    );
    assert_eq!(
        router.shape_attached_shapes("box".to_string(), RUNNING_TO),
        "[\"box\",\"peer\"]"
    );
    assert_eq!(
        router.shape_attached_shapes("box".to_string(), RUNNING_FROM),
        "[\"box\",\"peer\"]"
    );
    assert_eq!(
        router.junction_attached_connectors("junction".to_string()),
        "[\"junction-edge\"]"
    );
    assert_eq!(
        router.shape_attached_connectors("missing".to_string()),
        "null"
    );
    assert_eq!(
        router.shape_attached_shapes("missing".to_string(), RUNNING_TO),
        "null"
    );
    assert_eq!(
        router.junction_attached_connectors("missing".to_string()),
        "null"
    );
}

#[test]
fn transactions_defer_shape_and_junction_moves_until_processed() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.transaction_use());
    router.add_shape("box".to_string(), 0.0, 0.0, 10.0, 10.0);
    router.add_junction("j".to_string(), 20.0, 0.0);
    router.add_connector("edge".to_string(), 0.0, 5.0, 30.0, 5.0);

    assert!(router.move_shape_by("box".to_string(), 100.0, 0.0));
    assert!(router.move_junction("j".to_string(), 40.0, 0.0));
    assert_eq!(
        router.shape_bounding_box("box".to_string()),
        "{\"left\":0,\"top\":0,\"right\":10,\"bottom\":10}"
    );
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":20,\"y\":0}"
    );
    assert!(router.set_connector_source_endpoint("edge".to_string(), -20.0, 5.0));
    assert!(router.set_connector_dest_junction_endpoint("edge".to_string(), "j".to_string()));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":0,\"y\":5},",
            "\"position\":{\"x\":0,\"y\":5},\"directions\":15},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":30,\"y\":5},",
            "\"position\":{\"x\":30,\"y\":5},\"directions\":15}}"
        )
    );

    assert!(router.process_transaction());
    assert_eq!(
        router.shape_bounding_box("box".to_string()),
        "{\"left\":100,\"top\":0,\"right\":110,\"bottom\":10}"
    );
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":40,\"y\":0}"
    );
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":-20,\"y\":5},",
            "\"position\":{\"x\":-20,\"y\":5},\"directions\":15},",
            "\"destination\":{\"type\":\"junction\",\"junctionId\":\"j\",",
            "\"position\":{\"x\":40,\"y\":0},\"directions\":15}}"
        )
    );
    assert!(!router.process_transaction());

    assert!(router.move_shape_by("box".to_string(), 5.0, 0.0));
    router.set_transaction_use(false);
    assert!(!router.transaction_use());
    assert_eq!(
        router.shape_bounding_box("box".to_string()),
        "{\"left\":100,\"top\":0,\"right\":110,\"bottom\":10}"
    );
    assert!(router.process_transaction());
    assert_eq!(
        router.shape_bounding_box("box".to_string()),
        "{\"left\":105,\"top\":0,\"right\":115,\"bottom\":10}"
    );

    router.set_transaction_use(false);
    assert!(!router.transaction_use());
    assert!(router.move_shape_by("box".to_string(), -10.0, 0.0));
    assert_eq!(
        router.shape_bounding_box("box".to_string()),
        "{\"left\":95,\"top\":0,\"right\":105,\"bottom\":10}"
    );

    router.set_transaction_use(true);
    assert!(router.remove_shape("box".to_string()));
    assert_ne!(router.shape_bounding_box("box".to_string()), "null");
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_eq!(router.shape_bounding_box("box".to_string()), "null");
    assert_ne!(router.route_connector("edge".to_string()), "null");

    router.add_connector("delete-edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(router.remove_connector("delete-edge".to_string()));
    assert_ne!(router.route_connector("delete-edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_eq!(router.route_connector("delete-edge".to_string()), "null");

    router.set_transaction_use(false);
    router.add_junction("delete-j".to_string(), 0.0, 0.0);
    assert!(router.remove_junction("delete-j".to_string()));
    assert_eq!(router.junction_position("delete-j".to_string()), "null");
}

#[test]
fn polygon_shape_keeps_upstream_style_boundary_and_bounding_box() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.add_polygon_shape("triangle".to_string(), vec![1.0, 1.0, 4.0, 2.0, 2.0, 6.0]));

    assert_eq!(
        router.shape_bounding_box("triangle".to_string()),
        "{\"left\":1,\"top\":1,\"right\":4,\"bottom\":6}"
    );
    assert_eq!(
        router.shape_polygon("triangle".to_string()),
        "[{\"x\":1,\"y\":1},{\"x\":4,\"y\":2},{\"x\":2,\"y\":6}]"
    );

    assert!(router.move_shape_by("triangle".to_string(), 10.0, -1.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.shape_bounding_box("triangle".to_string()),
        "{\"left\":11,\"top\":0,\"right\":14,\"bottom\":5}"
    );
    assert_eq!(
        router.shape_polygon("triangle".to_string()),
        "[{\"x\":11,\"y\":0},{\"x\":14,\"y\":1},{\"x\":12,\"y\":5}]"
    );
}

#[test]
fn invalid_polygon_shapes_are_rejected() {
    let mut router = AvoidRouter::new(0.0);
    assert!(!router.add_polygon_shape("odd".to_string(), vec![0.0, 0.0, 1.0]));
    assert!(!router.add_polygon_shape("short".to_string(), vec![0.0, 0.0, 1.0, 1.0]));
    assert!(!router.add_polygon_shape("nan".to_string(), vec![0.0, 0.0, f64::NAN, 1.0, 1.0, 0.0]));
    assert_eq!(router.shape_polygon("odd".to_string()), "null");
}

#[test]
fn polygon_offset_bounding_box_matches_upstream_contract() {
    let polygon = Polygon::rectangle(10.0, 20.0, 30.0, 50.0);
    let bbox = polygon.offset_bounding_box(3.0).unwrap();

    assert_eq!(bbox.min, Point { x: 7.0, y: 17.0 });
    assert_eq!(bbox.max, Point { x: 33.0, y: 53.0 });
    assert_eq!(bbox.length(XDIM), 26.0);
    assert_eq!(bbox.length(YDIM), 36.0);
    assert_eq!(bbox.width(), 26.0);
    assert_eq!(bbox.height(), 36.0);
}

#[test]
fn polygon_offset_polygon_expands_edges_like_upstream_contract() {
    let polygon = Polygon::rectangle(10.0, 20.0, 30.0, 50.0);
    assert_eq!(
        polygon.points,
        vec![
            Point { x: 30.0, y: 20.0 },
            Point { x: 30.0, y: 50.0 },
            Point { x: 10.0, y: 50.0 },
            Point { x: 10.0, y: 20.0 },
        ]
    );
    assert_eq!(
        polygon.offset_polygon(3.0).unwrap().points,
        vec![
            Point { x: 33.0, y: 17.0 },
            Point { x: 33.0, y: 53.0 },
            Point { x: 7.0, y: 53.0 },
            Point { x: 7.0, y: 17.0 },
        ]
    );
    assert_eq!(polygon.offset_polygon(0.0).unwrap(), polygon);
}

#[test]
fn upstream_point_and_rectangle_helpers_match_geomtypes_contract() {
    let point = Point { x: 1.0, y: 2.0 };
    assert!(point.equals(
        Point {
            x: 1.00001,
            y: 2.00001
        },
        0.0001
    ));
    assert!(!point.equals(Point { x: 1.001, y: 2.0 }, 0.0001));
    assert_eq!(point.component(XDIM), 1.0);
    assert_eq!(point.component(YDIM), 2.0);
    assert_eq!(
        point.add(Point { x: 3.0, y: 4.0 }),
        Point { x: 4.0, y: 6.0 }
    );
    assert_eq!(
        point.sub(Point { x: 3.0, y: 4.0 }),
        Point { x: -2.0, y: -2.0 }
    );

    assert_eq!(
        Rect::from_center(Point { x: 20.0, y: 30.0 }, 10.0, 20.0)
            .to_polygon()
            .points,
        vec![
            Point { x: 25.0, y: 20.0 },
            Point { x: 25.0, y: 40.0 },
            Point { x: 15.0, y: 40.0 },
            Point { x: 15.0, y: 20.0 },
        ]
    );
}

#[test]
fn upstream_polygon_simplify_collapses_collinear_segments() {
    let polyline = Polygon::new(vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 4.0, y: 0.0 },
        Point { x: 8.0, y: 0.0 },
        Point { x: 8.0, y: 3.0 },
        Point { x: 8.0, y: 6.0 },
        Point { x: 10.0, y: 6.0 },
    ])
    .unwrap();

    assert_eq!(
        polyline.simplify().points,
        vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            Point { x: 8.0, y: 6.0 },
            Point { x: 10.0, y: 6.0 },
        ]
    );
}

#[test]
fn upstream_geometry_polygon_containment_matches_in_poly_contracts() {
    let convex = Polygon::rectangle(10.0, 20.0, 30.0, 50.0);
    assert!(in_poly(&convex, Point { x: 20.0, y: 30.0 }, true));
    assert!(in_poly(&convex, Point { x: 10.0, y: 30.0 }, true));
    assert!(!in_poly(&convex, Point { x: 10.0, y: 30.0 }, false));
    assert!(!in_poly(&convex, Point { x: 35.0, y: 30.0 }, true));

    // Upstream geometry.cpp exposes inPolyGen for non-convex polygons and
    // treats boundary points as inside.
    let concave = Polygon::new(vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 6.0, y: 0.0 },
        Point { x: 6.0, y: 6.0 },
        Point { x: 3.0, y: 3.0 },
        Point { x: 0.0, y: 6.0 },
    ])
    .unwrap();
    assert!(in_poly_gen(&concave, Point { x: 1.0, y: 1.0 }));
    assert!(in_poly_gen(&concave, Point { x: 3.0, y: 3.0 }));
    assert!(!in_poly_gen(&concave, Point { x: 3.0, y: 5.0 }));
    assert!(concave.contains_point(Point { x: 3.0, y: 3.0 }, true));
    assert!(!concave.contains_point(Point { x: 3.0, y: 3.0 }, false));
}

#[test]
fn upstream_geometry_segment_predicates_match_documented_cases() {
    assert!(colinear(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 4.0, y: 0.0 },
        0.0,
    ));
    assert!(point_on_line(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 4.0, y: 0.0 },
        0.0,
    ));
    assert!(segment_intersect(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
        Point { x: 0.0, y: 10.0 },
        Point { x: 10.0, y: 0.0 },
    ));
    assert!(!segment_intersect(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 20.0, y: 0.0 },
    ));
}

#[test]
fn upstream_geometry_segment_and_ray_intersection_points_match_source_contract() {
    let (result, point) = segment_intersect_point(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
        Point { x: 0.0, y: 10.0 },
        Point { x: 10.0, y: 0.0 },
    );
    assert_eq!(result, DO_INTERSECT);
    assert_eq!(point.unwrap(), Point { x: 5.0, y: 5.0 });

    let (result, point) = segment_intersect_point(
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
        Point { x: 2.0, y: 0.0 },
        Point { x: 3.0, y: 0.0 },
    );
    assert_eq!(result, DONT_INTERSECT);
    assert_eq!(point, None);

    let (result, point) = segment_intersect_point(
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 5.0, y: 0.0 },
        Point { x: 15.0, y: 0.0 },
    );
    assert_eq!(result, PARALLEL);
    assert_eq!(point, None);

    let (result, point) = ray_intersect_point(
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
        Point { x: 2.0, y: -1.0 },
        Point { x: 2.0, y: 1.0 },
    );
    assert_eq!(result, DO_INTERSECT);
    assert_eq!(point.unwrap(), Point { x: 2.0, y: 0.0 });
}

#[test]
fn upstream_geometry_distance_angle_and_projection_helpers_match_documented_cases() {
    let a = Point { x: 0.0, y: 0.0 };
    let b = Point { x: 3.0, y: 4.0 };
    assert_eq!(euclidean_distance(a, b), 5.0);
    assert_eq!(manhattan_distance(a, b), 7.0);

    let polyline = Polygon::new(vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 3.0, y: 4.0 },
        Point { x: 6.0, y: 4.0 },
    ])
    .unwrap();
    assert_eq!(total_length(&polyline), 8.0);

    assert!(
        (angle(
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
        ) - std::f64::consts::FRAC_PI_2)
            .abs()
            < EPS
    );

    assert_eq!(
        projection(
            Point { x: 0.0, y: 0.0 },
            Point { x: 3.0, y: 4.0 },
            Point { x: 10.0, y: 0.0 },
        ),
        Point { x: 3.0, y: 0.0 }
    );
}

#[test]
fn upstream_geometry_region_corner_and_rotation_helpers_match_documented_cases() {
    assert_eq!(rotational_angle(Point { x: 1.0, y: 0.0 }), 0.0);
    assert_eq!(rotational_angle(Point { x: 0.0, y: 1.0 }), 90.0);
    assert_eq!(rotational_angle(Point { x: -1.0, y: 0.0 }), 180.0);
    assert_eq!(rotational_angle(Point { x: 0.0, y: -1.0 }), 270.0);
    assert_eq!(rotational_angle(Point { x: -1.0, y: -1.0 }), 225.0);

    let c1 = Point { x: 0.0, y: 0.0 };
    let c2 = Point { x: 1.0, y: 0.0 };
    let c3 = Point { x: 1.0, y: 1.0 };
    assert_eq!(corner_side(c1, c2, c3, Point { x: 0.5, y: 0.5 }), 1);
    assert_eq!(corner_side(c1, c2, c3, Point { x: 2.0, y: 0.5 }), -1);

    assert!(in_valid_region(false, c1, c2, c3, Point { x: 2.0, y: 2.0 }));
    assert!(!in_valid_region(true, c1, c2, c3, Point { x: 0.5, y: 0.5 }));
}

#[test]
fn visibility_graph_records_orthogonal_edges_between_vertices() {
    let mut graph = VisibilityGraph::default();
    let source = graph.add_vertex(VertId::new(1, 0, PROP_CONN_POINT), Point { x: 0.0, y: 0.0 });
    let target = graph.add_vertex(
        VertId::new(2, 0, PROP_CONN_POINT),
        Point { x: 10.0, y: 0.0 },
    );
    graph.add_orthogonal_edge(source, target, Direction::Horizontal);

    let edge = graph.neighbours(source).next().unwrap().1;
    assert!(edge.orthogonal);
    assert_eq!(edge.other_vertex(source), Some(target));
    assert!(edge.is_between(source, target));
    assert!(edge.is_between(target, source));
    assert!(!edge.is_between(source, source));
    assert_eq!(
        graph.edge_points(edge.from, edge.to),
        Some((Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }))
    );
    assert_eq!(
        graph.vertices.get(source).unwrap().id.props,
        PROP_CONN_POINT
    );
    assert_eq!(
        graph.vertices.get(target).unwrap().id.props,
        PROP_CONN_POINT
    );
    assert_eq!(
        same_axis(Point { x: 5.0, y: 1.0 }, Point { x: 5.0, y: 9.0 }),
        Some(Direction::Vertical)
    );
    assert_eq!(
        VertId::new(3, 0, PROP_ORTH_SHAPE_EDGE).props,
        PROP_ORTH_SHAPE_EDGE
    );
}

#[test]
fn vert_id_matches_upstream_identity_and_property_flag_contract() {
    let endpoint = VertId::new(12, VERT_ID_SRC, PROP_CONN_POINT);
    let same_identity_with_more_props =
        VertId::new(12, VERT_ID_SRC, PROP_CONN_POINT | PROP_CONNECTION_PIN);
    let target = VertId::new(12, VERT_ID_TAR, PROP_CONN_POINT);
    let checkpoint = VertId::new(
        12,
        VERT_ID_SRC,
        PROP_CONN_POINT | PROP_CONN_CHECKPOINT | PROP_DUMMY_PIN_HELPER,
    );

    assert_eq!(endpoint, same_identity_with_more_props);
    assert_ne!(endpoint, target);
    assert!(endpoint < target);
    assert_eq!(endpoint.offset(1), target);
    assert!(endpoint.is_conn_point());
    assert!(!endpoint.is_orth_shape_edge());
    assert!(same_identity_with_more_props.is_connection_pin());
    assert!(checkpoint.is_conn_checkpoint());
    assert!(checkpoint.is_dummy_pin_helper());
    assert!(VertId::new(0, 0, PROP_ORTH_SHAPE_EDGE).is_orth_shape_edge());
}

#[test]
fn vert_inf_list_matches_upstream_lookup_and_category_contract() {
    let top_bit = 1_u32 << 31;
    let mut vertices = VertInfList::default();
    let shape_a = vertices.add_vertex(VertInf::new(
        VertId::new(20, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: 20.0 },
    ));
    let source = vertices.add_vertex(VertInf::new(
        VertId::new(10, VERT_ID_SRC, PROP_CONN_POINT),
        Point { x: 0.0, y: 0.0 },
    ));
    let target = vertices.add_vertex(VertInf::new(
        VertId::new(10, VERT_ID_TAR, PROP_CONN_POINT),
        Point { x: 100.0, y: 0.0 },
    ));
    let shape_b = vertices.add_vertex(VertInf::new(
        VertId::new(20, 1, PROP_ORTH_SHAPE_EDGE),
        Point { x: 30.0, y: 40.0 },
    ));

    assert_eq!(vertices.conns_size(), 2);
    assert_eq!(vertices.shapes_size(), 2);
    assert_eq!(
        vertices
            .conns_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        vec![source, target]
    );
    assert_eq!(
        vertices
            .shapes_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        vec![shape_a, shape_b]
    );
    assert_eq!(
        vertices
            .get_vertex_by_id(VertId::new(
                top_bit | 10,
                VERT_ID_UNASSIGNED,
                PROP_CONN_POINT
            ))
            .map(|(index, _)| index),
        Some(source)
    );
    assert_eq!(
        vertices
            .get_vertex_by_id(VertId::new(10, VERT_ID_UNASSIGNED, PROP_CONN_POINT))
            .map(|(index, _)| index),
        Some(target)
    );
    assert_eq!(
        vertices
            .get_shape_vertex_by_pos(Point { x: 30.0, y: 40.0 })
            .map(|(index, _)| index),
        Some(shape_b)
    );
    assert!(vertices
        .get_shape_vertex_by_pos(Point { x: 0.0, y: 0.0 })
        .is_none());
}

#[test]
fn visibility_graph_hides_restores_removes_and_tracks_contains_maps() {
    let mut graph = VisibilityGraph::default();
    let source = graph.add_vertex(VertId::new(1, 0, PROP_CONN_POINT), Point { x: 0.0, y: 0.0 });
    let target = graph.add_vertex(VertId::new(2, 0, PROP_CONN_POINT), Point { x: 3.0, y: 4.0 });

    graph.add_visibility_edge(source, target);
    assert!(graph.existing_edge(source, target).is_some());
    assert!(graph.existing_edge(target, source).is_some());
    assert_eq!(
        graph.edge_ids(source, target),
        Some((
            VertId::new(1, 0, PROP_CONN_POINT),
            VertId::new(2, 0, PROP_CONN_POINT)
        ))
    );
    assert_eq!(graph.edge_is_dummy_connection(source, target), Some(false));
    assert_eq!(graph.visible_edges().count(), 1);
    assert_eq!(graph.invisible_edges().count(), 0);
    assert_eq!(graph.neighbours(source).count(), 1);
    let edge = graph.edge_between(source, target).unwrap();
    assert!(edge.added());
    assert!(!edge.is_orthogonal());
    assert!(!edge.is_disabled());
    assert_eq!(edge.blocker(), 0);
    assert_eq!(edge.get_dist(), 5.0);

    assert!(graph.hide_edge(source, target, 42));
    let hidden = graph.edge_between(source, target).unwrap();
    assert!(!hidden.visible);
    assert_eq!(hidden.blocker, Some(42));
    assert_eq!(hidden.blocker(), 42);
    assert_eq!(hidden.dist, 0.0);
    assert_eq!(graph.visible_edges().count(), 0);
    assert_eq!(graph.invisible_edges().count(), 1);
    assert_eq!(graph.neighbours(source).count(), 0);

    assert!(graph.set_edge_dist(source, target, 12.0));
    let distance_set = graph.edge_between(source, target).unwrap();
    assert!(distance_set.visible);
    assert_eq!(distance_set.blocker(), 0);
    assert_eq!(distance_set.get_dist(), 12.0);

    assert!(graph.add_cycle_blocker(source, target));
    let cycle_blocked = graph.edge_between(source, target).unwrap();
    assert!(!cycle_blocked.visible);
    assert_eq!(cycle_blocked.blocker(), -1);

    assert!(graph.restore_edge(source, target));
    let restored = graph.edge_between(source, target).unwrap();
    assert!(restored.visible);
    assert_eq!(restored.blocker, None);
    assert_eq!(restored.blocker(), 0);
    assert_eq!(restored.dist, 5.0);
    assert_eq!(graph.neighbours(source).count(), 1);

    assert!(graph.set_edge_disabled(source, target, true));
    assert!(graph.edge_between(source, target).unwrap().is_disabled());
    assert_eq!(graph.visible_edges().count(), 0);
    assert_eq!(graph.neighbours(source).count(), 0);
    assert!(graph.set_edge_disabled(source, target, false));
    assert!(!graph.edge_between(source, target).unwrap().is_disabled());
    assert_eq!(graph.visible_edges().count(), 1);

    assert!(graph.set_contains(source, 99, true));
    assert!(graph.vertex_contains_shape(source, 99));
    assert_eq!(graph.contained_shapes(source), vec![99]);
    assert!(graph.set_contains(source, 99, false));
    assert!(!graph.vertex_contains_shape(source, 99));
    assert!(graph.contained_shapes(source).is_empty());

    assert!(graph.remove_edge(source, target));
    assert!(graph.edge_between(source, target).is_none());
    assert_eq!(graph.visible_edges().count(), 0);
}

#[test]
fn edge_dummy_connection_detection_matches_upstream_pin_helper_rule() {
    let mut graph = VisibilityGraph::default();
    let pin = graph.add_vertex(
        VertId::new(1, 3, PROP_CONN_POINT | PROP_CONNECTION_PIN),
        Point { x: 10.0, y: 0.0 },
    );
    let centre = graph.add_vertex(
        VertId::new(1, VERT_ID_SRC, PROP_CONN_POINT),
        Point { x: 0.0, y: 0.0 },
    );
    let shape = graph.add_vertex(
        VertId::new(1, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 20.0, y: 0.0 },
    );

    graph.add_visibility_edge(pin, centre);
    graph.add_visibility_edge(pin, shape);

    assert_eq!(graph.edge_is_dummy_connection(pin, centre), Some(true));
    assert_eq!(graph.edge_is_dummy_connection(centre, pin), Some(true));
    assert_eq!(graph.edge_is_dummy_connection(pin, shape), Some(false));
}

#[test]
fn vertinf_visible_directions_disable_incident_edges_like_upstream() {
    let mut graph = VisibilityGraph::default();
    let center = graph.add_vertex(
        VertId::new(1, VERT_ID_SRC, PROP_CONN_POINT),
        Point { x: 0.0, y: 0.0 },
    );
    let right = graph.add_vertex(
        VertId::new(2, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: 0.0 },
    );
    let left = graph.add_vertex(
        VertId::new(3, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: -10.0, y: 0.0 },
    );
    let up = graph.add_vertex(
        VertId::new(4, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 0.0, y: 10.0 },
    );
    let diagonal = graph.add_vertex(
        VertId::new(5, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: 10.0 },
    );

    for other in [right, left, up, diagonal] {
        graph.add_visibility_edge(center, other);
    }

    assert_eq!(
        graph
            .vertices
            .get(right)
            .unwrap()
            .direction_from(graph.vertices.get(center).unwrap()),
        CONN_DIR_RIGHT
    );
    assert_eq!(
        graph
            .vertices
            .get(diagonal)
            .unwrap()
            .direction_from(graph.vertices.get(center).unwrap()),
        CONN_DIR_UP | CONN_DIR_RIGHT
    );

    assert!(graph.set_vertex_visible_directions(center, CONN_DIR_RIGHT));
    assert_eq!(
        graph.vertices.get(center).unwrap().vis_directions,
        CONN_DIR_RIGHT
    );
    assert!(!graph.edge_between(center, right).unwrap().is_disabled());
    assert!(graph.edge_between(center, left).unwrap().is_disabled());
    assert!(graph.edge_between(center, up).unwrap().is_disabled());
    assert!(!graph.edge_between(center, diagonal).unwrap().is_disabled());
    assert_eq!(graph.neighbours(center).count(), 2);

    assert!(graph.set_vertex_visible_directions(center, CONN_DIR_ALL));
    assert_eq!(graph.neighbours(center).count(), 4);
    assert!(graph.visible_edges().all(|edge| !edge.is_disabled()));
}

#[test]
fn static_visibility_graph_records_visible_invisible_and_containment_state() {
    let shape = Polygon::rectangle(40.0, 40.0, 80.0, 80.0);
    let graph = build_static_visibility_graph(
        &[shape],
        &[
            Point { x: 0.0, y: 60.0 },
            Point { x: 120.0, y: 60.0 },
            Point { x: 60.0, y: 60.0 },
        ],
    );

    let crossing = graph.edge_between(0, 1).unwrap();
    assert!(!crossing.visible);
    assert_eq!(crossing.blocker, Some(1));
    assert_eq!(
        graph.neighbours(0).filter(|(other, _)| *other == 1).count(),
        0
    );

    assert!(graph.vertex_contains_shape(2, 1));

    let top_boundary = graph.edge_between(3, 4).unwrap();
    assert!(top_boundary.visible);
    assert_eq!(top_boundary.blocker, None);

    let diagonal = graph.edge_between(3, 5).unwrap();
    assert!(!diagonal.visible);
    assert_eq!(diagonal.blocker, Some(1));
}

#[test]
fn static_visibility_graph_cache_invalidates_and_regenerates_for_shape_lifecycle() {
    let mut cache = StaticVisibilityGraph::new(
        vec![Polygon::rectangle(40.0, 40.0, 80.0, 80.0)],
        vec![Point { x: 0.0, y: 60.0 }, Point { x: 120.0, y: 60.0 }],
    );

    assert!(cache.is_dirty());
    assert!(!cache.graph().edge_between(0, 1).unwrap().visible);
    assert!(!cache.is_dirty());

    assert!(cache.set_shape(0, Polygon::rectangle(40.0, 80.0, 80.0, 120.0)));
    assert!(cache.is_dirty());
    assert!(cache.graph().edge_between(0, 1).unwrap().visible);
    assert!(!cache.is_dirty());

    cache.add_shape(Polygon::rectangle(50.0, 50.0, 70.0, 70.0));
    assert!(cache.is_dirty());
    assert!(!cache.graph().edge_between(0, 1).unwrap().visible);

    assert!(cache.remove_shape(0));
    assert!(cache.remove_shape(0));
    assert!(cache.graph().edge_between(0, 1).unwrap().visible);
}

#[test]
fn router_visibility_graph_cache_tracks_shape_lifecycle_transactions() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);

    assert!(router.visibility_graph_dirty_for_tests());
    assert!(
        router
            .visibility_graph_for_tests()
            .edge_between(0, 1)
            .unwrap()
            .visible
    );
    assert!(!router.visibility_graph_dirty_for_tests());

    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    assert!(router.visibility_graph_dirty_for_tests());
    assert!(
        !router
            .visibility_graph_for_tests()
            .edge_between(0, 1)
            .unwrap()
            .visible
    );

    assert!(router.move_shape("box".to_string(), 40.0, 80.0, 80.0, 120.0));
    assert!(!router.visibility_graph_dirty_for_tests());
    assert!(router.process_transaction());
    assert!(router.visibility_graph_dirty_for_tests());
    assert!(
        router
            .visibility_graph_for_tests()
            .edge_between(0, 1)
            .unwrap()
            .visible
    );

    assert!(router.remove_shape("box".to_string()));
    assert!(!router.visibility_graph_dirty_for_tests());
    assert!(router.process_transaction());
    assert!(router.visibility_graph_dirty_for_tests());
    assert!(
        router
            .visibility_graph_for_tests()
            .edge_between(0, 1)
            .unwrap()
            .visible
    );
}

#[test]
fn router_visibility_graph_cache_tracks_connector_endpoint_lifecycle() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);
    assert!(
        !router
            .visibility_graph_for_tests()
            .edge_between(0, 1)
            .unwrap()
            .visible
    );

    assert!(router.set_connector_dest_endpoint("edge".to_string(), 20.0, 60.0));
    assert!(!router.visibility_graph_dirty_for_tests());
    assert!(router.process_transaction());
    assert!(router.visibility_graph_dirty_for_tests());
    let graph = router.visibility_graph_for_tests();
    assert_eq!(
        graph.edge_points(0, 1),
        Some((Point { x: 0.0, y: 60.0 }, Point { x: 20.0, y: 60.0 }))
    );
    assert!(graph.edge_between(0, 1).unwrap().visible);

    assert!(router.remove_connector("edge".to_string()));
    assert!(!router.visibility_graph_dirty_for_tests());
    assert!(router.process_transaction());
    assert!(router.visibility_graph_dirty_for_tests());
    assert_eq!(
        router.visibility_graph_for_tests().vertices.iter().count(),
        4
    );
}

#[test]
fn scanline_orders_visibility_vertices_by_quantized_axis() {
    let mut graph = VisibilityGraph::default();
    let v0 = graph.add_vertex(
        VertId::new(1, 0, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: 20.0 },
    );
    let v1 = graph.add_vertex(
        VertId::new(1, 1, PROP_ORTH_SHAPE_EDGE),
        Point { x: 0.0, y: 20.0 },
    );
    let v2 = graph.add_vertex(
        VertId::new(1, 2, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: -5.0 },
    );
    let v3 = graph.add_vertex(
        VertId::new(1, 3, PROP_ORTH_SHAPE_EDGE),
        Point { x: 10.0, y: 0.0 },
    );

    let rows = ordered_axis_lines(&graph, ScanDimension::Y);
    let row_20 = rows.iter().find(|line| line.axis == 20_000).unwrap();
    assert_eq!(row_20.vertices, vec![v1, v0]);

    let columns = ordered_axis_lines(&graph, ScanDimension::X);
    let column_10 = columns.iter().find(|line| line.axis == 10_000).unwrap();
    assert_eq!(column_10.vertices, vec![v2, v3, v0]);
}

#[test]
fn scanline_events_follow_upstream_position_type_and_object_order() {
    let mut events = vec![
        ScanEvent::new(ScanEventType::Close, 10, 1),
        ScanEvent::new(ScanEventType::ConnPoint, 5, 4),
        ScanEvent::new(ScanEventType::Open, 10, 9),
        ScanEvent::new(ScanEventType::SegClose, 10, 2),
        ScanEvent::new(ScanEventType::SegOpen, 10, 3),
        ScanEvent::new(ScanEventType::Open, 10, 0),
    ];

    order_scan_events(&mut events);

    assert_eq!(
        events,
        vec![
            ScanEvent::new(ScanEventType::ConnPoint, 5, 4),
            ScanEvent::new(ScanEventType::Open, 10, 0),
            ScanEvent::new(ScanEventType::Open, 10, 9),
            ScanEvent::new(ScanEventType::SegOpen, 10, 3),
            ScanEvent::new(ScanEventType::SegClose, 10, 2),
            ScanEvent::new(ScanEventType::Close, 10, 1),
        ]
    );
}

#[test]
fn scanline_obstacle_limits_ignore_inline_edges_and_find_nearest_blockers() {
    let obstacles = vec![
        Rect::new(20.0, 40.0, 40.0, 80.0),
        Rect::new(70.0, 40.0, 90.0, 80.0),
        Rect::new(110.0, 40.0, 130.0, 80.0),
    ];

    assert_eq!(
        nearest_obstacle_limits(&obstacles, ScanDimension::X, 60.0, 60.0),
        ScanObstacleLimits {
            lower: Some(40.0),
            upper: Some(70.0),
        }
    );
    assert_eq!(
        nearest_obstacle_limits(&obstacles, ScanDimension::X, 40.0, 60.0),
        ScanObstacleLimits {
            lower: None,
            upper: None,
        }
    );
    assert_eq!(
        nearest_obstacle_limits(&obstacles, ScanDimension::X, 60.0, 75.0),
        ScanObstacleLimits {
            lower: Some(70.0),
            upper: Some(90.0),
        }
    );
    assert_eq!(
        nearest_obstacle_limits(&obstacles, ScanDimension::Y, 80.0, 30.0),
        ScanObstacleLimits {
            lower: None,
            upper: Some(40.0),
        }
    );
}

#[test]
fn node_quantization_round_trips_points_for_visibility_grid() {
    let node = crate::geometry::Node::from_point(Point {
        x: 12.3454,
        y: -9.8765,
    });
    assert_eq!(
        node.to_point(),
        Point {
            x: 12.345,
            y: -9.877
        }
    );
}

#[test]
fn polygon_obstacle_uses_actual_shape_for_zero_clearance() {
    let mut router = AvoidRouter::new(0.0);
    assert!(router.add_polygon_shape(
        "triangle".to_string(),
        vec![40.0, 0.0, 80.0, 40.0, 40.0, 80.0]
    ));
    router.add_connector("edge".to_string(), 55.0, 70.0, 100.0, 70.0);

    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 55.0, y: 70.0 }, Point { x: 100.0, y: 70.0 }]
    );

    router.add_connector("blocked".to_string(), 0.0, 40.0, 100.0, 40.0);
    assert!(router.route(&router.connectors[1]).len() > 2);
}

#[test]
fn polygon_obstacle_uses_actual_offset_shape_for_positive_clearance() {
    let mut router = AvoidRouter::new(5.0);
    assert!(router.add_polygon_shape(
        "triangle".to_string(),
        vec![40.0, 0.0, 80.0, 40.0, 40.0, 80.0]
    ));
    router.add_connector("outside-offset".to_string(), 55.0, 75.0, 100.0, 75.0);

    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 55.0, y: 75.0 }, Point { x: 100.0, y: 75.0 }]
    );

    router.add_connector("blocked-offset".to_string(), 0.0, 40.0, 100.0, 40.0);
    assert!(router.route(&router.connectors[1]).len() > 2);
}

#[test]
fn connector_routing_checkpoints_are_visited_in_order_and_blocked_points_are_skipped() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 120.0, 0.0);
    assert!(router.set_connector_routing_checkpoints(
        "edge".to_string(),
        vec![
            20.0,
            0.0,
            CONN_DIR_ALL as f64,
            CONN_DIR_ALL as f64,
            60.0,
            60.0,
            CONN_DIR_ALL as f64,
            CONN_DIR_ALL as f64,
            100.0,
            0.0,
            CONN_DIR_ALL as f64,
            CONN_DIR_ALL as f64,
        ],
    ));

    assert_eq!(
        router.connector_routing_checkpoints("edge".to_string()),
        concat!(
            "[{\"point\":{\"x\":20,\"y\":0},\"arrivalDirections\":15,\"departureDirections\":15},",
            "{\"point\":{\"x\":60,\"y\":60},\"arrivalDirections\":15,\"departureDirections\":15},",
            "{\"point\":{\"x\":100,\"y\":0},\"arrivalDirections\":15,\"departureDirections\":15}]"
        )
    );
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 100.0, y: 0.0 },
            Point { x: 120.0, y: 0.0 },
        ]
    );
}

#[test]
fn checkpoint_arrival_direction_filters_final_segment_into_checkpoint() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 120.0, 0.0);
    assert!(router.set_connector_routing_checkpoints(
        "edge".to_string(),
        vec![100.0, 0.0, CONN_DIR_UP as f64, CONN_DIR_ALL as f64,],
    ));

    let route = router.route(&router.connectors[0]);
    let checkpoint_index = route
        .iter()
        .position(|point| *point == Point { x: 100.0, y: 0.0 })
        .unwrap();
    assert!(checkpoint_index > 0);
    assert!(route[checkpoint_index - 1].y > 0.0);
    assert_eq!(route.last(), Some(&Point { x: 120.0, y: 0.0 }));
}

#[test]
fn upstream_inline_point_direction_fixture_routes_orthogonally() {
    let mut router = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    router.add_shape("box".to_string(), 0.0, 0.0, 100.0, 100.0);
    for (id, dst, directions) in [
        (
            "right-upper",
            Point { x: 95.0, y: 25.0 },
            CONN_DIR_UP | CONN_DIR_RIGHT | CONN_DIR_DOWN,
        ),
        (
            "right-lower",
            Point { x: 95.0, y: 75.0 },
            CONN_DIR_UP | CONN_DIR_RIGHT | CONN_DIR_DOWN,
        ),
        (
            "top-left",
            Point { x: 25.0, y: 5.0 },
            CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_UP,
        ),
        (
            "top-right",
            Point { x: 75.0, y: 5.0 },
            CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_UP,
        ),
        (
            "bottom-left",
            Point { x: 25.0, y: 95.0 },
            CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_DOWN,
        ),
        (
            "bottom-right",
            Point { x: 75.0, y: 95.0 },
            CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_DOWN,
        ),
    ] {
        router.add_connector_with_endpoint_directions(
            id.to_string(),
            200.0,
            200.0,
            CONN_DIR_ALL,
            dst.x,
            dst.y,
            directions,
        );
    }

    assert!(router.process_transaction());
    assert!(!router.exists_invalid_orthogonal_paths());
    for connector in &router.connectors {
        let route = router.route(connector);
        assert_eq!(route.first(), Some(&Point { x: 200.0, y: 200.0 }));
        let Endpoint::Point { point: dst, .. } = connector.dst else {
            panic!("inline fixture uses point endpoints");
        };
        assert_eq!(route.last(), Some(&dst));
        for segment in route.windows(2) {
            assert!(
                (segment[0].x - segment[1].x).abs() < EPS
                    || (segment[0].y - segment[1].y).abs() < EPS
            );
        }
    }
}

#[test]
fn point_endpoint_directions_filter_first_and_last_segments() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector_with_endpoint_directions(
        "edge".to_string(),
        0.0,
        0.0,
        CONN_DIR_UP,
        120.0,
        0.0,
        CONN_DIR_UP,
    );

    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":0,\"y\":0},",
            "\"position\":{\"x\":0,\"y\":0},\"directions\":1},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":120,\"y\":0},",
            "\"position\":{\"x\":120,\"y\":0},\"directions\":1}}"
        )
    );

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 0.0, y: 0.0 }));
    assert_eq!(route.last(), Some(&Point { x: 120.0, y: 0.0 }));
    assert!(route.len() > 2);
    assert!(route[1].y > 0.0);
    assert!(route[route.len() - 2].y > 0.0);

    assert!(router.set_connector_source_endpoint_with_directions(
        "edge".to_string(),
        0.0,
        10.0,
        CONN_DIR_RIGHT,
    ));
    assert!(router.process_transaction());
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":0,\"y\":10},",
            "\"position\":{\"x\":0,\"y\":10},\"directions\":8},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":120,\"y\":0},",
            "\"position\":{\"x\":120,\"y\":0},\"directions\":1}}"
        )
    );
}

#[test]
fn fixed_route_is_returned_verbatim_and_clear_restores_automatic_routing() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);

    assert!(router
        .set_connector_fixed_route("edge".to_string(), vec![0.0, 60.0, 60.0, 60.0, 120.0, 60.0],));
    assert!(router.process_transaction());
    assert!(!router.process_transaction());
    assert!(router.connector_has_fixed_route("edge".to_string()));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![
            Point { x: 0.0, y: 60.0 },
            Point { x: 60.0, y: 60.0 },
            Point { x: 120.0, y: 60.0 },
        ]
    );

    assert!(router.clear_connector_fixed_route("edge".to_string()));
    assert!(router.process_transaction());
    assert!(!router.connector_has_fixed_route("edge".to_string()));
    let route = router.route(&router.connectors[0]);
    assert_ne!(
        route,
        vec![
            Point { x: 0.0, y: 60.0 },
            Point { x: 60.0, y: 60.0 },
            Point { x: 120.0, y: 60.0 }
        ]
    );
    assert!(route.len() > 2);
}

#[test]
fn invalid_orthogonal_path_diagnostic_checks_display_routes() {
    let mut router = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(!router.exists_invalid_orthogonal_paths());

    assert!(router.set_connector_fixed_route("edge".to_string(), vec![0.0, 0.0, 10.0, 10.0]));
    assert!(router.exists_invalid_orthogonal_paths());

    assert!(router.set_connector_routing_type("edge".to_string(), CONN_TYPE_POLY_LINE));
    assert!(!router.exists_invalid_orthogonal_paths());
}

#[test]
fn upstream_valid_paths02_has_no_invalid_orthogonal_paths() {
    let mut router = AvoidRouter::with_flags(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    assert!(router.set_routing_penalty(SEGMENT_PENALTY, 50.0));
    assert!(router.set_routing_penalty(ANGLE_PENALTY, 0.0));
    assert!(router.set_routing_penalty(CROSSING_PENALTY, 200.0));
    assert!(router.set_routing_penalty(CLUSTER_CROSSING_PENALTY, 4000.0));
    assert!(router.set_routing_penalty(FIXED_SHARED_PATH_PENALTY, 110.0));
    assert!(router.set_routing_penalty(PORT_DIRECTION_PENALTY, 100.0));
    assert!(router.set_routing_option(NUDGE_ORTHOGONAL_SEGMENTS_CONNECTED_TO_SHAPES, false,));
    assert!(router.set_routing_option(IMPROVE_HYPEREDGE_ROUTES_MOVING_JUNCTIONS, true,));
    assert!(router.set_routing_parameter(IDEAL_NUDGING_DISTANCE, 25.0));

    router.add_shape("186982048".to_string(), 52090.0, 50490.0, 52660.0, 50960.0);
    assert!(router.add_shape_connection_pin_with_options(
        "186982048".to_string(),
        "5".to_string(),
        5,
        0.982456,
        0.12766,
        true,
        0.0,
        CONN_DIR_RIGHT,
        f64::NAN,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "186982048".to_string(),
        "13".to_string(),
        13,
        0.982456,
        0.234043,
        true,
        0.0,
        CONN_DIR_RIGHT,
        f64::NAN,
        false,
    ));

    router.add_shape("197692264".to_string(), 52440.0, 50342.0, 52560.0, 50425.0);
    assert!(router.add_shape_connection_pin_with_options(
        "197692264".to_string(),
        "1".to_string(),
        1,
        0.5,
        0.39759,
        true,
        0.0,
        CONN_DIR_UP | CONN_DIR_LEFT | CONN_DIR_RIGHT,
        f64::NAN,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "197692264".to_string(),
        "2".to_string(),
        2,
        0.5,
        1.0,
        true,
        10.0,
        CONN_DIR_DOWN,
        f64::NAN,
        false,
    ));

    assert!(router.add_connector_between_pins(
        "111820287".to_string(),
        "186982048".to_string(),
        "13".to_string(),
        "197692264".to_string(),
        "2".to_string(),
    ));
    assert!(router.add_connector_between_pins(
        "744282799".to_string(),
        "186982048".to_string(),
        "5".to_string(),
        "197692264".to_string(),
        "1".to_string(),
    ));

    assert!(router.process_transaction());
    assert!(!router.exists_invalid_orthogonal_paths());
}

#[test]
fn crossing_diagnostic_counts_strict_display_route_crossings() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("a".to_string(), 0.0, 0.0, 10.0, 10.0);
    router.add_connector("b".to_string(), 0.0, 10.0, 10.0, 0.0);
    assert!(router.set_connector_fixed_route("a".to_string(), vec![0.0, 0.0, 10.0, 10.0]));
    assert!(router.set_connector_fixed_route("b".to_string(), vec![0.0, 10.0, 10.0, 0.0]));

    assert_eq!(router.exists_crossings(false), 1);
    assert_eq!(router.exists_crossings(true), 1);

    assert!(router.set_connector_fixed_route("b".to_string(), vec![0.0, 10.0, 10.0, 10.0]));
    assert_eq!(router.exists_crossings(false), 0);

    router.add_connector("touch".to_string(), 10.0, 10.0, 20.0, 10.0);
    assert!(router.set_connector_fixed_route("touch".to_string(), vec![10.0, 10.0, 20.0, 10.0]));
    assert_eq!(router.exists_crossings(false), 0);
}

#[test]
fn orthogonal_segment_overlap_diagnostic_honours_at_ends_flag() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("a".to_string(), 0.0, 0.0, 10.0, 0.0);
    router.add_connector("b".to_string(), 5.0, 0.0, 15.0, 0.0);
    assert!(router.set_connector_fixed_route("a".to_string(), vec![0.0, 0.0, 10.0, 0.0]));
    assert!(router.set_connector_fixed_route("b".to_string(), vec![5.0, 0.0, 15.0, 0.0]));

    assert!(!router.exists_orthogonal_segment_overlap(false));
    assert!(router.exists_orthogonal_segment_overlap(true));
    assert!(!router.exists_orthogonal_fixed_segment_overlap(false));
    assert!(router.exists_orthogonal_fixed_segment_overlap(true));

    assert!(router.set_connector_fixed_route(
        "a".to_string(),
        vec![0.0, 10.0, 0.0, 0.0, 10.0, 0.0, 10.0, 10.0]
    ));
    assert!(router.set_connector_fixed_route(
        "b".to_string(),
        vec![5.0, -10.0, 5.0, 0.0, 15.0, 0.0, 15.0, -10.0]
    ));
    assert!(router.exists_orthogonal_segment_overlap(false));
    assert!(router.exists_orthogonal_fixed_segment_overlap(false));

    assert!(router.set_connector_fixed_route("b".to_string(), vec![10.0, 0.0, 20.0, 0.0]));
    assert!(!router.exists_orthogonal_segment_overlap(true));
    assert!(!router.exists_orthogonal_fixed_segment_overlap(true));

    let mut automatic = AvoidRouter::new(0.0);
    automatic.add_connector("a".to_string(), 0.0, 0.0, 10.0, 0.0);
    automatic.add_connector("b".to_string(), 5.0, 0.0, 15.0, 0.0);
    assert!(automatic.process_transaction());
    assert!(automatic.exists_orthogonal_segment_overlap(true));
    assert!(!automatic.exists_orthogonal_fixed_segment_overlap(true));
}

#[test]
fn orthogonal_touching_paths_diagnostic_reports_route_contacts() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("a".to_string(), 0.0, 0.0, 10.0, 0.0);
    router.add_connector("b".to_string(), 10.0, 0.0, 10.0, 10.0);
    assert!(router.set_connector_fixed_route("a".to_string(), vec![0.0, 0.0, 10.0, 0.0]));
    assert!(router.set_connector_fixed_route("b".to_string(), vec![10.0, 0.0, 10.0, 10.0]));
    assert!(router.exists_orthogonal_touching_paths());
    assert!(!router.exists_orthogonal_segment_overlap(true));

    assert!(router.set_connector_fixed_route("b".to_string(), vec![20.0, 0.0, 20.0, 10.0]));
    assert!(!router.exists_orthogonal_touching_paths());

    assert!(router.set_connector_fixed_route("b".to_string(), vec![5.0, 0.0, 15.0, 0.0]));
    assert!(router.exists_orthogonal_touching_paths());
}

#[test]
fn fixed_existing_route_freezes_current_polyline() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("box".to_string(), 40.0, 40.0, 80.0, 80.0);
    router.add_connector("edge".to_string(), 0.0, 60.0, 120.0, 60.0);
    let automatic_route = router.route(&router.connectors[0]);
    assert!(automatic_route.len() > 2);

    assert!(router.set_connector_fixed_existing_route("edge".to_string()));
    assert!(router.remove_shape("box".to_string()));
    assert_eq!(router.route(&router.connectors[0]), automatic_route);
    assert!(router.process_transaction());

    assert!(router.clear_connector_fixed_route("edge".to_string()));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 60.0 }, Point { x: 120.0, y: 60.0 }]
    );
}

#[test]
fn connector_point_endpoints_can_be_updated_after_creation() {
    let mut router = AvoidRouter::new(0.0);
    router.set_transaction_use(false);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);

    assert!(router.set_connector_source_endpoint("edge".to_string(), 5.0, 5.0));
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 5.0, y: 5.0 }));
    assert_eq!(route.last(), Some(&Point { x: 10.0, y: 0.0 }));

    assert!(router.set_connector_dest_endpoint("edge".to_string(), 20.0, 5.0));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 5.0, y: 5.0 }, Point { x: 20.0, y: 5.0 }]
    );

    assert!(router.set_connector_endpoints("edge".to_string(), 1.0, 2.0, 3.0, 4.0));
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 1.0, y: 2.0 }));
    assert_eq!(route.last(), Some(&Point { x: 3.0, y: 4.0 }));
    assert!(!router.set_connector_dest_endpoint("missing".to_string(), 0.0, 0.0));
}

#[test]
fn connector_shape_pin_endpoints_can_be_updated_after_creation() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin("left".to_string(), "east".to_string(), 40.0, 20.0));
    assert!(router.add_shape_connection_pin("right".to_string(), "west".to_string(), 100.0, 20.0));
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);

    assert!(router.set_connector_source_pin_endpoint(
        "edge".to_string(),
        "left".to_string(),
        "east".to_string()
    ));
    assert!(router.set_connector_dest_pin_endpoint(
        "edge".to_string(),
        "right".to_string(),
        "west".to_string()
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":0,\"y\":0},",
            "\"position\":{\"x\":0,\"y\":0},\"directions\":15},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":10,\"y\":0},",
            "\"position\":{\"x\":10,\"y\":0},\"directions\":15}}"
        )
    );
    assert!(router.process_transaction());
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"left\",\"pinId\":\"east\",",
            "\"position\":{\"x\":40,\"y\":20},\"directions\":15},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"pinId\":\"west\",",
            "\"position\":{\"x\":100,\"y\":20},\"directions\":4}}"
        )
    );
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 40.0, y: 20.0 }));
    assert_eq!(route.last(), Some(&Point { x: 100.0, y: 20.0 }));

    router.set_transaction_use(false);
    assert!(router.set_connector_source_endpoint("edge".to_string(), 5.0, 5.0));
    assert!(router.set_connector_source_pin_endpoint(
        "edge".to_string(),
        "left".to_string(),
        "east".to_string()
    ));
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin { ref pin_id, .. } if pin_id == "east"
    ));
    assert!(!router.set_connector_dest_pin_endpoint(
        "edge".to_string(),
        "right".to_string(),
        "missing".to_string()
    ));
}

#[test]
fn connector_shape_pin_class_endpoints_can_be_updated_after_creation() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "expensive".to_string(),
        1,
        ATTACH_POS_RIGHT,
        0.25,
        true,
        0.0,
        CONN_DIR_RIGHT,
        10.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "cheap".to_string(),
        1,
        ATTACH_POS_RIGHT,
        0.75,
        true,
        0.0,
        CONN_DIR_RIGHT,
        1.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "target".to_string(),
        2,
        ATTACH_POS_LEFT,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);

    assert!(router.set_connector_source_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        1
    ));
    assert!(router.set_connector_dest_pin_class_endpoint(
        "edge".to_string(),
        "right".to_string(),
        2
    ));
    assert!(router
        .connector_endpoint_conn_ends("edge".to_string())
        .contains("\"type\":\"point\""));
    assert!(router.process_transaction());

    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"left\",\"pinId\":\"cheap\",",
            "\"classId\":1,\"position\":{\"x\":40,\"y\":30},\"directions\":8},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"pinId\":\"target\",",
            "\"classId\":2,\"position\":{\"x\":100,\"y\":20},\"directions\":4}}"
        )
    );
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 40.0, y: 30.0 }));
    assert_eq!(route.last(), Some(&Point { x: 100.0, y: 20.0 }));

    router.set_transaction_use(false);
    assert!(router.set_connector_source_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        1
    ));
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::ShapePin {
            ref pin_id,
            pin_class_id: Some(1),
            ..
        } if pin_id == "cheap"
    ));
    assert!(router.set_connector_dest_pin_class_endpoint(
        "edge".to_string(),
        "right".to_string(),
        99
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"left\",\"pinId\":\"cheap\",",
            "\"classId\":1,\"position\":{\"x\":40,\"y\":30},\"directions\":8},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"classId\":99,",
            "\"position\":{\"x\":120,\"y\":20},\"directions\":15}}"
        )
    );
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "late".to_string(),
        99,
        0.0,
        10.0,
        false,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(matches!(
        router.connectors[0].dst,
        Endpoint::ShapePin {
            ref pin_id,
            pin_class_id: Some(99),
            ..
        } if pin_id == "late"
    ));
}

#[test]
fn deleting_shape_detaches_shape_pin_connends_to_points() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("left".to_string(), 0.0, 0.0, 40.0, 40.0);
    router.add_shape("right".to_string(), 100.0, 0.0, 140.0, 40.0);
    assert!(router.add_shape_connection_pin_with_options(
        "left".to_string(),
        "out".to_string(),
        1,
        ATTACH_POS_RIGHT,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_RIGHT,
        0.0,
        true,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "right".to_string(),
        "in".to_string(),
        2,
        ATTACH_POS_LEFT,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_LEFT,
        0.0,
        true,
    ));
    assert!(router.add_connector_between_pins(
        "edge".to_string(),
        "left".to_string(),
        "out".to_string(),
        "right".to_string(),
        "in".to_string(),
    ));

    assert!(router.remove_shape("left".to_string()));
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":40,\"y\":20},",
            "\"position\":{\"x\":40,\"y\":20},\"directions\":15},",
            "\"destination\":{\"type\":\"shapePin\",\"shapeId\":\"right\",\"pinId\":\"in\",",
            "\"position\":{\"x\":100,\"y\":20},\"directions\":4}}"
        )
    );

    router.set_transaction_use(false);
    assert!(router.remove_shape("right".to_string()));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":40,\"y\":20},",
            "\"position\":{\"x\":40,\"y\":20},\"directions\":15},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":100,\"y\":20},",
            "\"position\":{\"x\":100,\"y\":20},\"directions\":15}}"
        )
    );
}

#[test]
fn endpoint_updates_are_used_after_fixed_route_is_cleared() {
    let mut router = AvoidRouter::new(0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 10.0, 0.0);
    assert!(
        router.set_connector_fixed_route("edge".to_string(), vec![0.0, 0.0, 5.0, 5.0, 10.0, 0.0])
    );
    assert!(router.set_connector_endpoints("edge".to_string(), 20.0, 20.0, 40.0, 20.0));
    assert!(router.process_transaction());

    assert_eq!(
        router.route(&router.connectors[0]),
        vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 5.0, y: 5.0 },
            Point { x: 10.0, y: 0.0 },
        ]
    );

    assert!(router.clear_connector_fixed_route("edge".to_string()));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 20.0, y: 20.0 }, Point { x: 40.0, y: 20.0 }]
    );
}

#[test]
fn junction_endpoints_follow_junction_moves_and_can_be_removed() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("j".to_string(), 10.0, 20.0);
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert_eq!(
        router.junction_recommended_position("j".to_string()),
        "{\"x\":10,\"y\":20}"
    );
    assert!(!router.junction_position_fixed("j".to_string()));
    assert!(router.set_junction_position_fixed("j".to_string(), true));
    assert!(router.junction_position_fixed("j".to_string()));

    assert!(router.add_connector_from_junction("edge".to_string(), "j".to_string(), 50.0, 20.0));
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 10.0, y: 20.0 }, Point { x: 50.0, y: 20.0 }]
    );

    assert!(router.move_junction_by("j".to_string(), 5.0, -10.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":15,\"y\":10}"
    );
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 15.0, y: 10.0 }));
    assert_eq!(route.last(), Some(&Point { x: 50.0, y: 20.0 }));

    assert!(router.remove_junction("j".to_string()));
    assert_ne!(router.route_connector("edge".to_string()), "null");
    assert!(router.process_transaction());
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"point\",\"point\":{\"x\":15,\"y\":10},",
            "\"position\":{\"x\":15,\"y\":10},\"directions\":15},",
            "\"destination\":{\"type\":\"point\",\"point\":{\"x\":50,\"y\":20},",
            "\"position\":{\"x\":50,\"y\":20},\"directions\":15}}"
        )
    );
    assert_ne!(router.route_connector("edge".to_string()), "null");
}

#[test]
fn connectors_can_attach_to_junctions_after_creation() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("a".to_string(), 0.0, 0.0);
    router.add_junction("b".to_string(), 100.0, 0.0);
    router.add_connector("edge".to_string(), 10.0, 10.0, 90.0, 10.0);

    assert!(router.set_connector_source_junction_endpoint("edge".to_string(), "a".to_string()));
    assert!(router.set_connector_dest_junction_endpoint("edge".to_string(), "b".to_string()));
    assert!(router.process_transaction());
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );

    assert!(router.move_junction("b".to_string(), 120.0, 0.0));
    assert!(router.process_transaction());
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 120.0, y: 0.0 }]
    );
    assert!(!router.set_connector_dest_junction_endpoint("edge".to_string(), "missing".to_string()));
}

#[test]
fn connector_junction_pin_class_endpoints_can_be_updated_after_creation() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("left".to_string(), 0.0, 0.0);
    router.add_junction("right".to_string(), 100.0, 0.0);
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "expensive".to_string(),
        7,
        CONN_DIR_RIGHT,
    ));
    assert!(router.add_junction_connection_pin(
        "left".to_string(),
        "cheap".to_string(),
        7,
        CONN_DIR_RIGHT,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "expensive".to_string(),
        10.0,
    ));
    assert!(router.set_junction_connection_pin_connection_cost(
        "left".to_string(),
        "cheap".to_string(),
        1.0,
    ));
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "target".to_string(),
        8,
        CONN_DIR_LEFT,
    ));
    router.add_connector("edge".to_string(), 10.0, 10.0, 90.0, 10.0);

    assert!(router.set_connector_source_junction_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        7
    ));
    assert!(router.set_connector_dest_junction_pin_class_endpoint(
        "edge".to_string(),
        "right".to_string(),
        8
    ));
    assert!(router.process_transaction());

    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"junctionPin\",\"junctionId\":\"left\",\"pinId\":\"cheap\",",
            "\"classId\":7,\"position\":{\"x\":0,\"y\":0},\"directions\":8},",
            "\"destination\":{\"type\":\"junctionPin\",\"junctionId\":\"right\",\"pinId\":\"target\",",
            "\"classId\":8,\"position\":{\"x\":100,\"y\":0},\"directions\":4}}"
        )
    );

    router.set_transaction_use(false);
    assert!(router.set_connector_source_junction_pin_class_endpoint(
        "edge".to_string(),
        "left".to_string(),
        7
    ));
    assert!(matches!(
        router.connectors[0].src,
        Endpoint::JunctionPin {
            ref pin_id,
            pin_class_id: Some(7),
            ..
        } if pin_id == "cheap"
    ));
    assert!(router.set_connector_dest_junction_pin_class_endpoint(
        "edge".to_string(),
        "right".to_string(),
        99
    ));
    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"junctionPin\",\"junctionId\":\"left\",\"pinId\":\"cheap\",",
            "\"classId\":7,\"position\":{\"x\":0,\"y\":0},\"directions\":8},",
            "\"destination\":{\"type\":\"junctionPin\",\"junctionId\":\"right\",\"classId\":99,",
            "\"position\":{\"x\":100,\"y\":0},\"directions\":15}}"
        )
    );
    assert!(router.add_junction_connection_pin(
        "right".to_string(),
        "late".to_string(),
        99,
        CONN_DIR_LEFT,
    ));
    assert!(matches!(
        router.connectors[0].dst,
        Endpoint::JunctionPin {
            ref pin_id,
            pin_class_id: Some(99),
            ..
        } if pin_id == "late"
    ));
}

#[test]
fn connector_endpoint_conn_ends_reports_endpoint_types() {
    let mut router = AvoidRouter::new(0.0);
    router.add_shape("shape".to_string(), 10.0, 10.0, 30.0, 30.0);
    assert!(router.add_shape_connection_pin("shape".to_string(), "pin".to_string(), 20.0, 10.0));
    router.add_junction("j".to_string(), 100.0, 100.0);
    assert!(router.add_connector_between_pins(
        "edge".to_string(),
        "shape".to_string(),
        "pin".to_string(),
        "shape".to_string(),
        "pin".to_string(),
    ));
    assert!(router.set_connector_dest_junction_endpoint("edge".to_string(), "j".to_string()));
    assert!(router.process_transaction());

    assert_eq!(
        router.connector_endpoint_conn_ends("edge".to_string()),
        concat!(
            "{\"source\":{\"type\":\"shapePin\",\"shapeId\":\"shape\",\"pinId\":\"pin\",",
            "\"position\":{\"x\":20,\"y\":10},\"directions\":1},",
            "\"destination\":{\"type\":\"junction\",\"junctionId\":\"j\",",
            "\"position\":{\"x\":100,\"y\":100},\"directions\":15}}"
        )
    );
    assert_eq!(
        router.connector_endpoint_conn_ends("missing".to_string()),
        "null"
    );
}

#[test]
fn junction_with_two_attached_connector_ends_can_be_removed_and_merged() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("j".to_string(), 50.0, 0.0);
    assert!(router.add_connector_to_junction("left".to_string(), 0.0, 0.0, "j".to_string()));
    assert!(router.add_connector_from_junction("right".to_string(), "j".to_string(), 100.0, 0.0));

    assert_eq!(
        router.remove_junction_and_merge_connectors("j".to_string()),
        "\"left\""
    );
    assert_eq!(router.connectors.len(), 1);
    assert_eq!(router.junction_position("j".to_string()), "null");
    assert_eq!(router.route_connector("right".to_string()), "null");
    assert_eq!(
        router.route(&router.connectors[0]),
        vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }]
    );
    assert!(matches!(
        &router.connectors[0].src,
        Endpoint::Point {
            point: Point { x: 0.0, y: 0.0 },
            directions: CONN_DIR_ALL
        }
    ));
    assert!(matches!(
        &router.connectors[0].dst,
        Endpoint::Point {
            point: Point { x: 100.0, y: 0.0 },
            directions: CONN_DIR_ALL
        }
    ));
}

#[test]
fn junction_merge_rejects_missing_or_non_degree_two_junctions() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("j".to_string(), 50.0, 0.0);
    assert!(router.add_connector_from_junction("only".to_string(), "j".to_string(), 100.0, 0.0));

    assert_eq!(
        router.remove_junction_and_merge_connectors("missing".to_string()),
        "null"
    );
    assert_eq!(
        router.remove_junction_and_merge_connectors("j".to_string()),
        "null"
    );
    assert_eq!(
        router.junction_position("j".to_string()),
        "{\"x\":50,\"y\":0}"
    );
    assert_eq!(router.connectors.len(), 1);
}

#[test]
fn unattached_junctions_are_small_obstacles() {
    let mut router = AvoidRouter::new(0.0);
    router.add_junction("mid".to_string(), 50.0, 0.0);
    router.add_connector("edge".to_string(), 0.0, 0.0, 100.0, 0.0);

    let route = router.route(&router.connectors[0]);
    assert!(route.len() > 2);
    for segment in route.windows(2) {
        assert!(segment[0] != Point { x: 50.0, y: 0.0 });
        assert!(segment[1] != Point { x: 50.0, y: 0.0 });
    }
}

#[test]
fn upstream_connectionpin01_centres_follow_relative_shape_moves() {
    // Translated from vendor/adaptagrams/cola/libavoid/tests/connectionpin01.cpp.
    let mut router = AvoidRouter::with_flags(ORTHOGONAL_ROUTING);
    router.add_shape("shape1".to_string(), 0.0, 0.0, 10.0, 10.0);
    router.add_shape("shape2".to_string(), 40.0, 20.0, 50.0, 30.0);
    assert!(router.add_shape_connection_pin_with_options(
        "shape1".to_string(),
        "centre".to_string(),
        1,
        ATTACH_POS_CENTRE,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));
    assert!(router.add_shape_connection_pin_with_options(
        "shape2".to_string(),
        "centre".to_string(),
        1,
        ATTACH_POS_CENTRE,
        ATTACH_POS_CENTRE,
        true,
        0.0,
        CONN_DIR_NONE,
        0.0,
        false,
    ));
    assert!(router.add_connector_between_pin_classes(
        "conn".to_string(),
        "shape2".to_string(),
        1,
        "shape1".to_string(),
        1,
    ));

    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 45.0, y: 25.0 }));
    assert_eq!(route.last(), Some(&Point { x: 5.0, y: 5.0 }));

    assert!(router.move_shape_by("shape2".to_string(), 5.0, 0.0));
    assert!(router.process_transaction());
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 50.0, y: 25.0 }));
    assert_eq!(route.last(), Some(&Point { x: 5.0, y: 5.0 }));

    assert!(router.move_shape_by("shape1".to_string(), 0.0, -10.0));
    assert!(router.process_transaction());
    let route = router.route(&router.connectors[0]);
    assert_eq!(route.first(), Some(&Point { x: 50.0, y: 25.0 }));
    assert_eq!(route.last(), Some(&Point { x: 5.0, y: -5.0 }));
}
