use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use wasm_bindgen::prelude::*;

use crate::constants::*;
use crate::geometry::{
    angle, manhattan_distance, rotational_angle, segment_intersect, Point, Polygon, Rect,
};
use crate::graph::VisibilityGraph;
use crate::json::{number_to_json, points_to_json, push_json_string};
use crate::model::{
    pin_visibility, Checkpoint, Cluster, Connector, Endpoint, Junction, Shape, ShapeConnectionPin,
};
use crate::orthogonal::{
    clear_segment, route_crossing_count, route_orthogonal_with_directions, route_shared_path_count,
    segment_reverses_direction, segment_satisfies_directions, OrthogonalObstacle,
    RoutingCostSettings,
};
use crate::visibility::build_static_visibility_graph;

#[wasm_bindgen]
pub struct AvoidRouter {
    pub(crate) flags: u32,
    pub(crate) clearance: f64,
    pub(crate) routing_parameters: [f64; ROUTING_PARAMETER_COUNT],
    pub(crate) routing_options: [bool; ROUTING_OPTION_COUNT],
    pub(crate) transaction_use: bool,
    settings_changed: bool,
    last_transaction_updated_connector_ids: Vec<String>,
    pending_actions: Vec<PendingAction>,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) clusters: Vec<Cluster>,
    pub(crate) junctions: Vec<Junction>,
    pub(crate) connectors: Vec<Connector>,
    visibility_graph: VisibilityGraph,
    visibility_graph_dirty: bool,
}

#[derive(Clone, Debug)]
enum PendingAction {
    DeleteShape {
        id: String,
    },
    DeleteShapeConnectionPin {
        shape_id: String,
        pin_id: String,
    },
    ChangeShapeConnectionPin {
        shape_id: String,
        pin_id: String,
    },
    SetShapeRect {
        id: String,
        rect: Rect,
    },
    SetShapePolygon {
        id: String,
        polygon: Polygon,
    },
    TranslateShape {
        id: String,
        x_diff: f64,
        y_diff: f64,
    },
    SetJunctionPosition {
        id: String,
        position: Point,
    },
    TranslateJunction {
        id: String,
        x_diff: f64,
        y_diff: f64,
    },
    DeleteJunction {
        id: String,
    },
    DeleteJunctionConnectionPin {
        junction_id: String,
        pin_id: String,
    },
    ChangeJunctionConnectionPin {
        junction_id: String,
        pin_id: String,
    },
    SetClusterPolygon {
        id: String,
        polygon: Polygon,
    },
    DeleteCluster {
        id: String,
    },
    DeleteConnector {
        id: String,
    },
    SetConnectorEndpoints {
        id: String,
        src: Endpoint,
        dst: Endpoint,
    },
    SetConnectorSourceEndpoint {
        id: String,
        src: Endpoint,
    },
    SetConnectorDestEndpoint {
        id: String,
        dst: Endpoint,
    },
}

enum EndpointReplacement {
    Unchanged,
    Replace(Endpoint),
    RemoveConnector,
}

#[derive(Clone, Copy, Debug)]
struct PolylineQueueState {
    cost: f64,
    node: usize,
    previous: Option<usize>,
}

#[wasm_bindgen]
impl AvoidRouter {
    #[wasm_bindgen(constructor)]
    pub fn new(clearance: f64) -> AvoidRouter {
        let mut routing_parameters = [0.0; ROUTING_PARAMETER_COUNT];
        for (parameter, value) in routing_parameters.iter_mut().enumerate() {
            *value = default_routing_parameter(parameter as u32);
        }
        routing_parameters[SHAPE_BUFFER_DISTANCE as usize] = clearance.max(0.0);
        AvoidRouter {
            flags: ORTHOGONAL_ROUTING,
            clearance: clearance.max(0.0),
            routing_parameters,
            routing_options: default_routing_options(),
            transaction_use: true,
            settings_changed: false,
            last_transaction_updated_connector_ids: Vec::new(),
            pending_actions: Vec::new(),
            shapes: Vec::new(),
            clusters: Vec::new(),
            junctions: Vec::new(),
            connectors: Vec::new(),
            visibility_graph: VisibilityGraph::default(),
            visibility_graph_dirty: true,
        }
    }

    #[wasm_bindgen(js_name = withFlags)]
    pub fn with_flags(flags: u32) -> AvoidRouter {
        let mut router = AvoidRouter::new(0.0);
        router.flags = flags;
        router
    }

    #[wasm_bindgen(js_name = routerFlags)]
    pub fn router_flags(&self) -> u32 {
        self.flags
    }

    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&mut self) {
        self.settings_changed = false;
        self.last_transaction_updated_connector_ids.clear();
        self.pending_actions.clear();
        self.shapes.clear();
        self.clusters.clear();
        self.junctions.clear();
        self.connectors.clear();
        self.visibility_graph = VisibilityGraph::default();
        self.visibility_graph_dirty = true;
    }

    #[wasm_bindgen(js_name = setTransactionUse)]
    pub fn set_transaction_use(&mut self, transactions: bool) {
        self.transaction_use = transactions;
    }

    #[wasm_bindgen(js_name = transactionUse)]
    pub fn transaction_use(&self) -> bool {
        self.transaction_use
    }

    #[wasm_bindgen(js_name = processTransaction)]
    pub fn process_transaction(&mut self) -> bool {
        let has_pending_actions = !self.pending_actions.is_empty();
        let has_invalid_connectors = self
            .connectors
            .iter()
            .any(|connector| connector.needs_reroute);
        let has_settings_changes = self.settings_changed;
        if !has_pending_actions && !has_invalid_connectors && !has_settings_changes {
            self.last_transaction_updated_connector_ids.clear();
            return false;
        }
        self.settings_changed = false;
        let actions = std::mem::take(&mut self.pending_actions);
        for action in actions {
            self.apply_pending_action(action);
        }
        self.last_transaction_updated_connector_ids = self.refresh_invalidated_routes();
        has_pending_actions
            || !self.last_transaction_updated_connector_ids.is_empty()
            || has_settings_changes
    }

    #[wasm_bindgen(js_name = lastTransactionUpdatedConnectorIds)]
    pub fn last_transaction_updated_connector_ids(&self) -> String {
        ids_to_json(
            self.last_transaction_updated_connector_ids
                .iter()
                .map(|id| id.as_str()),
        )
    }

    #[wasm_bindgen(js_name = setRoutingParameter)]
    pub fn set_routing_parameter(&mut self, parameter: u32, value: f64) -> bool {
        let Some(slot) = self.routing_parameters.get_mut(parameter as usize) else {
            return false;
        };
        *slot = if value < 0.0 {
            sensible_routing_parameter(parameter)
        } else {
            value
        };
        if parameter == SHAPE_BUFFER_DISTANCE {
            self.clearance = (*slot).max(0.0);
        }
        self.settings_changed = true;
        self.invalidate_all_connector_paths();
        true
    }

    #[wasm_bindgen(js_name = setRoutingPenalty)]
    pub fn set_routing_penalty(&mut self, penalty_type: u32, penalty_value: f64) -> bool {
        self.set_routing_parameter(penalty_type, penalty_value)
    }

    #[wasm_bindgen(js_name = routingParameter)]
    pub fn routing_parameter(&self, parameter: u32) -> f64 {
        self.routing_parameters
            .get(parameter as usize)
            .copied()
            .unwrap_or(f64::NAN)
    }

    #[wasm_bindgen(js_name = setRoutingOption)]
    pub fn set_routing_option(&mut self, option: u32, value: bool) -> bool {
        let Some(slot) = self.routing_options.get_mut(option as usize) else {
            return false;
        };
        *slot = value;
        self.settings_changed = true;
        true
    }

    #[wasm_bindgen(js_name = routingOption)]
    pub fn routing_option(&self, option: u32) -> bool {
        self.routing_options
            .get(option as usize)
            .copied()
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = validConnType)]
    pub fn valid_conn_type(&self, select: u32) -> u32 {
        if select != CONN_TYPE_NONE {
            if select == CONN_TYPE_ORTHOGONAL && (self.flags & ORTHOGONAL_ROUTING) != 0 {
                return CONN_TYPE_ORTHOGONAL;
            }
            if select == CONN_TYPE_POLY_LINE && (self.flags & POLY_LINE_ROUTING) != 0 {
                return CONN_TYPE_POLY_LINE;
            }
        }
        if (self.flags & POLY_LINE_ROUTING) != 0 {
            CONN_TYPE_POLY_LINE
        } else if (self.flags & ORTHOGONAL_ROUTING) != 0 {
            CONN_TYPE_ORTHOGONAL
        } else {
            CONN_TYPE_NONE
        }
    }

    #[wasm_bindgen(js_name = objectIdIsUnused)]
    pub fn object_id_is_unused_public(&self, id: String) -> bool {
        self.object_id_is_unused(&id)
    }

    #[wasm_bindgen(js_name = addCluster)]
    pub fn add_cluster(&mut self, id: String, coordinates: Vec<f64>) -> bool {
        let Some(polygon) = polygon_from_flat_coordinates(coordinates) else {
            return false;
        };
        let Some(cluster) = Cluster::from_polygon(id.clone(), polygon) else {
            return false;
        };
        self.force_remove_cluster(&id);
        self.remove_pending_cluster_actions(&id);
        self.clusters.push(cluster);
        self.invalidate_all_connector_paths();
        true
    }

    #[wasm_bindgen(js_name = addClusterRectangle)]
    pub fn add_cluster_rectangle(
        &mut self,
        id: String,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    ) -> bool {
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            return false;
        }
        let polygon = Rect::new(left, top, right, bottom).to_polygon();
        let Some(cluster) = Cluster::from_polygon(id.clone(), polygon) else {
            return false;
        };
        self.force_remove_cluster(&id);
        self.remove_pending_cluster_actions(&id);
        self.clusters.push(cluster);
        self.invalidate_all_connector_paths();
        true
    }

    #[wasm_bindgen(js_name = setClusterPolygon)]
    pub fn set_cluster_polygon(&mut self, id: String, coordinates: Vec<f64>) -> bool {
        let Some(polygon) = polygon_from_flat_coordinates(coordinates) else {
            return false;
        };
        if !self.clusters.iter().any(|cluster| cluster.id == id) {
            return false;
        };
        if self.transaction_use {
            self.remove_pending_cluster_actions(&id);
            self.pending_actions
                .push(PendingAction::SetClusterPolygon { id, polygon });
        } else {
            let Some(cluster) = self.clusters.iter_mut().find(|cluster| cluster.id == id) else {
                return false;
            };
            if !cluster.set_polygon(polygon) {
                return false;
            }
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = setClusterRectangle)]
    pub fn set_cluster_rectangle(
        &mut self,
        id: String,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    ) -> bool {
        if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
            return false;
        }
        if !self.clusters.iter().any(|cluster| cluster.id == id) {
            return false;
        };
        let polygon = Rect::new(left, top, right, bottom).to_polygon();
        if self.transaction_use {
            self.remove_pending_cluster_actions(&id);
            self.pending_actions
                .push(PendingAction::SetClusterPolygon { id, polygon });
        } else {
            let Some(cluster) = self.clusters.iter_mut().find(|cluster| cluster.id == id) else {
                return false;
            };
            if !cluster.set_polygon(polygon) {
                return false;
            }
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = removeCluster)]
    pub fn remove_cluster(&mut self, id: String) -> bool {
        if !self.clusters.iter().any(|cluster| cluster.id == id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_cluster_actions(&id);
            self.pending_actions
                .push(PendingAction::DeleteCluster { id });
            true
        } else {
            self.force_remove_cluster(&id)
        }
    }

    #[wasm_bindgen(js_name = clusterBoundingBox)]
    pub fn cluster_bounding_box(&self, id: String) -> String {
        let Some(cluster) = self.clusters.iter().find(|cluster| cluster.id == id) else {
            return "null".to_string();
        };
        format!(
            "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
            number_to_json(cluster.rect.left),
            number_to_json(cluster.rect.top),
            number_to_json(cluster.rect.right),
            number_to_json(cluster.rect.bottom)
        )
    }

    #[wasm_bindgen(js_name = clusterPolygon)]
    pub fn cluster_polygon(&self, id: String) -> String {
        let Some(cluster) = self.clusters.iter().find(|cluster| cluster.id == id) else {
            return "null".to_string();
        };
        polygon_to_json(&cluster.polygon)
    }

    #[wasm_bindgen(js_name = clusterRectangularPolygon)]
    pub fn cluster_rectangular_polygon(&self, id: String) -> String {
        let Some(cluster) = self.clusters.iter().find(|cluster| cluster.id == id) else {
            return "null".to_string();
        };
        polygon_to_json(&cluster.rect.to_polygon())
    }

    #[wasm_bindgen(js_name = addShape)]
    pub fn add_shape(&mut self, id: String, left: f64, top: f64, right: f64, bottom: f64) {
        self.force_remove_shape(&id);
        self.remove_pending_shape_actions(&id);
        self.shapes
            .push(Shape::from_rect(id, Rect::new(left, top, right, bottom)));
        self.invalidate_all_connector_paths();
    }

    #[wasm_bindgen(js_name = addPolygonShape)]
    pub fn add_polygon_shape(&mut self, id: String, coordinates: Vec<f64>) -> bool {
        let Some(polygon) = polygon_from_flat_coordinates(coordinates) else {
            return false;
        };
        let Some(shape) = Shape::from_polygon(id.clone(), polygon) else {
            return false;
        };
        self.force_remove_shape(&id);
        self.remove_pending_shape_actions(&id);
        self.shapes.push(shape);
        self.invalidate_all_connector_paths();
        true
    }

    #[wasm_bindgen(js_name = moveShape)]
    pub fn move_shape(&mut self, id: String, left: f64, top: f64, right: f64, bottom: f64) -> bool {
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return false;
        }
        let rect = Rect::new(left, top, right, bottom);
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetShapeRect { id, rect });
        } else if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
            shape.set_rect(rect);
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = movePolygonShape)]
    pub fn move_polygon_shape(&mut self, id: String, coordinates: Vec<f64>) -> bool {
        let Some(polygon) = polygon_from_flat_coordinates(coordinates) else {
            return false;
        };
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return false;
        }
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetShapePolygon { id, polygon });
            true
        } else if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
            let changed = shape.set_polygon(polygon);
            if changed {
                self.invalidate_all_connector_paths();
            }
            changed
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = shapeBoundingBox)]
    pub fn shape_bounding_box(&self, id: String) -> String {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == id) else {
            return "null".to_string();
        };
        format!(
            "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
            shape.rect.left, shape.rect.top, shape.rect.right, shape.rect.bottom
        )
    }

    #[wasm_bindgen(js_name = shapePolygon)]
    pub fn shape_polygon(&self, id: String) -> String {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == id) else {
            return "null".to_string();
        };
        polygon_to_json(&shape.polygon)
    }

    #[wasm_bindgen(js_name = shapePosition)]
    pub fn shape_position(&self, id: String) -> String {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == id) else {
            return "null".to_string();
        };
        let position = Point {
            x: shape.rect.left + ((shape.rect.right - shape.rect.left) * 0.5),
            y: shape.rect.top + ((shape.rect.bottom - shape.rect.top) * 0.5),
        };
        point_to_json(position)
    }

    #[wasm_bindgen(js_name = shapeContainingPoint)]
    pub fn shape_containing_point(&self, x: f64, y: f64) -> String {
        let point = Point { x, y };
        let Some(shape) = self
            .shapes
            .iter()
            .find(|shape| shape.polygon.contains_point(point, true))
        else {
            return "null".to_string();
        };
        json_string(&shape.id)
    }

    #[wasm_bindgen(js_name = shapeAttachedConnectors)]
    pub fn shape_attached_connectors(&self, id: String) -> String {
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return "null".to_string();
        }
        ids_to_json(
            self.connectors
                .iter()
                .filter(|connector| connector.references_shape(&id))
                .map(|connector| connector.id.as_str()),
        )
    }

    #[wasm_bindgen(js_name = shapeAttachedShapes)]
    pub fn shape_attached_shapes(&self, id: String, relation: u32) -> String {
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return "null".to_string();
        }
        let mut shape_ids = Vec::new();
        for connector in &self.connectors {
            if (relation & RUNNING_TO) != 0 && connector.dst.references_shape(&id) {
                if let Some(other_id) = connector.src.shape_id() {
                    push_unique_id(&mut shape_ids, other_id);
                }
            } else if (relation & RUNNING_FROM) != 0 && connector.src.references_shape(&id) {
                if let Some(other_id) = connector.dst.shape_id() {
                    push_unique_id(&mut shape_ids, other_id);
                }
            }
        }
        ids_to_json(shape_ids.into_iter())
    }

    #[wasm_bindgen(js_name = transformShapeConnectionPinPositions)]
    pub fn transform_shape_connection_pin_positions(&mut self, id: String, transform: u32) -> bool {
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) else {
            return false;
        };
        if !shape.transform_connection_pin_positions(transform) {
            return false;
        }
        self.invalidate_connectors_for_shape(&id);
        true
    }

    #[wasm_bindgen(js_name = moveShapeBy)]
    pub fn move_shape_by(&mut self, id: String, x_diff: f64, y_diff: f64) -> bool {
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return false;
        }
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::TranslateShape { id, x_diff, y_diff });
        } else if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
            shape.translate(x_diff, y_diff);
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = removeShape)]
    pub fn remove_shape(&mut self, id: String) -> bool {
        if !self.shapes.iter().any(|shape| shape.id == id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_shape_actions(&id);
            self.pending_actions.push(PendingAction::DeleteShape { id });
            true
        } else {
            self.force_remove_shape(&id)
        }
    }

    #[wasm_bindgen(js_name = addJunction)]
    pub fn add_junction(&mut self, id: String, x: f64, y: f64) {
        self.force_remove_junction(&id);
        self.remove_pending_junction_actions(&id);
        let position = Point { x, y };
        self.junctions.push(Junction {
            id,
            position,
            recommended_position: position,
            fixed: false,
            preferred_orthogonal_dimension: None,
            pins: default_junction_pins(),
        });
        self.invalidate_all_connector_paths();
    }

    #[wasm_bindgen(js_name = removeJunction)]
    pub fn remove_junction(&mut self, id: String) -> bool {
        if !self.junctions.iter().any(|junction| junction.id == id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_junction_actions(&id);
            self.pending_actions
                .push(PendingAction::DeleteJunction { id });
            true
        } else {
            self.force_remove_junction(&id)
        }
    }

    #[wasm_bindgen(js_name = removeJunctionAndMergeConnectors)]
    pub fn remove_junction_and_merge_connectors(&mut self, id: String) -> String {
        if !self.junctions.iter().any(|junction| junction.id == id) {
            return "null".to_string();
        }

        let attached = self
            .connectors
            .iter()
            .enumerate()
            .flat_map(|(index, connector)| {
                [
                    (connector.src.references_junction(&id)).then_some((index, true)),
                    (connector.dst.references_junction(&id)).then_some((index, false)),
                ]
            })
            .flatten()
            .collect::<Vec<_>>();
        if attached.len() != 2 {
            return "null".to_string();
        }

        let (conn1_index, conn1_is_src) = attached[0];
        let (conn2_index, conn2_is_src) = attached[1];
        if conn1_index == conn2_index {
            return "null".to_string();
        }

        let other_endpoint = if conn2_is_src {
            self.connectors[conn2_index].dst.clone()
        } else {
            self.connectors[conn2_index].src.clone()
        };
        if other_endpoint.references_junction(&id) {
            return "null".to_string();
        }

        if conn1_is_src {
            self.connectors[conn1_index].src = other_endpoint;
        } else {
            self.connectors[conn1_index].dst = other_endpoint;
        }
        let merged_id = self.connectors[conn1_index].id.clone();
        let deleted_id = self.connectors[conn2_index].id.clone();
        self.connectors
            .retain(|connector| connector.id != deleted_id);
        self.junctions.retain(|junction| junction.id != id);
        self.remove_pending_junction_actions(&id);
        let mut out = String::from("\"");
        push_json_string(&mut out, &merged_id);
        out.push('"');
        out
    }

    #[wasm_bindgen(js_name = moveJunction)]
    pub fn move_junction(&mut self, id: String, x: f64, y: f64) -> bool {
        if !self.junctions.iter().any(|junction| junction.id == id) {
            return false;
        }
        let position = Point { x, y };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetJunctionPosition { id, position });
        } else if let Some(junction) = self.junctions.iter_mut().find(|junction| junction.id == id)
        {
            junction.position = position;
            junction.recommended_position = position;
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = moveJunctionBy)]
    pub fn move_junction_by(&mut self, id: String, x_diff: f64, y_diff: f64) -> bool {
        if !self.junctions.iter().any(|junction| junction.id == id) {
            return false;
        }
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::TranslateJunction { id, x_diff, y_diff });
        } else if let Some(junction) = self.junctions.iter_mut().find(|junction| junction.id == id)
        {
            junction.position.x += x_diff;
            junction.position.y += y_diff;
            junction.recommended_position = junction.position;
            self.invalidate_all_connector_paths();
        }
        true
    }

    #[wasm_bindgen(js_name = junctionPosition)]
    pub fn junction_position(&self, id: String) -> String {
        let Some(junction) = self.junctions.iter().find(|junction| junction.id == id) else {
            return "null".to_string();
        };
        point_to_json(junction.position)
    }

    #[wasm_bindgen(js_name = junctionRecommendedPosition)]
    pub fn junction_recommended_position(&self, id: String) -> String {
        let Some(junction) = self.junctions.iter().find(|junction| junction.id == id) else {
            return "null".to_string();
        };
        point_to_json(junction.recommended_position)
    }

    #[wasm_bindgen(js_name = setJunctionRecommendedPosition)]
    pub fn set_junction_recommended_position(&mut self, id: String, x: f64, y: f64) -> bool {
        let Some(junction) = self.junctions.iter_mut().find(|junction| junction.id == id) else {
            return false;
        };
        junction.recommended_position = Point { x, y };
        true
    }

    #[wasm_bindgen(js_name = setJunctionPositionFixed)]
    pub fn set_junction_position_fixed(&mut self, id: String, fixed: bool) -> bool {
        let Some(index) = self.junctions.iter().position(|junction| junction.id == id) else {
            return false;
        };
        if self.junctions[index].fixed != fixed {
            self.junctions[index].fixed = fixed;
            self.invalidate_all_connector_paths();
        }
        self.settings_changed = true;
        true
    }

    #[wasm_bindgen(js_name = junctionPositionFixed)]
    pub fn junction_position_fixed(&self, id: String) -> bool {
        self.junctions
            .iter()
            .find(|junction| junction.id == id)
            .map(|junction| junction.fixed)
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = preferJunctionOrthogonalDimension)]
    pub fn prefer_junction_orthogonal_dimension(&mut self, id: String, dimension: u32) -> bool {
        if dimension > 1 {
            return false;
        }
        let Some(junction_index) = self.junctions.iter().position(|junction| junction.id == id)
        else {
            return false;
        };
        self.junctions[junction_index].preferred_orthogonal_dimension = Some(dimension as usize);
        let mut changed_pins = Vec::new();
        for pin in &mut self.junctions[junction_index].pins {
            let penalise = if dimension == 1 {
                (pin.visibility_directions & (CONN_DIR_LEFT | CONN_DIR_RIGHT)) != 0
            } else {
                (pin.visibility_directions & (CONN_DIR_UP | CONN_DIR_DOWN)) != 0
            };
            if penalise && (pin.connection_cost - 1.0).abs() >= EPS {
                pin.connection_cost = 1.0;
                changed_pins.push((pin.id.clone(), pin.class_id));
            }
        }
        if self.transaction_use {
            for (pin_id, _) in changed_pins {
                self.queue_junction_connection_pin_change(id.clone(), pin_id);
            }
        } else {
            let mut changed_classes = Vec::new();
            for (_, class_id) in changed_pins {
                if !changed_classes.contains(&class_id) {
                    changed_classes.push(class_id);
                }
            }
            for class_id in changed_classes {
                self.reselect_junction_pin_class_endpoints(&id, class_id);
            }
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = junctionPreferredOrthogonalDimension)]
    pub fn junction_preferred_orthogonal_dimension(&self, id: String) -> i32 {
        self.junctions
            .iter()
            .find(|junction| junction.id == id)
            .and_then(|junction| junction.preferred_orthogonal_dimension)
            .map(|dimension| dimension as i32)
            .unwrap_or(-1)
    }

    #[wasm_bindgen(js_name = junctionAttachedConnectors)]
    pub fn junction_attached_connectors(&self, id: String) -> String {
        if !self.junctions.iter().any(|junction| junction.id == id) {
            return "null".to_string();
        }
        ids_to_json(
            self.connectors
                .iter()
                .filter(|connector| connector.references_junction(&id))
                .map(|connector| connector.id.as_str()),
        )
    }

    #[wasm_bindgen(js_name = addShapeConnectionPin)]
    pub fn add_shape_connection_pin(
        &mut self,
        shape_id: String,
        pin_id: String,
        x: f64,
        y: f64,
    ) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == shape_id) else {
            return false;
        };
        let x_offset = x - shape.rect.left;
        let y_offset = y - shape.rect.top;
        self.add_shape_connection_pin_with_options(
            shape_id,
            pin_id,
            CONNECTION_PIN_UNSET,
            x_offset,
            y_offset,
            false,
            0.0,
            CONN_DIR_NONE,
            f64::NAN,
            false,
        )
    }

    #[wasm_bindgen(js_name = addShapeConnectionPinWithOptions)]
    pub fn add_shape_connection_pin_with_options(
        &mut self,
        shape_id: String,
        pin_id: String,
        class_id: u32,
        x_offset: f64,
        y_offset: f64,
        proportional: bool,
        inside_offset: f64,
        visibility_directions: u32,
        connection_cost: f64,
        exclusive: bool,
    ) -> bool {
        if !x_offset.is_finite()
            || !y_offset.is_finite()
            || !inside_offset.is_finite()
            || connection_cost.is_infinite()
            || !valid_shape_connection_pin_class_id(class_id)
        {
            return false;
        }
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == shape_id) else {
            return false;
        };
        let replaced_class_id = shape
            .pins
            .iter()
            .find(|pin| pin.id == pin_id)
            .map(|pin| pin.class_id);
        shape.pins.retain(|pin| pin.id != pin_id);
        let visibility_directions = pin_visibility(x_offset, y_offset, visibility_directions);
        shape.pins.push(ShapeConnectionPin {
            id: pin_id,
            class_id,
            offset: Point {
                x: if proportional { x_offset } else { x_offset },
                y: if proportional { y_offset } else { y_offset },
            },
            proportional,
            inside_offset: inside_offset.max(0.0),
            visibility_directions,
            exclusive: exclusive || visibility_directions != CONN_DIR_ALL,
            connection_cost: if connection_cost.is_finite() {
                connection_cost.max(0.0)
            } else {
                0.0
            },
        });
        if let Some(replaced_class_id) = replaced_class_id {
            if replaced_class_id != class_id {
                self.reselect_shape_pin_class_endpoints(&shape_id, replaced_class_id);
            }
        }
        self.reselect_shape_pin_class_endpoints(&shape_id, class_id);
        self.invalidate_connectors_for_shape(&shape_id);
        true
    }

    #[wasm_bindgen(js_name = removeShapeConnectionPin)]
    pub fn remove_shape_connection_pin(&mut self, shape_id: String, pin_id: String) -> bool {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == shape_id) else {
            return false;
        };
        if !shape.pins.iter().any(|pin| pin.id == pin_id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_shape_pin_actions(&shape_id, &pin_id);
            self.pending_actions
                .push(PendingAction::DeleteShapeConnectionPin { shape_id, pin_id });
            true
        } else {
            self.force_remove_shape_connection_pin(&shape_id, &pin_id)
        }
    }

    #[wasm_bindgen(js_name = setShapeConnectionPinConnectionCost)]
    pub fn set_shape_connection_pin_connection_cost(
        &mut self,
        shape_id: String,
        pin_id: String,
        cost: f64,
    ) -> bool {
        if !cost.is_finite() || cost < 0.0 {
            return false;
        }
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == shape_id) else {
            return false;
        };
        let Some(pin) = shape.pins.iter_mut().find(|pin| pin.id == pin_id) else {
            return false;
        };
        let class_id = pin.class_id;
        pin.connection_cost = cost;
        if self.transaction_use {
            self.queue_shape_connection_pin_change(shape_id, pin_id);
        } else {
            self.reselect_shape_pin_class_endpoints(&shape_id, class_id);
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = shapeConnectionPinConnectionCost)]
    pub fn shape_connection_pin_connection_cost(&self, shape_id: String, pin_id: String) -> f64 {
        self.shapes
            .iter()
            .find(|shape| shape.id == shape_id)
            .and_then(|shape| shape.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.connection_cost)
            .unwrap_or(f64::NAN)
    }

    #[wasm_bindgen(js_name = setShapeConnectionPinExclusive)]
    pub fn set_shape_connection_pin_exclusive(
        &mut self,
        shape_id: String,
        pin_id: String,
        exclusive: bool,
    ) -> bool {
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == shape_id) else {
            return false;
        };
        let Some(pin) = shape.pins.iter_mut().find(|pin| pin.id == pin_id) else {
            return false;
        };
        let class_id = pin.class_id;
        pin.exclusive = exclusive;
        if self.transaction_use {
            self.queue_shape_connection_pin_change(shape_id, pin_id);
        } else {
            self.reselect_shape_pin_class_endpoints(&shape_id, class_id);
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = shapeConnectionPinExclusive)]
    pub fn shape_connection_pin_exclusive(&self, shape_id: String, pin_id: String) -> bool {
        self.shapes
            .iter()
            .find(|shape| shape.id == shape_id)
            .and_then(|shape| shape.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.exclusive)
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = shapeConnectionPinPosition)]
    pub fn shape_connection_pin_position(&self, shape_id: String, pin_id: String) -> String {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == shape_id) else {
            return "null".to_string();
        };
        let Some(pin) = shape.pins.iter().find(|pin| pin.id == pin_id) else {
            return "null".to_string();
        };
        point_to_json(pin.resolve_position(shape.rect))
    }

    #[wasm_bindgen(js_name = shapeConnectionPinDirections)]
    pub fn shape_connection_pin_directions(&self, shape_id: String, pin_id: String) -> u32 {
        self.shapes
            .iter()
            .find(|shape| shape.id == shape_id)
            .and_then(|shape| shape.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.visibility_directions)
            .unwrap_or(CONN_DIR_NONE)
    }

    #[wasm_bindgen(js_name = shapeConnectionPinIds)]
    pub fn shape_connection_pin_ids(&self, shape_id: String, pin_id: String) -> String {
        let Some(shape) = self.shapes.iter().find(|shape| shape.id == shape_id) else {
            return "null".to_string();
        };
        let Some(pin) = shape.pins.iter().find(|pin| pin.id == pin_id) else {
            return "null".to_string();
        };
        connection_pin_ids_to_json(&shape.id, pin.class_id)
    }

    #[wasm_bindgen(js_name = addJunctionConnectionPin)]
    pub fn add_junction_connection_pin(
        &mut self,
        junction_id: String,
        pin_id: String,
        class_id: u32,
        visibility_directions: u32,
    ) -> bool {
        let Some(junction) = self
            .junctions
            .iter_mut()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        let replaced_class_id = junction
            .pins
            .iter()
            .find(|pin| pin.id == pin_id)
            .map(|pin| pin.class_id);
        junction.pins.retain(|pin| pin.id != pin_id);
        junction.pins.push(ShapeConnectionPin {
            id: pin_id,
            class_id,
            offset: Point { x: 0.0, y: 0.0 },
            proportional: false,
            inside_offset: 0.0,
            visibility_directions,
            exclusive: true,
            connection_cost: 0.0,
        });
        if let Some(replaced_class_id) = replaced_class_id {
            if replaced_class_id != class_id {
                self.reselect_junction_pin_class_endpoints(&junction_id, replaced_class_id);
            }
        }
        self.reselect_junction_pin_class_endpoints(&junction_id, class_id);
        self.invalidate_connectors_for_junction(&junction_id);
        true
    }

    #[wasm_bindgen(js_name = removeJunctionConnectionPin)]
    pub fn remove_junction_connection_pin(&mut self, junction_id: String, pin_id: String) -> bool {
        let Some(junction) = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        if !junction.pins.iter().any(|pin| pin.id == pin_id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_junction_pin_actions(&junction_id, &pin_id);
            self.pending_actions
                .push(PendingAction::DeleteJunctionConnectionPin {
                    junction_id,
                    pin_id,
                });
            true
        } else {
            self.force_remove_junction_connection_pin(&junction_id, &pin_id)
        }
    }

    #[wasm_bindgen(js_name = junctionConnectionPinPosition)]
    pub fn junction_connection_pin_position(&self, junction_id: String, pin_id: String) -> String {
        let Some(junction) = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)
        else {
            return "null".to_string();
        };
        if !junction.pins.iter().any(|pin| pin.id == pin_id) {
            return "null".to_string();
        }
        point_to_json(junction.position)
    }

    #[wasm_bindgen(js_name = junctionConnectionPinDirections)]
    pub fn junction_connection_pin_directions(&self, junction_id: String, pin_id: String) -> u32 {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)
            .and_then(|junction| junction.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.visibility_directions)
            .unwrap_or(CONN_DIR_NONE)
    }

    #[wasm_bindgen(js_name = junctionConnectionPinIds)]
    pub fn junction_connection_pin_ids(&self, junction_id: String, pin_id: String) -> String {
        let Some(junction) = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)
        else {
            return "null".to_string();
        };
        let Some(pin) = junction.pins.iter().find(|pin| pin.id == pin_id) else {
            return "null".to_string();
        };
        connection_pin_ids_to_json(&junction.id, pin.class_id)
    }

    #[wasm_bindgen(js_name = setJunctionConnectionPinConnectionCost)]
    pub fn set_junction_connection_pin_connection_cost(
        &mut self,
        junction_id: String,
        pin_id: String,
        cost: f64,
    ) -> bool {
        if !cost.is_finite() || cost < 0.0 {
            return false;
        }
        let Some(junction) = self
            .junctions
            .iter_mut()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        let Some(pin) = junction.pins.iter_mut().find(|pin| pin.id == pin_id) else {
            return false;
        };
        let class_id = pin.class_id;
        pin.connection_cost = cost;
        if self.transaction_use {
            self.queue_junction_connection_pin_change(junction_id, pin_id);
        } else {
            self.reselect_junction_pin_class_endpoints(&junction_id, class_id);
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = junctionConnectionPinConnectionCost)]
    pub fn junction_connection_pin_connection_cost(
        &self,
        junction_id: String,
        pin_id: String,
    ) -> f64 {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)
            .and_then(|junction| junction.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.connection_cost)
            .unwrap_or(f64::NAN)
    }

    #[wasm_bindgen(js_name = setJunctionConnectionPinExclusive)]
    pub fn set_junction_connection_pin_exclusive(
        &mut self,
        junction_id: String,
        pin_id: String,
        exclusive: bool,
    ) -> bool {
        let Some(junction) = self
            .junctions
            .iter_mut()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        let Some(pin) = junction.pins.iter_mut().find(|pin| pin.id == pin_id) else {
            return false;
        };
        let class_id = pin.class_id;
        pin.exclusive = exclusive;
        if self.transaction_use {
            self.queue_junction_connection_pin_change(junction_id, pin_id);
        } else {
            self.reselect_junction_pin_class_endpoints(&junction_id, class_id);
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = junctionConnectionPinExclusive)]
    pub fn junction_connection_pin_exclusive(&self, junction_id: String, pin_id: String) -> bool {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)
            .and_then(|junction| junction.pins.iter().find(|pin| pin.id == pin_id))
            .map(|pin| pin.exclusive)
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = addConnector)]
    pub fn add_connector(&mut self, id: String, src_x: f64, src_y: f64, dst_x: f64, dst_y: f64) {
        self.add_connector_with_endpoint_directions(
            id,
            src_x,
            src_y,
            CONN_DIR_ALL,
            dst_x,
            dst_y,
            CONN_DIR_ALL,
        );
    }

    #[wasm_bindgen(js_name = addConnectorWithEndpointDirections)]
    pub fn add_connector_with_endpoint_directions(
        &mut self,
        id: String,
        src_x: f64,
        src_y: f64,
        src_directions: u32,
        dst_x: f64,
        dst_y: f64,
        dst_directions: u32,
    ) {
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors.push(Connector::new(
            id,
            Endpoint::Point {
                point: Point { x: src_x, y: src_y },
                directions: src_directions,
            },
            Endpoint::Point {
                point: Point { x: dst_x, y: dst_y },
                directions: dst_directions,
            },
            CONN_TYPE_ORTHOGONAL,
        ));
        self.mark_visibility_graph_dirty();
    }

    #[wasm_bindgen(js_name = setConnectorEndpoints)]
    pub fn set_connector_endpoints(
        &mut self,
        id: String,
        src_x: f64,
        src_y: f64,
        dst_x: f64,
        dst_y: f64,
    ) -> bool {
        self.set_connector_endpoints_with_directions(
            id,
            src_x,
            src_y,
            CONN_DIR_ALL,
            dst_x,
            dst_y,
            CONN_DIR_ALL,
        )
    }

    #[wasm_bindgen(js_name = setConnectorEndpointsWithDirections)]
    pub fn set_connector_endpoints_with_directions(
        &mut self,
        id: String,
        src_x: f64,
        src_y: f64,
        src_directions: u32,
        dst_x: f64,
        dst_y: f64,
        dst_directions: u32,
    ) -> bool {
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let src = Endpoint::Point {
            point: Point { x: src_x, y: src_y },
            directions: src_directions,
        };
        let dst = Endpoint::Point {
            point: Point { x: dst_x, y: dst_y },
            directions: dst_directions,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorEndpoints { id, src, dst });
        } else {
            self.apply_connector_endpoints(&id, src, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourceEndpoint)]
    pub fn set_connector_source_endpoint(&mut self, id: String, x: f64, y: f64) -> bool {
        self.set_connector_source_endpoint_with_directions(id, x, y, CONN_DIR_ALL)
    }

    #[wasm_bindgen(js_name = setConnectorSourceEndpointWithDirections)]
    pub fn set_connector_source_endpoint_with_directions(
        &mut self,
        id: String,
        x: f64,
        y: f64,
        directions: u32,
    ) -> bool {
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let src = Endpoint::Point {
            point: Point { x, y },
            directions,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestEndpoint)]
    pub fn set_connector_dest_endpoint(&mut self, id: String, x: f64, y: f64) -> bool {
        self.set_connector_dest_endpoint_with_directions(id, x, y, CONN_DIR_ALL)
    }

    #[wasm_bindgen(js_name = setConnectorDestEndpointWithDirections)]
    pub fn set_connector_dest_endpoint_with_directions(
        &mut self,
        id: String,
        x: f64,
        y: f64,
        directions: u32,
    ) -> bool {
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let dst = Endpoint::Point {
            point: Point { x, y },
            directions,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourceJunctionEndpoint)]
    pub fn set_connector_source_junction_endpoint(
        &mut self,
        id: String,
        junction_id: String,
    ) -> bool {
        if self.find_junction_point(&junction_id).is_none() {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let src = Endpoint::Junction { junction_id };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestJunctionEndpoint)]
    pub fn set_connector_dest_junction_endpoint(
        &mut self,
        id: String,
        junction_id: String,
    ) -> bool {
        if self.find_junction_point(&junction_id).is_none() {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let dst = Endpoint::Junction { junction_id };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourcePinEndpoint)]
    pub fn set_connector_source_pin_endpoint(
        &mut self,
        id: String,
        shape_id: String,
        pin_id: String,
    ) -> bool {
        if self.find_pin_point(&shape_id, &pin_id).is_none() {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let src = Endpoint::ShapePin {
            shape_id,
            pin_id,
            pin_class_id: None,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestPinEndpoint)]
    pub fn set_connector_dest_pin_endpoint(
        &mut self,
        id: String,
        shape_id: String,
        pin_id: String,
    ) -> bool {
        if self.find_pin_point(&shape_id, &pin_id).is_none() {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let dst = Endpoint::ShapePin {
            shape_id,
            pin_id,
            pin_class_id: None,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourcePinClassEndpoint)]
    pub fn set_connector_source_pin_class_endpoint(
        &mut self,
        id: String,
        shape_id: String,
        pin_class_id: u32,
    ) -> bool {
        if !valid_shape_pin_class_id(pin_class_id) {
            return false;
        }
        let Some(connector_index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return false;
        };
        let other_endpoint = self.connectors[connector_index].dst.clone();
        let src = if let Some(pin_id) = self.find_best_pin_id_for_connector(
            &shape_id,
            pin_class_id,
            connector_index,
            Some(&other_endpoint),
        ) {
            Endpoint::ShapePin {
                shape_id,
                pin_id,
                pin_class_id: Some(pin_class_id),
            }
        } else if self.shapes.iter().any(|shape| shape.id == shape_id) {
            Endpoint::ShapePinClass {
                shape_id,
                pin_class_id,
            }
        } else {
            return false;
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestPinClassEndpoint)]
    pub fn set_connector_dest_pin_class_endpoint(
        &mut self,
        id: String,
        shape_id: String,
        pin_class_id: u32,
    ) -> bool {
        if !valid_shape_pin_class_id(pin_class_id) {
            return false;
        }
        let Some(connector_index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return false;
        };
        let other_endpoint = self.connectors[connector_index].src.clone();
        let dst = if let Some(pin_id) = self.find_best_pin_id_for_connector(
            &shape_id,
            pin_class_id,
            connector_index,
            Some(&other_endpoint),
        ) {
            Endpoint::ShapePin {
                shape_id,
                pin_id,
                pin_class_id: Some(pin_class_id),
            }
        } else if self.shapes.iter().any(|shape| shape.id == shape_id) {
            Endpoint::ShapePinClass {
                shape_id,
                pin_class_id,
            }
        } else {
            return false;
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourceJunctionPinEndpoint)]
    pub fn set_connector_source_junction_pin_endpoint(
        &mut self,
        id: String,
        junction_id: String,
        pin_id: String,
    ) -> bool {
        if self
            .find_junction_pin_point(&junction_id, &pin_id)
            .is_none()
        {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let src = Endpoint::JunctionPin {
            junction_id,
            pin_id,
            pin_class_id: None,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestJunctionPinEndpoint)]
    pub fn set_connector_dest_junction_pin_endpoint(
        &mut self,
        id: String,
        junction_id: String,
        pin_id: String,
    ) -> bool {
        if self
            .find_junction_pin_point(&junction_id, &pin_id)
            .is_none()
        {
            return false;
        }
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        };
        let dst = Endpoint::JunctionPin {
            junction_id,
            pin_id,
            pin_class_id: None,
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorSourceJunctionPinClassEndpoint)]
    pub fn set_connector_source_junction_pin_class_endpoint(
        &mut self,
        id: String,
        junction_id: String,
        pin_class_id: u32,
    ) -> bool {
        let Some(connector_index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return false;
        };
        let src = if let Some(pin_id) = self.find_best_junction_pin_id_for_connector(
            &junction_id,
            pin_class_id,
            connector_index,
        ) {
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                pin_class_id: Some(pin_class_id),
            }
        } else if self
            .junctions
            .iter()
            .any(|junction| junction.id == junction_id)
        {
            Endpoint::JunctionPinClass {
                junction_id,
                pin_class_id,
            }
        } else {
            return false;
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorSourceEndpoint { id, src });
        } else {
            self.apply_connector_source_endpoint(&id, src);
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorDestJunctionPinClassEndpoint)]
    pub fn set_connector_dest_junction_pin_class_endpoint(
        &mut self,
        id: String,
        junction_id: String,
        pin_class_id: u32,
    ) -> bool {
        let Some(connector_index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return false;
        };
        let dst = if let Some(pin_id) = self.find_best_junction_pin_id_for_connector(
            &junction_id,
            pin_class_id,
            connector_index,
        ) {
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                pin_class_id: Some(pin_class_id),
            }
        } else if self
            .junctions
            .iter()
            .any(|junction| junction.id == junction_id)
        {
            Endpoint::JunctionPinClass {
                junction_id,
                pin_class_id,
            }
        } else {
            return false;
        };
        if self.transaction_use {
            self.pending_actions
                .push(PendingAction::SetConnectorDestEndpoint { id, dst });
        } else {
            self.apply_connector_dest_endpoint(&id, dst);
        }
        true
    }

    #[wasm_bindgen(js_name = addConnectorBetweenPins)]
    pub fn add_connector_between_pins(
        &mut self,
        id: String,
        src_shape_id: String,
        src_pin_id: String,
        dst_shape_id: String,
        dst_pin_id: String,
    ) -> bool {
        if self.find_pin_point(&src_shape_id, &src_pin_id).is_none()
            || self.find_pin_point(&dst_shape_id, &dst_pin_id).is_none()
        {
            return false;
        }
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors.push(Connector::new(
            id,
            Endpoint::ShapePin {
                shape_id: src_shape_id,
                pin_id: src_pin_id,
                pin_class_id: None,
            },
            Endpoint::ShapePin {
                shape_id: dst_shape_id,
                pin_id: dst_pin_id,
                pin_class_id: None,
            },
            CONN_TYPE_ORTHOGONAL,
        ));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = addConnectorFromJunction)]
    pub fn add_connector_from_junction(
        &mut self,
        id: String,
        src_junction_id: String,
        dst_x: f64,
        dst_y: f64,
    ) -> bool {
        if self.find_junction_point(&src_junction_id).is_none() {
            return false;
        }
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors.push(Connector::new(
            id,
            Endpoint::Junction {
                junction_id: src_junction_id,
            },
            Endpoint::Point {
                point: Point { x: dst_x, y: dst_y },
                directions: CONN_DIR_ALL,
            },
            CONN_TYPE_ORTHOGONAL,
        ));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = addConnectorToJunction)]
    pub fn add_connector_to_junction(
        &mut self,
        id: String,
        src_x: f64,
        src_y: f64,
        dst_junction_id: String,
    ) -> bool {
        if self.find_junction_point(&dst_junction_id).is_none() {
            return false;
        }
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors.push(Connector::new(
            id,
            Endpoint::Point {
                point: Point { x: src_x, y: src_y },
                directions: CONN_DIR_ALL,
            },
            Endpoint::Junction {
                junction_id: dst_junction_id,
            },
            CONN_TYPE_ORTHOGONAL,
        ));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = addConnectorBetweenJunctions)]
    pub fn add_connector_between_junctions(
        &mut self,
        id: String,
        src_junction_id: String,
        dst_junction_id: String,
    ) -> bool {
        if self.find_junction_point(&src_junction_id).is_none()
            || self.find_junction_point(&dst_junction_id).is_none()
        {
            return false;
        }
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors.push(Connector::new(
            id,
            Endpoint::Junction {
                junction_id: src_junction_id,
            },
            Endpoint::Junction {
                junction_id: dst_junction_id,
            },
            CONN_TYPE_ORTHOGONAL,
        ));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = addConnectorBetweenPinClasses)]
    pub fn add_connector_between_pin_classes(
        &mut self,
        id: String,
        src_shape_id: String,
        src_pin_class_id: u32,
        dst_shape_id: String,
        dst_pin_class_id: u32,
    ) -> bool {
        if !valid_shape_pin_class_id(src_pin_class_id)
            || !valid_shape_pin_class_id(dst_pin_class_id)
        {
            return false;
        }
        let src = if let Some(src_pin_id) = self.find_best_pin_id(&src_shape_id, src_pin_class_id) {
            Endpoint::ShapePin {
                shape_id: src_shape_id,
                pin_id: src_pin_id,
                pin_class_id: Some(src_pin_class_id),
            }
        } else if self.shapes.iter().any(|shape| shape.id == src_shape_id) {
            Endpoint::ShapePinClass {
                shape_id: src_shape_id,
                pin_class_id: src_pin_class_id,
            }
        } else {
            return false;
        };
        let dst = if let Some(dst_pin_id) = self.find_best_pin_id(&dst_shape_id, dst_pin_class_id) {
            Endpoint::ShapePin {
                shape_id: dst_shape_id,
                pin_id: dst_pin_id,
                pin_class_id: Some(dst_pin_class_id),
            }
        } else if self.shapes.iter().any(|shape| shape.id == dst_shape_id) {
            Endpoint::ShapePinClass {
                shape_id: dst_shape_id,
                pin_class_id: dst_pin_class_id,
            }
        } else {
            return false;
        };
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors
            .push(Connector::new(id, src, dst, CONN_TYPE_ORTHOGONAL));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = addConnectorBetweenJunctionPinClasses)]
    pub fn add_connector_between_junction_pin_classes(
        &mut self,
        id: String,
        src_junction_id: String,
        src_pin_class_id: u32,
        dst_junction_id: String,
        dst_pin_class_id: u32,
    ) -> bool {
        let src = if let Some(src_pin_id) =
            self.find_best_junction_pin_id(&src_junction_id, src_pin_class_id)
        {
            Endpoint::JunctionPin {
                junction_id: src_junction_id,
                pin_id: src_pin_id,
                pin_class_id: Some(src_pin_class_id),
            }
        } else if self
            .junctions
            .iter()
            .any(|junction| junction.id == src_junction_id)
        {
            Endpoint::JunctionPinClass {
                junction_id: src_junction_id,
                pin_class_id: src_pin_class_id,
            }
        } else {
            return false;
        };
        let dst = if let Some(dst_pin_id) =
            self.find_best_junction_pin_id(&dst_junction_id, dst_pin_class_id)
        {
            Endpoint::JunctionPin {
                junction_id: dst_junction_id,
                pin_id: dst_pin_id,
                pin_class_id: Some(dst_pin_class_id),
            }
        } else if self
            .junctions
            .iter()
            .any(|junction| junction.id == dst_junction_id)
        {
            Endpoint::JunctionPinClass {
                junction_id: dst_junction_id,
                pin_class_id: dst_pin_class_id,
            }
        } else {
            return false;
        };
        self.force_remove_connector(&id);
        self.remove_pending_connector_actions(&id);
        self.connectors
            .push(Connector::new(id, src, dst, CONN_TYPE_ORTHOGONAL));
        self.mark_visibility_graph_dirty();
        true
    }

    #[wasm_bindgen(js_name = setConnectorRoutingType)]
    pub fn set_connector_routing_type(&mut self, id: String, routing_type: u32) -> bool {
        if !matches!(
            routing_type,
            CONN_TYPE_NONE | CONN_TYPE_POLY_LINE | CONN_TYPE_ORTHOGONAL
        ) {
            return false;
        }
        let routing_type = self.valid_conn_type(routing_type);
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        if connector.routing_type == routing_type {
            return true;
        }
        connector.routing_type = routing_type;
        connector.make_path_invalid();
        if !self.transaction_use {
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = setConnectorHateCrossings)]
    pub fn set_connector_hate_crossings(&mut self, id: String, value: bool) -> bool {
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        connector.hate_crossings = value;
        connector.make_path_invalid();
        if !self.transaction_use {
            let _ = self.refresh_invalidated_routes();
        }
        true
    }

    #[wasm_bindgen(js_name = connectorDoesHateCrossings)]
    pub fn connector_does_hate_crossings(&self, id: String) -> bool {
        self.connectors
            .iter()
            .find(|connector| connector.id == id)
            .map(|connector| connector.hate_crossings)
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = connectorRoutingType)]
    pub fn connector_routing_type(&self, id: String) -> u32 {
        self.connectors
            .iter()
            .find(|connector| connector.id == id)
            .map(|connector| connector.routing_type)
            .unwrap_or(CONN_TYPE_NONE)
    }

    #[wasm_bindgen(js_name = connectorEndpointConnEnds)]
    pub fn connector_endpoint_conn_ends(&self, id: String) -> String {
        let Some(connector) = self.connectors.iter().find(|connector| connector.id == id) else {
            return "null".to_string();
        };
        let mut out = String::from("{\"source\":");
        self.endpoint_to_json(&connector.src, &mut out);
        out.push_str(",\"destination\":");
        self.endpoint_to_json(&connector.dst, &mut out);
        out.push('}');
        out
    }

    #[wasm_bindgen(js_name = connectorPossibleDstPinPoints)]
    pub fn connector_possible_dst_pin_points(&self, id: String) -> String {
        let Some(connector) = self.connectors.iter().find(|connector| connector.id == id) else {
            return "null".to_string();
        };
        match &connector.dst {
            Endpoint::ShapePin {
                shape_id,
                pin_id,
                pin_class_id,
            } => pin_class_id
                .map(|class_id| points_to_json(&self.find_pin_class_points(shape_id, class_id)))
                .or_else(|| {
                    self.find_pin_point(shape_id, pin_id)
                        .map(|point| points_to_json(&[point]))
                })
                .unwrap_or_else(|| "[]".to_string()),
            Endpoint::ShapePinClass {
                shape_id,
                pin_class_id,
            } => points_to_json(&self.find_pin_class_points(shape_id, *pin_class_id)),
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                pin_class_id,
            } => pin_class_id
                .map(|class_id| {
                    points_to_json(&self.find_junction_pin_class_points(junction_id, class_id))
                })
                .or_else(|| {
                    self.find_junction_pin_point(junction_id, pin_id)
                        .map(|point| points_to_json(&[point]))
                })
                .unwrap_or_else(|| "[]".to_string()),
            Endpoint::JunctionPinClass {
                junction_id,
                pin_class_id,
            } => points_to_json(&self.find_junction_pin_class_points(junction_id, *pin_class_id)),
            Endpoint::Junction { junction_id } => points_to_json(
                &self.find_junction_pin_class_points(junction_id, CONNECTION_PIN_CENTRE),
            ),
            Endpoint::Point { .. } => "[]".to_string(),
        }
    }

    #[wasm_bindgen(js_name = setConnectorRoutingCheckpoints)]
    pub fn set_connector_routing_checkpoints(&mut self, id: String, checkpoints: Vec<f64>) -> bool {
        let Some(parsed) = checkpoints_from_flat_coordinates(checkpoints) else {
            return false;
        };
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        connector.checkpoints = parsed;
        connector.make_path_invalid();
        true
    }

    #[wasm_bindgen(js_name = connectorRoutingCheckpoints)]
    pub fn connector_routing_checkpoints(&self, id: String) -> String {
        let Some(connector) = self.connectors.iter().find(|connector| connector.id == id) else {
            return "null".to_string();
        };
        checkpoints_to_json(&connector.checkpoints)
    }

    #[wasm_bindgen(js_name = setConnectorFixedRoute)]
    pub fn set_connector_fixed_route(&mut self, id: String, route: Vec<f64>) -> bool {
        let Some(points) = points_from_flat_coordinates(route) else {
            return false;
        };
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        if points.len() >= 2 {
            connector.src = Endpoint::Point {
                point: points[0],
                directions: CONN_DIR_ALL,
            };
            connector.dst = Endpoint::Point {
                point: *points.last().unwrap(),
                directions: CONN_DIR_ALL,
            };
        }
        connector.fixed_route = Some(points.clone());
        connector.set_display_route(points);
        self.settings_changed = true;
        true
    }

    #[wasm_bindgen(js_name = setConnectorFixedExistingRoute)]
    pub fn set_connector_fixed_existing_route(&mut self, id: String) -> bool {
        let Some(index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return false;
        };
        let route = self.route(&self.connectors[index]);
        if route.len() < 2 {
            return false;
        }
        self.connectors[index].fixed_route = Some(route.clone());
        self.connectors[index].set_display_route(route);
        self.settings_changed = true;
        true
    }

    #[wasm_bindgen(js_name = connectorHasFixedRoute)]
    pub fn connector_has_fixed_route(&self, id: String) -> bool {
        self.connectors
            .iter()
            .find(|connector| connector.id == id)
            .and_then(|connector| connector.fixed_route.as_ref())
            .is_some()
    }

    #[wasm_bindgen(js_name = clearConnectorFixedRoute)]
    pub fn clear_connector_fixed_route(&mut self, id: String) -> bool {
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        let had_fixed_route = connector.fixed_route.is_some();
        connector.fixed_route = None;
        if had_fixed_route {
            connector.make_path_invalid();
            self.settings_changed = true;
        }
        had_fixed_route
    }

    #[wasm_bindgen(js_name = makeConnectorPathInvalid)]
    pub fn make_connector_path_invalid(&mut self, id: String) -> bool {
        let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        else {
            return false;
        };
        connector.make_path_invalid();
        true
    }

    #[wasm_bindgen(js_name = connectorNeedsRepaint)]
    pub fn connector_needs_repaint(&self, id: String) -> bool {
        self.connectors
            .iter()
            .find(|connector| connector.id == id)
            .map(|connector| connector.needs_repaint)
            .unwrap_or(false)
    }

    #[wasm_bindgen(js_name = existsInvalidOrthogonalPaths)]
    pub fn exists_invalid_orthogonal_paths(&self) -> bool {
        self.connectors.iter().any(|connector| {
            connector.routing_type == CONN_TYPE_ORTHOGONAL
                && self
                    .current_display_route_for_diagnostics(connector)
                    .windows(2)
                    .any(|segment| {
                        (segment[0].x - segment[1].x).abs() >= EPS
                            && (segment[0].y - segment[1].y).abs() >= EPS
                    })
        })
    }

    #[wasm_bindgen(js_name = existsCrossings)]
    pub fn exists_crossings(&self, _optimised_for_connector_type: bool) -> u32 {
        let routes = self
            .connectors
            .iter()
            .map(|connector| self.current_display_route_for_diagnostics(connector))
            .collect::<Vec<_>>();
        let mut crossings = 0;
        for i in 0..routes.len() {
            for j in (i + 1)..routes.len() {
                for segment_a in routes[i].windows(2) {
                    for segment_b in routes[j].windows(2) {
                        if segment_intersect(segment_a[0], segment_a[1], segment_b[0], segment_b[1])
                        {
                            crossings += 1;
                        }
                    }
                }
            }
        }
        crossings
    }

    #[wasm_bindgen(js_name = existsOrthogonalSegmentOverlap)]
    pub fn exists_orthogonal_segment_overlap(&self, at_ends: bool) -> bool {
        let routes = self
            .connectors
            .iter()
            .map(|connector| self.current_display_route_for_diagnostics(connector))
            .collect::<Vec<_>>();
        for i in 0..routes.len() {
            for j in (i + 1)..routes.len() {
                if routes_share_orthogonal_segment(&routes[i], &routes[j], at_ends) {
                    return true;
                }
            }
        }
        false
    }

    #[wasm_bindgen(js_name = existsOrthogonalFixedSegmentOverlap)]
    pub fn exists_orthogonal_fixed_segment_overlap(&self, at_ends: bool) -> bool {
        let routes = self
            .connectors
            .iter()
            .map(|connector| self.current_display_route_for_diagnostics(connector))
            .collect::<Vec<_>>();
        for i in 0..routes.len() {
            for j in (i + 1)..routes.len() {
                let includes_fixed_route = self.connectors[i].fixed_route.is_some()
                    || self.connectors[j].fixed_route.is_some();
                if includes_fixed_route
                    && routes_share_orthogonal_segment(&routes[i], &routes[j], at_ends)
                {
                    return true;
                }
            }
        }
        false
    }

    #[wasm_bindgen(js_name = existsOrthogonalTouchingPaths)]
    pub fn exists_orthogonal_touching_paths(&self) -> bool {
        let routes = self
            .connectors
            .iter()
            .map(|connector| self.current_display_route_for_diagnostics(connector))
            .collect::<Vec<_>>();
        for i in 0..routes.len() {
            for j in (i + 1)..routes.len() {
                if routes_have_orthogonal_touch(&routes[i], &routes[j]) {
                    return true;
                }
            }
        }
        false
    }

    #[wasm_bindgen(js_name = removeConnector)]
    pub fn remove_connector(&mut self, id: String) -> bool {
        if !self.connectors.iter().any(|connector| connector.id == id) {
            return false;
        }
        if self.transaction_use {
            self.remove_pending_connector_actions(&id);
            self.pending_actions
                .push(PendingAction::DeleteConnector { id });
            true
        } else {
            self.force_remove_connector(&id)
        }
    }

    #[wasm_bindgen(js_name = routeConnector)]
    pub fn route_connector(&self, id: String) -> String {
        let Some(connector) = self.connectors.iter().find(|connector| connector.id == id) else {
            return "null".to_string();
        };
        points_to_json(&self.route(connector))
    }

    #[wasm_bindgen(js_name = displayConnectorRoute)]
    pub fn display_connector_route(&mut self, id: String) -> String {
        let Some(index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return "null".to_string();
        };
        if self.connectors[index].needs_reroute || self.connectors[index].display_route.is_empty() {
            let route = self.route(&self.connectors[index]);
            self.connectors[index].set_display_route(route);
        }
        self.connectors[index].needs_repaint = false;
        points_to_json(&self.connectors[index].display_route)
    }

    #[wasm_bindgen(js_name = splitConnectorAtSegment)]
    pub fn split_connector_at_segment(
        &mut self,
        id: String,
        segment_n: u32,
        junction_id: String,
        connector_id: String,
    ) -> String {
        if segment_n == 0
            || !self.object_id_is_unused(&junction_id)
            || !self.object_id_is_unused(&connector_id)
        {
            return "null".to_string();
        }
        let Some(index) = self
            .connectors
            .iter()
            .position(|connector| connector.id == id)
        else {
            return "null".to_string();
        };
        if self.connectors[index].display_route.is_empty() || self.connectors[index].needs_reroute {
            let route = self.route(&self.connectors[index]);
            self.connectors[index].set_display_route(route);
        }

        let segment_n = segment_n as usize;
        let route = self.connectors[index].display_route.clone();
        if route.len() <= segment_n {
            return "null".to_string();
        }
        let before = route[segment_n - 1];
        let after = route[segment_n];
        let junction_position = Point {
            x: (before.x + after.x) / 2.0,
            y: (before.y + after.y) / 2.0,
        };
        let preferred_orthogonal_dimension = if (before.x - after.x).abs() < EPS {
            Some(1)
        } else {
            Some(0)
        };
        let old_dst = self.connectors[index].dst.clone();
        let routing_type = self.connectors[index].routing_type;

        self.junctions.push(Junction {
            id: junction_id.clone(),
            position: junction_position,
            recommended_position: junction_position,
            fixed: false,
            preferred_orthogonal_dimension,
            pins: default_junction_pins(),
        });
        self.connectors[index].dst = Endpoint::Junction {
            junction_id: junction_id.clone(),
        };
        self.connectors[index].fixed_route = None;
        self.connectors[index].make_path_invalid();
        self.connectors.push(Connector::new(
            connector_id.clone(),
            Endpoint::Junction {
                junction_id: junction_id.clone(),
            },
            old_dst,
            routing_type,
        ));
        self.mark_visibility_graph_dirty();

        let mut out = String::from("{\"junctionId\":\"");
        push_json_string(&mut out, &junction_id);
        out.push_str("\",\"connectorId\":\"");
        push_json_string(&mut out, &connector_id);
        out.push_str("\",\"position\":");
        out.push_str(&point_to_json(junction_position));
        out.push('}');
        out
    }

    #[wasm_bindgen(js_name = routeAll)]
    pub fn route_all(&self) -> String {
        let mut out = String::from("{");
        let mut computed_routes: Vec<(&str, Vec<Point>)> = Vec::new();
        for (index, connector) in self.connectors.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push('"');
            push_json_string(&mut out, &connector.id);
            out.push_str("\":");
            let route = self.route_with_penalty_overrides(connector, &computed_routes);
            out.push_str(&points_to_json(&route));
            computed_routes.push((&connector.id, route));
        }
        out.push('}');
        out
    }
}

fn polygon_from_flat_coordinates(coordinates: Vec<f64>) -> Option<Polygon> {
    if coordinates.len() < 6 || coordinates.len() % 2 != 0 {
        return None;
    }
    let points = coordinates
        .chunks_exact(2)
        .map(|chunk| Point {
            x: chunk[0],
            y: chunk[1],
        })
        .collect::<Vec<_>>();
    Polygon::new(points)
}

fn points_from_flat_coordinates(coordinates: Vec<f64>) -> Option<Vec<Point>> {
    if coordinates.len() % 2 != 0 {
        return None;
    }
    coordinates
        .chunks_exact(2)
        .map(|chunk| {
            if !chunk[0].is_finite() || !chunk[1].is_finite() {
                return None;
            }
            Some(Point {
                x: chunk[0],
                y: chunk[1],
            })
        })
        .collect()
}

fn checkpoints_from_flat_coordinates(coordinates: Vec<f64>) -> Option<Vec<Checkpoint>> {
    if coordinates.len() % 4 != 0 {
        return None;
    }
    coordinates
        .chunks_exact(4)
        .map(|chunk| {
            if !chunk.iter().all(|value| value.is_finite()) {
                return None;
            }
            Some(Checkpoint {
                point: Point {
                    x: chunk[0],
                    y: chunk[1],
                },
                arrival_directions: chunk[2] as u32,
                departure_directions: chunk[3] as u32,
            })
        })
        .collect()
}

fn polygon_to_json(polygon: &Polygon) -> String {
    let mut out = String::from("[");
    for (index, point) in polygon.points.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("{\"x\":");
        out.push_str(&point.x.to_string());
        out.push_str(",\"y\":");
        out.push_str(&point.y.to_string());
        out.push('}');
    }
    out.push(']');
    out
}

fn point_to_json(point: Point) -> String {
    format!(
        "{{\"x\":{},\"y\":{}}}",
        number_to_json(point.x),
        number_to_json(point.y)
    )
}

fn connection_pin_ids_to_json(object_id: &str, class_id: u32) -> String {
    let mut out = String::from("{\"objectId\":\"");
    push_json_string(&mut out, object_id);
    out.push_str("\",\"classId\":");
    out.push_str(&class_id.to_string());
    out.push('}');
    out
}

fn default_junction_pins() -> Vec<ShapeConnectionPin> {
    vec![ShapeConnectionPin {
        id: "centre".to_string(),
        class_id: CONNECTION_PIN_CENTRE,
        offset: Point { x: 0.0, y: 0.0 },
        proportional: false,
        inside_offset: 0.0,
        visibility_directions: CONN_DIR_ALL,
        exclusive: false,
        connection_cost: 0.0,
    }]
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    push_json_string(&mut out, value);
    out.push('"');
    out
}

fn ids_to_json<'a>(ids: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::from("[");
    for (index, id) in ids.enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        push_json_string(&mut out, id);
        out.push('"');
    }
    out.push(']');
    out
}

fn push_unique_id<'a>(ids: &mut Vec<&'a str>, id: &'a str) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

fn checkpoints_to_json(checkpoints: &[Checkpoint]) -> String {
    let mut out = String::from("[");
    for (index, checkpoint) in checkpoints.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("{\"point\":{\"x\":");
        out.push_str(&number_to_json(checkpoint.point.x));
        out.push_str(",\"y\":");
        out.push_str(&number_to_json(checkpoint.point.y));
        out.push_str("},\"arrivalDirections\":");
        out.push_str(&checkpoint.arrival_directions.to_string());
        out.push_str(",\"departureDirections\":");
        out.push_str(&checkpoint.departure_directions.to_string());
        out.push('}');
    }
    out.push(']');
    out
}

fn routes_share_orthogonal_segment(route_a: &[Point], route_b: &[Point], at_ends: bool) -> bool {
    for (index_a, segment_a) in route_a.windows(2).enumerate() {
        for (index_b, segment_b) in route_b.windows(2).enumerate() {
            if orthogonal_segments_overlap(segment_a[0], segment_a[1], segment_b[0], segment_b[1])
                && (at_ends
                    || !segment_is_route_end(index_a, route_a.len())
                    || !segment_is_route_end(index_b, route_b.len()))
            {
                return true;
            }
        }
    }
    false
}

fn routes_have_orthogonal_touch(route_a: &[Point], route_b: &[Point]) -> bool {
    for segment_a in route_a.windows(2) {
        for segment_b in route_b.windows(2) {
            if orthogonal_segments_overlap(segment_a[0], segment_a[1], segment_b[0], segment_b[1])
                || segments_touch_or_cross(segment_a[0], segment_a[1], segment_b[0], segment_b[1])
            {
                return true;
            }
        }
    }
    false
}

fn segment_is_route_end(segment_index: usize, route_len: usize) -> bool {
    segment_index == 0 || segment_index + 2 == route_len
}

fn orthogonal_segments_overlap(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    if (a1.x - a2.x).abs() < EPS && (b1.x - b2.x).abs() < EPS && (a1.x - b1.x).abs() < EPS {
        return ranges_overlap_strict(a1.y, a2.y, b1.y, b2.y);
    }
    if (a1.y - a2.y).abs() < EPS && (b1.y - b2.y).abs() < EPS && (a1.y - b1.y).abs() < EPS {
        return ranges_overlap_strict(a1.x, a2.x, b1.x, b2.x);
    }
    false
}

fn segments_touch_or_cross(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    segment_intersect(a1, a2, b1, b2)
        || point_on_segment(a1, b1, b2)
        || point_on_segment(a2, b1, b2)
        || point_on_segment(b1, a1, a2)
        || point_on_segment(b2, a1, a2)
}

fn point_on_segment(point: Point, segment_start: Point, segment_end: Point) -> bool {
    let cross = (segment_end.x - segment_start.x) * (point.y - segment_start.y)
        - (segment_end.y - segment_start.y) * (point.x - segment_start.x);
    if cross.abs() >= EPS {
        return false;
    }
    point.x + EPS >= segment_start.x.min(segment_end.x)
        && point.x <= segment_start.x.max(segment_end.x) + EPS
        && point.y + EPS >= segment_start.y.min(segment_end.y)
        && point.y <= segment_start.y.max(segment_end.y) + EPS
}

fn ranges_overlap_strict(a1: f64, a2: f64, b1: f64, b2: f64) -> bool {
    let a_min = a1.min(a2);
    let a_max = a1.max(a2);
    let b_min = b1.min(b2);
    let b_max = b1.max(b2);
    a_min.max(b_min) + EPS < a_max.min(b_max)
}

impl AvoidRouter {
    fn apply_pending_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::DeleteShape { id } => {
                self.force_remove_shape(&id);
            }
            PendingAction::DeleteShapeConnectionPin { shape_id, pin_id } => {
                self.force_remove_shape_connection_pin(&shape_id, &pin_id);
            }
            PendingAction::ChangeShapeConnectionPin { shape_id, pin_id } => {
                if let Some(class_id) = self.shape_pin_class_id(&shape_id, &pin_id) {
                    self.reselect_shape_pin_class_endpoints(&shape_id, class_id);
                } else {
                    self.invalidate_connectors_for_shape_pin(&shape_id, &pin_id);
                }
            }
            PendingAction::SetShapeRect { id, rect } => {
                if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
                    shape.set_rect(rect);
                    self.invalidate_all_connector_paths();
                }
            }
            PendingAction::SetShapePolygon { id, polygon } => {
                if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
                    if shape.set_polygon(polygon) {
                        self.invalidate_all_connector_paths();
                    }
                }
            }
            PendingAction::TranslateShape { id, x_diff, y_diff } => {
                if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == id) {
                    shape.translate(x_diff, y_diff);
                    self.invalidate_all_connector_paths();
                }
            }
            PendingAction::SetJunctionPosition { id, position } => {
                if let Some(junction) = self.junctions.iter_mut().find(|junction| junction.id == id)
                {
                    junction.position = position;
                    junction.recommended_position = position;
                    self.invalidate_all_connector_paths();
                }
            }
            PendingAction::TranslateJunction { id, x_diff, y_diff } => {
                if let Some(junction) = self.junctions.iter_mut().find(|junction| junction.id == id)
                {
                    junction.position.x += x_diff;
                    junction.position.y += y_diff;
                    junction.recommended_position = junction.position;
                    self.invalidate_all_connector_paths();
                }
            }
            PendingAction::DeleteJunction { id } => {
                self.force_remove_junction(&id);
            }
            PendingAction::DeleteJunctionConnectionPin {
                junction_id,
                pin_id,
            } => {
                self.force_remove_junction_connection_pin(&junction_id, &pin_id);
            }
            PendingAction::ChangeJunctionConnectionPin {
                junction_id,
                pin_id,
            } => {
                if let Some(class_id) = self.junction_pin_class_id(&junction_id, &pin_id) {
                    self.reselect_junction_pin_class_endpoints(&junction_id, class_id);
                } else {
                    self.invalidate_connectors_for_junction_pin(&junction_id, &pin_id);
                }
            }
            PendingAction::SetClusterPolygon { id, polygon } => {
                if let Some(cluster) = self.clusters.iter_mut().find(|cluster| cluster.id == id) {
                    if cluster.set_polygon(polygon) {
                        self.invalidate_all_connector_paths();
                    }
                }
            }
            PendingAction::DeleteCluster { id } => {
                self.force_remove_cluster(&id);
            }
            PendingAction::DeleteConnector { id } => {
                self.force_remove_connector(&id);
            }
            PendingAction::SetConnectorEndpoints { id, src, dst } => {
                self.apply_connector_endpoints(&id, src, dst);
            }
            PendingAction::SetConnectorSourceEndpoint { id, src } => {
                self.apply_connector_source_endpoint(&id, src);
            }
            PendingAction::SetConnectorDestEndpoint { id, dst } => {
                self.apply_connector_dest_endpoint(&id, dst);
            }
        }
    }

    fn apply_connector_endpoints(&mut self, id: &str, src: Endpoint, dst: Endpoint) {
        if let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        {
            connector.src = src;
            connector.dst = dst;
            connector.make_path_invalid();
            self.mark_visibility_graph_dirty();
        }
    }

    fn apply_connector_source_endpoint(&mut self, id: &str, src: Endpoint) {
        if let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        {
            connector.src = src;
            connector.make_path_invalid();
            self.mark_visibility_graph_dirty();
        }
    }

    fn apply_connector_dest_endpoint(&mut self, id: &str, dst: Endpoint) {
        if let Some(connector) = self
            .connectors
            .iter_mut()
            .find(|connector| connector.id == id)
        {
            connector.dst = dst;
            connector.make_path_invalid();
            self.mark_visibility_graph_dirty();
        }
    }

    fn force_remove_shape(&mut self, id: &str) -> bool {
        let before = self.shapes.len();
        self.detach_connector_ends_from_shape(id);
        self.shapes.retain(|shape| shape.id != id);
        let removed = self.shapes.len() != before;
        if removed {
            self.invalidate_all_connector_paths();
        }
        removed
    }

    fn force_remove_shape_connection_pin(&mut self, shape_id: &str, pin_id: &str) -> bool {
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == shape_id) else {
            return false;
        };
        let before = shape.pins.len();
        if before == 0 || !shape.pins.iter().any(|pin| pin.id == pin_id) {
            return false;
        }
        self.reassign_class_shape_pin_endpoints(shape_id, pin_id);
        let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id == shape_id) else {
            return false;
        };
        shape.pins.retain(|pin| pin.id != pin_id);
        let removed = shape.pins.len() != before;
        if removed {
            self.connectors
                .retain(|connector| !connector.references_pin(shape_id, pin_id));
            self.invalidate_all_connector_paths();
        }
        removed
    }

    fn force_remove_junction(&mut self, id: &str) -> bool {
        let before = self.junctions.len();
        self.detach_connector_ends_from_junction(id);
        self.junctions.retain(|junction| junction.id != id);
        let removed = self.junctions.len() != before;
        if removed {
            self.invalidate_all_connector_paths();
        }
        removed
    }

    fn force_remove_junction_connection_pin(&mut self, junction_id: &str, pin_id: &str) -> bool {
        let Some(junction) = self
            .junctions
            .iter_mut()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        let before = junction.pins.len();
        if before == 0 || !junction.pins.iter().any(|pin| pin.id == pin_id) {
            return false;
        }
        self.reassign_class_junction_pin_endpoints(junction_id, pin_id);
        let Some(junction) = self
            .junctions
            .iter_mut()
            .find(|junction| junction.id == junction_id)
        else {
            return false;
        };
        junction.pins.retain(|pin| pin.id != pin_id);
        let removed = junction.pins.len() != before;
        if removed {
            self.connectors
                .retain(|connector| !connector.references_junction_pin(junction_id, pin_id));
            self.invalidate_all_connector_paths();
        }
        removed
    }

    fn detach_connector_ends_from_shape(&mut self, shape_id: &str) {
        let replacements = self
            .connectors
            .iter()
            .enumerate()
            .filter_map(|(index, connector)| {
                let src = self.detached_shape_endpoint(&connector.src, shape_id);
                let dst = self.detached_shape_endpoint(&connector.dst, shape_id);
                if src.is_some() || dst.is_some() {
                    Some((index, src, dst))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for (index, src, dst) in replacements {
            if let Some(endpoint) = src {
                self.connectors[index].src = endpoint;
            }
            if let Some(endpoint) = dst {
                self.connectors[index].dst = endpoint;
            }
            self.connectors[index].make_path_invalid();
        }
    }

    fn detached_shape_endpoint(&self, endpoint: &Endpoint, shape_id: &str) -> Option<Endpoint> {
        match endpoint {
            Endpoint::ShapePin {
                shape_id: endpoint_shape_id,
                ..
            }
            | Endpoint::ShapePinClass {
                shape_id: endpoint_shape_id,
                ..
            } if endpoint_shape_id == shape_id => {
                self.resolve_endpoint(endpoint)
                    .map(|point| Endpoint::Point {
                        point,
                        directions: CONN_DIR_ALL,
                    })
            }
            Endpoint::Point { .. }
            | Endpoint::ShapePin { .. }
            | Endpoint::ShapePinClass { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPin { .. }
            | Endpoint::JunctionPinClass { .. } => None,
        }
    }

    fn detach_connector_ends_from_junction(&mut self, junction_id: &str) {
        let replacements = self
            .connectors
            .iter()
            .enumerate()
            .filter_map(|(index, connector)| {
                let src = self.detached_junction_endpoint(&connector.src, junction_id);
                let dst = self.detached_junction_endpoint(&connector.dst, junction_id);
                if src.is_some() || dst.is_some() {
                    Some((index, src, dst))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for (index, src, dst) in replacements {
            if let Some(endpoint) = src {
                self.connectors[index].src = endpoint;
            }
            if let Some(endpoint) = dst {
                self.connectors[index].dst = endpoint;
            }
            self.connectors[index].make_path_invalid();
        }
    }

    fn detached_junction_endpoint(
        &self,
        endpoint: &Endpoint,
        junction_id: &str,
    ) -> Option<Endpoint> {
        match endpoint {
            Endpoint::Junction {
                junction_id: endpoint_junction_id,
            }
            | Endpoint::JunctionPin {
                junction_id: endpoint_junction_id,
                ..
            }
            | Endpoint::JunctionPinClass {
                junction_id: endpoint_junction_id,
                ..
            } if endpoint_junction_id == junction_id => {
                self.resolve_endpoint(endpoint)
                    .map(|point| Endpoint::Point {
                        point,
                        directions: CONN_DIR_ALL,
                    })
            }
            Endpoint::Point { .. }
            | Endpoint::ShapePin { .. }
            | Endpoint::ShapePinClass { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPin { .. }
            | Endpoint::JunctionPinClass { .. } => None,
        }
    }

    fn reassign_class_shape_pin_endpoints(&mut self, shape_id: &str, pin_id: &str) {
        for index in 0..self.connectors.len() {
            let src_action =
                self.shape_pin_endpoint_replacement(&self.connectors[index].src, shape_id, pin_id);
            let dst_action =
                self.shape_pin_endpoint_replacement(&self.connectors[index].dst, shape_id, pin_id);
            if matches!(src_action, EndpointReplacement::RemoveConnector)
                || matches!(dst_action, EndpointReplacement::RemoveConnector)
            {
                continue;
            }
            let mut changed = false;
            if let EndpointReplacement::Replace(endpoint) = src_action {
                self.connectors[index].src = endpoint;
                changed = true;
            }
            if let EndpointReplacement::Replace(endpoint) = dst_action {
                self.connectors[index].dst = endpoint;
                changed = true;
            }
            if changed {
                self.connectors[index].make_path_invalid();
            }
        }
    }

    fn shape_pin_endpoint_replacement(
        &self,
        endpoint: &Endpoint,
        shape_id: &str,
        pin_id: &str,
    ) -> EndpointReplacement {
        let Endpoint::ShapePin {
            shape_id: endpoint_shape_id,
            pin_id: endpoint_pin_id,
            pin_class_id,
        } = endpoint
        else {
            return EndpointReplacement::Unchanged;
        };
        if endpoint_shape_id != shape_id || endpoint_pin_id != pin_id {
            return EndpointReplacement::Unchanged;
        }
        let Some(class_id) = pin_class_id else {
            return EndpointReplacement::RemoveConnector;
        };
        EndpointReplacement::Replace(
            self.find_best_pin_id_except(shape_id, *class_id, pin_id)
                .map(|replacement_pin_id| Endpoint::ShapePin {
                    shape_id: shape_id.to_string(),
                    pin_id: replacement_pin_id,
                    pin_class_id: Some(*class_id),
                })
                .unwrap_or(Endpoint::ShapePinClass {
                    shape_id: shape_id.to_string(),
                    pin_class_id: *class_id,
                }),
        )
    }

    fn reassign_class_junction_pin_endpoints(&mut self, junction_id: &str, pin_id: &str) {
        for index in 0..self.connectors.len() {
            let src_action = self.junction_pin_endpoint_replacement(
                &self.connectors[index].src,
                junction_id,
                pin_id,
            );
            let dst_action = self.junction_pin_endpoint_replacement(
                &self.connectors[index].dst,
                junction_id,
                pin_id,
            );
            if matches!(src_action, EndpointReplacement::RemoveConnector)
                || matches!(dst_action, EndpointReplacement::RemoveConnector)
            {
                continue;
            }
            let mut changed = false;
            if let EndpointReplacement::Replace(endpoint) = src_action {
                self.connectors[index].src = endpoint;
                changed = true;
            }
            if let EndpointReplacement::Replace(endpoint) = dst_action {
                self.connectors[index].dst = endpoint;
                changed = true;
            }
            if changed {
                self.connectors[index].make_path_invalid();
            }
        }
    }

    fn junction_pin_endpoint_replacement(
        &self,
        endpoint: &Endpoint,
        junction_id: &str,
        pin_id: &str,
    ) -> EndpointReplacement {
        let Endpoint::JunctionPin {
            junction_id: endpoint_junction_id,
            pin_id: endpoint_pin_id,
            pin_class_id,
        } = endpoint
        else {
            return EndpointReplacement::Unchanged;
        };
        if endpoint_junction_id != junction_id || endpoint_pin_id != pin_id {
            return EndpointReplacement::Unchanged;
        }
        let Some(class_id) = pin_class_id else {
            return EndpointReplacement::RemoveConnector;
        };
        EndpointReplacement::Replace(
            self.find_best_junction_pin_id_except(junction_id, *class_id, pin_id)
                .map(|replacement_pin_id| Endpoint::JunctionPin {
                    junction_id: junction_id.to_string(),
                    pin_id: replacement_pin_id,
                    pin_class_id: Some(*class_id),
                })
                .unwrap_or(Endpoint::JunctionPinClass {
                    junction_id: junction_id.to_string(),
                    pin_class_id: *class_id,
                }),
        )
    }

    fn reselect_shape_pin_class_endpoints(&mut self, shape_id: &str, class_id: u32) {
        for index in 0..self.connectors.len() {
            let src_matches =
                endpoint_is_shape_pin_class(&self.connectors[index].src, shape_id, class_id);
            let dst_matches =
                endpoint_is_shape_pin_class(&self.connectors[index].dst, shape_id, class_id);
            if !src_matches && !dst_matches {
                continue;
            }
            if src_matches {
                let other_endpoint = self.connectors[index].dst.clone();
                self.connectors[index].src = if let Some(pin_id) = self
                    .find_best_pin_id_for_connector(
                        shape_id,
                        class_id,
                        index,
                        Some(&other_endpoint),
                    ) {
                    Endpoint::ShapePin {
                        shape_id: shape_id.to_string(),
                        pin_id,
                        pin_class_id: Some(class_id),
                    }
                } else {
                    Endpoint::ShapePinClass {
                        shape_id: shape_id.to_string(),
                        pin_class_id: class_id,
                    }
                };
            }
            if dst_matches {
                let other_endpoint = self.connectors[index].src.clone();
                self.connectors[index].dst = if let Some(pin_id) = self
                    .find_best_pin_id_for_connector(
                        shape_id,
                        class_id,
                        index,
                        Some(&other_endpoint),
                    ) {
                    Endpoint::ShapePin {
                        shape_id: shape_id.to_string(),
                        pin_id,
                        pin_class_id: Some(class_id),
                    }
                } else {
                    Endpoint::ShapePinClass {
                        shape_id: shape_id.to_string(),
                        pin_class_id: class_id,
                    }
                };
            }
            self.connectors[index].make_path_invalid();
        }
    }

    fn reselect_junction_pin_class_endpoints(&mut self, junction_id: &str, class_id: u32) {
        for index in 0..self.connectors.len() {
            let src_matches =
                endpoint_is_junction_pin_class(&self.connectors[index].src, junction_id, class_id);
            let dst_matches =
                endpoint_is_junction_pin_class(&self.connectors[index].dst, junction_id, class_id);
            if !src_matches && !dst_matches {
                continue;
            }
            if src_matches {
                self.connectors[index].src = if let Some(pin_id) =
                    self.find_best_junction_pin_id_for_connector(junction_id, class_id, index)
                {
                    Endpoint::JunctionPin {
                        junction_id: junction_id.to_string(),
                        pin_id,
                        pin_class_id: Some(class_id),
                    }
                } else {
                    Endpoint::JunctionPinClass {
                        junction_id: junction_id.to_string(),
                        pin_class_id: class_id,
                    }
                };
            }
            if dst_matches {
                self.connectors[index].dst = if let Some(pin_id) =
                    self.find_best_junction_pin_id_for_connector(junction_id, class_id, index)
                {
                    Endpoint::JunctionPin {
                        junction_id: junction_id.to_string(),
                        pin_id,
                        pin_class_id: Some(class_id),
                    }
                } else {
                    Endpoint::JunctionPinClass {
                        junction_id: junction_id.to_string(),
                        pin_class_id: class_id,
                    }
                };
            }
            self.connectors[index].make_path_invalid();
        }
    }

    fn force_remove_cluster(&mut self, id: &str) -> bool {
        let before = self.clusters.len();
        self.clusters.retain(|cluster| cluster.id != id);
        let removed = self.clusters.len() != before;
        if removed {
            self.invalidate_all_connector_paths();
        }
        removed
    }

    fn force_remove_connector(&mut self, id: &str) -> bool {
        let before = self.connectors.len();
        self.connectors.retain(|connector| connector.id != id);
        let removed = self.connectors.len() != before;
        if removed {
            self.mark_visibility_graph_dirty();
        }
        removed
    }

    fn remove_pending_shape_actions(&mut self, id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::DeleteShape { id: action_id } => action_id != id,
            PendingAction::SetShapeRect { id: action_id, .. }
            | PendingAction::SetShapePolygon { id: action_id, .. }
            | PendingAction::TranslateShape { id: action_id, .. } => action_id != id,
            PendingAction::DeleteShapeConnectionPin {
                shape_id: action_id,
                ..
            } => action_id != id,
            PendingAction::ChangeShapeConnectionPin {
                shape_id: action_id,
                ..
            } => action_id != id,
            PendingAction::SetJunctionPosition { .. }
            | PendingAction::TranslateJunction { .. }
            | PendingAction::DeleteJunction { .. }
            | PendingAction::DeleteJunctionConnectionPin { .. }
            | PendingAction::ChangeJunctionConnectionPin { .. }
            | PendingAction::SetClusterPolygon { .. }
            | PendingAction::DeleteCluster { .. }
            | PendingAction::DeleteConnector { .. } => true,
            PendingAction::SetConnectorEndpoints { src, dst, .. } => {
                !src.references_shape(id) && !dst.references_shape(id)
            }
            PendingAction::SetConnectorSourceEndpoint { src, .. } => !src.references_shape(id),
            PendingAction::SetConnectorDestEndpoint { dst, .. } => !dst.references_shape(id),
        });
    }

    fn remove_pending_junction_actions(&mut self, id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::DeleteJunction { id: action_id } => action_id != id,
            PendingAction::SetJunctionPosition { id: action_id, .. }
            | PendingAction::TranslateJunction { id: action_id, .. } => action_id != id,
            PendingAction::DeleteJunctionConnectionPin {
                junction_id: action_id,
                ..
            } => action_id != id,
            PendingAction::ChangeJunctionConnectionPin {
                junction_id: action_id,
                ..
            } => action_id != id,
            PendingAction::DeleteShape { .. }
            | PendingAction::SetShapeRect { .. }
            | PendingAction::SetShapePolygon { .. }
            | PendingAction::TranslateShape { .. }
            | PendingAction::DeleteShapeConnectionPin { .. }
            | PendingAction::ChangeShapeConnectionPin { .. }
            | PendingAction::SetClusterPolygon { .. }
            | PendingAction::DeleteCluster { .. }
            | PendingAction::DeleteConnector { .. } => true,
            PendingAction::SetConnectorEndpoints { src, dst, .. } => {
                !src.references_junction(id) && !dst.references_junction(id)
            }
            PendingAction::SetConnectorSourceEndpoint { src, .. } => !src.references_junction(id),
            PendingAction::SetConnectorDestEndpoint { dst, .. } => !dst.references_junction(id),
        });
    }

    fn remove_pending_connector_actions(&mut self, id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::DeleteConnector { id: action_id }
            | PendingAction::SetConnectorEndpoints { id: action_id, .. }
            | PendingAction::SetConnectorSourceEndpoint { id: action_id, .. }
            | PendingAction::SetConnectorDestEndpoint { id: action_id, .. } => action_id != id,
            PendingAction::DeleteShape { .. }
            | PendingAction::DeleteShapeConnectionPin { .. }
            | PendingAction::ChangeShapeConnectionPin { .. }
            | PendingAction::SetShapeRect { .. }
            | PendingAction::SetShapePolygon { .. }
            | PendingAction::TranslateShape { .. }
            | PendingAction::SetJunctionPosition { .. }
            | PendingAction::TranslateJunction { .. }
            | PendingAction::DeleteJunction { .. }
            | PendingAction::DeleteJunctionConnectionPin { .. }
            | PendingAction::ChangeJunctionConnectionPin { .. }
            | PendingAction::SetClusterPolygon { .. }
            | PendingAction::DeleteCluster { .. } => true,
        });
    }

    fn queue_shape_connection_pin_change(&mut self, shape_id: String, pin_id: String) {
        if !self.pending_actions.iter().any(|action| {
            matches!(
                action,
                PendingAction::ChangeShapeConnectionPin {
                    shape_id: action_shape_id,
                    pin_id: action_pin_id,
                } if action_shape_id == &shape_id && action_pin_id == &pin_id
            )
        }) {
            self.pending_actions
                .push(PendingAction::ChangeShapeConnectionPin { shape_id, pin_id });
        }
    }

    fn queue_junction_connection_pin_change(&mut self, junction_id: String, pin_id: String) {
        if !self.pending_actions.iter().any(|action| {
            matches!(
                action,
                PendingAction::ChangeJunctionConnectionPin {
                    junction_id: action_junction_id,
                    pin_id: action_pin_id,
                } if action_junction_id == &junction_id && action_pin_id == &pin_id
            )
        }) {
            self.pending_actions
                .push(PendingAction::ChangeJunctionConnectionPin {
                    junction_id,
                    pin_id,
                });
        }
    }

    fn remove_pending_shape_pin_actions(&mut self, shape_id: &str, pin_id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::DeleteShapeConnectionPin {
                shape_id: action_shape_id,
                pin_id: action_pin_id,
            } => action_shape_id != shape_id || action_pin_id != pin_id,
            PendingAction::ChangeShapeConnectionPin {
                shape_id: action_shape_id,
                pin_id: action_pin_id,
            } => action_shape_id != shape_id || action_pin_id != pin_id,
            PendingAction::SetConnectorEndpoints { src, dst, .. } => {
                !src.references_pin(shape_id, pin_id) && !dst.references_pin(shape_id, pin_id)
            }
            PendingAction::SetConnectorSourceEndpoint { src, .. } => {
                !src.references_pin(shape_id, pin_id)
            }
            PendingAction::SetConnectorDestEndpoint { dst, .. } => {
                !dst.references_pin(shape_id, pin_id)
            }
            PendingAction::DeleteShape { .. }
            | PendingAction::SetShapeRect { .. }
            | PendingAction::SetShapePolygon { .. }
            | PendingAction::TranslateShape { .. }
            | PendingAction::SetJunctionPosition { .. }
            | PendingAction::TranslateJunction { .. }
            | PendingAction::DeleteJunction { .. }
            | PendingAction::DeleteJunctionConnectionPin { .. }
            | PendingAction::ChangeJunctionConnectionPin { .. }
            | PendingAction::SetClusterPolygon { .. }
            | PendingAction::DeleteCluster { .. }
            | PendingAction::DeleteConnector { .. } => true,
        });
    }

    fn remove_pending_junction_pin_actions(&mut self, junction_id: &str, pin_id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::DeleteJunctionConnectionPin {
                junction_id: action_junction_id,
                pin_id: action_pin_id,
            } => action_junction_id != junction_id || action_pin_id != pin_id,
            PendingAction::ChangeJunctionConnectionPin {
                junction_id: action_junction_id,
                pin_id: action_pin_id,
            } => action_junction_id != junction_id || action_pin_id != pin_id,
            PendingAction::SetConnectorEndpoints { src, dst, .. } => {
                !src.references_junction_pin(junction_id, pin_id)
                    && !dst.references_junction_pin(junction_id, pin_id)
            }
            PendingAction::SetConnectorSourceEndpoint { src, .. } => {
                !src.references_junction_pin(junction_id, pin_id)
            }
            PendingAction::SetConnectorDestEndpoint { dst, .. } => {
                !dst.references_junction_pin(junction_id, pin_id)
            }
            PendingAction::DeleteShape { .. }
            | PendingAction::SetShapeRect { .. }
            | PendingAction::SetShapePolygon { .. }
            | PendingAction::TranslateShape { .. }
            | PendingAction::DeleteShapeConnectionPin { .. }
            | PendingAction::ChangeShapeConnectionPin { .. }
            | PendingAction::SetJunctionPosition { .. }
            | PendingAction::TranslateJunction { .. }
            | PendingAction::DeleteJunction { .. }
            | PendingAction::SetClusterPolygon { .. }
            | PendingAction::DeleteCluster { .. }
            | PendingAction::DeleteConnector { .. } => true,
        });
    }

    fn remove_pending_cluster_actions(&mut self, id: &str) {
        self.pending_actions.retain(|action| match action {
            PendingAction::SetClusterPolygon { id: action_id, .. }
            | PendingAction::DeleteCluster { id: action_id } => action_id != id,
            PendingAction::DeleteShape { .. }
            | PendingAction::DeleteShapeConnectionPin { .. }
            | PendingAction::ChangeShapeConnectionPin { .. }
            | PendingAction::SetShapeRect { .. }
            | PendingAction::SetShapePolygon { .. }
            | PendingAction::TranslateShape { .. }
            | PendingAction::SetJunctionPosition { .. }
            | PendingAction::TranslateJunction { .. }
            | PendingAction::DeleteJunction { .. }
            | PendingAction::DeleteJunctionConnectionPin { .. }
            | PendingAction::ChangeJunctionConnectionPin { .. }
            | PendingAction::DeleteConnector { .. }
            | PendingAction::SetConnectorEndpoints { .. }
            | PendingAction::SetConnectorSourceEndpoint { .. }
            | PendingAction::SetConnectorDestEndpoint { .. } => true,
        });
    }

    fn invalidate_all_connector_paths(&mut self) {
        for connector in &mut self.connectors {
            connector.make_path_invalid();
        }
        self.mark_visibility_graph_dirty();
    }

    fn invalidate_connectors_for_shape(&mut self, shape_id: &str) {
        for connector in &mut self.connectors {
            if connector.references_shape(shape_id) {
                connector.make_path_invalid();
            }
        }
        self.mark_visibility_graph_dirty();
    }

    fn invalidate_connectors_for_shape_pin(&mut self, shape_id: &str, pin_id: &str) {
        for connector in &mut self.connectors {
            if connector.references_pin(shape_id, pin_id) {
                connector.make_path_invalid();
            }
        }
        self.mark_visibility_graph_dirty();
    }

    fn invalidate_connectors_for_junction(&mut self, junction_id: &str) {
        for connector in &mut self.connectors {
            if connector.references_junction(junction_id) {
                connector.make_path_invalid();
            }
        }
        self.mark_visibility_graph_dirty();
    }

    fn invalidate_connectors_for_junction_pin(&mut self, junction_id: &str, pin_id: &str) {
        for connector in &mut self.connectors {
            if connector.references_junction_pin(junction_id, pin_id) {
                connector.make_path_invalid();
            }
        }
        self.mark_visibility_graph_dirty();
    }

    fn mark_visibility_graph_dirty(&mut self) {
        self.visibility_graph_dirty = true;
    }

    #[allow(dead_code)]
    fn rebuild_visibility_graph_if_dirty(&mut self) {
        if !self.visibility_graph_dirty {
            return;
        }
        let shapes = self
            .shapes
            .iter()
            .map(|shape| shape.polygon.clone())
            .collect::<Vec<_>>();
        let connection_points = self
            .connectors
            .iter()
            .flat_map(|connector| {
                [
                    self.resolve_endpoint(&connector.src),
                    self.resolve_endpoint(&connector.dst),
                ]
            })
            .flatten()
            .collect::<Vec<_>>();
        self.visibility_graph = build_static_visibility_graph(&shapes, &connection_points);
        self.visibility_graph_dirty = false;
    }

    #[cfg(test)]
    pub(crate) fn visibility_graph_dirty_for_tests(&self) -> bool {
        self.visibility_graph_dirty
    }

    #[cfg(test)]
    pub(crate) fn visibility_graph_for_tests(&mut self) -> &VisibilityGraph {
        self.rebuild_visibility_graph_if_dirty();
        &self.visibility_graph
    }

    fn refresh_invalidated_routes(&mut self) -> Vec<String> {
        let mut invalid_indices = self
            .connectors
            .iter()
            .enumerate()
            .filter_map(|(index, connector)| connector.needs_reroute.then_some(index))
            .collect::<Vec<_>>();
        invalid_indices.sort_by_key(|index| self.connectors[*index].hate_crossings);
        let mut updated_ids = Vec::new();
        for index in invalid_indices {
            let route = self.route(&self.connectors[index]);
            if self.connectors[index].display_route != route {
                updated_ids.push(self.connectors[index].id.clone());
            }
            self.connectors[index].set_display_route(route);
        }
        updated_ids
    }

    fn object_id_is_unused(&self, id: &str) -> bool {
        !self.shapes.iter().any(|shape| shape.id == id)
            && !self.clusters.iter().any(|cluster| cluster.id == id)
            && !self.junctions.iter().any(|junction| junction.id == id)
            && !self.connectors.iter().any(|connector| connector.id == id)
    }

    fn current_display_route_for_diagnostics(&self, connector: &Connector) -> Vec<Point> {
        if connector.needs_reroute || connector.display_route.is_empty() {
            self.route(connector)
        } else {
            connector.display_route.clone()
        }
    }

    pub(crate) fn route(&self, connector: &Connector) -> Vec<Point> {
        self.route_with_penalty_overrides(connector, &[])
    }

    fn route_with_penalty_overrides(
        &self,
        connector: &Connector,
        penalty_route_overrides: &[(&str, Vec<Point>)],
    ) -> Vec<Point> {
        if connector.routing_type == CONN_TYPE_NONE {
            return Vec::new();
        }
        if let Some(route) = &connector.fixed_route {
            return route.clone();
        }
        let Some(src) = self.resolve_endpoint(&connector.src) else {
            return Vec::new();
        };
        let Some(dst) = self.resolve_endpoint(&connector.dst) else {
            return Vec::new();
        };
        let ignored_shapes = connector.endpoint_shape_ids();
        let ignored_junctions = connector.endpoint_junction_ids();
        let obstacles = self
            .shapes
            .iter()
            .filter(|shape| !ignored_shapes.iter().any(|id| id == &&shape.id))
            .filter_map(|shape| {
                let polygon = shape.polygon.offset_polygon(self.clearance)?;
                let bounds = polygon.bounding_box()?.to_rect();
                Some(OrthogonalObstacle { polygon, bounds })
            })
            .chain(
                self.clusters
                    .iter()
                    .filter(|_| self.cluster_routing_enabled())
                    .filter(|cluster| {
                        !cluster.polygon.contains_point(src, true)
                            || !cluster.polygon.contains_point(dst, true)
                    })
                    .filter_map(|cluster| {
                        let bounds = cluster.polygon.bounding_box()?.to_rect();
                        Some(OrthogonalObstacle {
                            polygon: cluster.polygon.clone(),
                            bounds,
                        })
                    }),
            )
            .chain(
                self.junctions
                    .iter()
                    .filter(|junction| !ignored_junctions.iter().any(|id| id == &&junction.id))
                    .map(|junction| {
                        let bounds = self.junction_bounds(junction.position);
                        OrthogonalObstacle {
                            polygon: bounds.to_polygon(),
                            bounds,
                        }
                    }),
            )
            .collect::<Vec<_>>();

        let mut route = vec![src];
        let mut start = src;
        let mut start_directions = self.endpoint_directions(&connector.src);
        let crossing_routes =
            self.penalty_routes_for_connector(&connector.id, penalty_route_overrides);
        let cost_settings = self.routing_cost_settings();
        for checkpoint in &connector.checkpoints {
            if point_inside_obstacle(checkpoint.point, &obstacles) {
                continue;
            }
            if let Some(segment) = self.route_segment(
                start,
                checkpoint.point,
                &obstacles,
                &self.containing_cluster_polygons(start, checkpoint.point),
                &self.crossing_cluster_polygons(start, checkpoint.point),
                connector.routing_type,
                start_directions,
                checkpoint.arrival_directions,
                &crossing_routes,
                cost_settings,
            ) {
                append_segment(&mut route, segment);
                start = checkpoint.point;
                start_directions = checkpoint.departure_directions;
            }
        }
        let segment = self
            .route_segment(
                start,
                dst,
                &obstacles,
                &self.containing_cluster_polygons(start, dst),
                &self.crossing_cluster_polygons(start, dst),
                connector.routing_type,
                start_directions,
                self.endpoint_directions(&connector.dst),
                &crossing_routes,
                cost_settings,
            )
            .unwrap_or_else(|| vec![start, dst]);
        append_segment(&mut route, segment);
        route
    }

    fn route_segment(
        &self,
        src: Point,
        dst: Point,
        obstacles: &[OrthogonalObstacle],
        containing_clusters: &[&Polygon],
        crossing_clusters: &[&Polygon],
        routing_type: u32,
        src_directions: u32,
        dst_directions: u32,
        crossing_routes: &[Vec<Point>],
        cost_settings: RoutingCostSettings,
    ) -> Option<Vec<Point>> {
        if !containing_clusters.is_empty() {
            return route_orthogonal_inside_clusters(
                src,
                dst,
                obstacles,
                containing_clusters,
                src_directions,
                dst_directions,
            );
        }
        if !crossing_clusters.is_empty() {
            return route_orthogonal_across_cluster_boundaries(
                src,
                dst,
                obstacles,
                crossing_clusters,
                self.clearance,
                src_directions,
                dst_directions,
            );
        }
        if clear_segment(src, dst, obstacles)
            && segment_satisfies_directions(src, dst, src_directions, dst_directions)
            && (cost_settings.crossing_penalty <= 0.0
                || route_crossing_count(src, dst, crossing_routes) == 0)
            && (cost_settings.fixed_shared_path_penalty <= 0.0
                || route_shared_path_count(src, dst, crossing_routes) == 0)
        {
            Some(vec![src, dst])
        } else if routing_type == CONN_TYPE_POLY_LINE {
            route_polyline(
                src,
                dst,
                obstacles,
                src_directions,
                dst_directions,
                crossing_routes,
                cost_settings,
            )
        } else {
            route_orthogonal_with_directions(
                src,
                dst,
                obstacles,
                crossing_routes,
                self.clearance,
                src_directions,
                dst_directions,
                cost_settings,
            )
        }
    }

    fn routing_cost_settings(&self) -> RoutingCostSettings {
        RoutingCostSettings {
            segment_penalty: self.routing_parameter(SEGMENT_PENALTY).max(0.0),
            angle_penalty: self.routing_parameter(ANGLE_PENALTY).max(0.0),
            crossing_penalty: self.routing_parameter(CROSSING_PENALTY).max(0.0),
            fixed_shared_path_penalty: self.routing_parameter(FIXED_SHARED_PATH_PENALTY).max(0.0),
            reverse_direction_penalty: self.routing_parameter(REVERSE_DIRECTION_PENALTY).max(0.0),
        }
    }

    fn penalty_routes_for_connector(
        &self,
        connector_id: &str,
        overrides: &[(&str, Vec<Point>)],
    ) -> Vec<Vec<Point>> {
        let mut routes = Vec::new();
        for connector in &self.connectors {
            if connector.id == connector_id {
                continue;
            }
            if let Some((_, route)) = overrides
                .iter()
                .find(|(id, _)| *id == connector.id.as_str())
            {
                routes.push(route.clone());
            } else if let Some(route) = &connector.fixed_route {
                routes.push(route.clone());
            } else if connector.display_route.len() >= 2 {
                routes.push(connector.display_route.clone());
            }
        }
        routes
    }

    fn resolve_endpoint(&self, endpoint: &Endpoint) -> Option<Point> {
        match endpoint {
            Endpoint::Point { point, .. } => Some(*point),
            Endpoint::ShapePin {
                shape_id, pin_id, ..
            } => self.find_pin_point(shape_id, pin_id),
            Endpoint::ShapePinClass { shape_id, .. } => self.find_shape_position(shape_id),
            Endpoint::Junction { junction_id } => self.find_junction_point(junction_id),
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                ..
            } => self.find_junction_pin_point(junction_id, pin_id),
            Endpoint::JunctionPinClass { junction_id, .. } => self.find_junction_point(junction_id),
        }
    }

    fn endpoint_directions(&self, endpoint: &Endpoint) -> u32 {
        match endpoint {
            Endpoint::Point { directions, .. } => *directions,
            Endpoint::ShapePin { .. }
            | Endpoint::ShapePinClass { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPinClass { .. } => CONN_DIR_ALL,
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                ..
            } => self
                .junctions
                .iter()
                .find(|junction| junction.id == junction_id.as_str())
                .and_then(|junction| junction.pins.iter().find(|pin| pin.id == pin_id.as_str()))
                .map(|pin| pin.visibility_directions)
                .unwrap_or(CONN_DIR_NONE),
        }
    }

    fn conn_end_directions(&self, endpoint: &Endpoint) -> u32 {
        match endpoint {
            Endpoint::Point { directions, .. } => *directions,
            Endpoint::ShapePin {
                shape_id, pin_id, ..
            } => self
                .shapes
                .iter()
                .find(|shape| shape.id == shape_id.as_str())
                .and_then(|shape| shape.pins.iter().find(|pin| pin.id == pin_id.as_str()))
                .map(|pin| pin.visibility_directions)
                .unwrap_or(CONN_DIR_NONE),
            Endpoint::ShapePinClass { .. } | Endpoint::Junction { .. } => CONN_DIR_ALL,
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                ..
            } => self
                .junctions
                .iter()
                .find(|junction| junction.id == junction_id.as_str())
                .and_then(|junction| junction.pins.iter().find(|pin| pin.id == pin_id.as_str()))
                .map(|pin| pin.visibility_directions)
                .unwrap_or(CONN_DIR_NONE),
            Endpoint::JunctionPinClass { .. } => CONN_DIR_ALL,
        }
    }

    fn endpoint_to_json(&self, endpoint: &Endpoint, out: &mut String) {
        match endpoint {
            Endpoint::Point { point, directions } => {
                out.push_str("{\"type\":\"point\",\"point\":");
                out.push_str(&point_to_json(*point));
                out.push_str(",\"position\":");
                out.push_str(&point_to_json(*point));
                out.push_str(",\"directions\":");
                out.push_str(&directions.to_string());
                out.push('}');
            }
            Endpoint::ShapePin {
                shape_id,
                pin_id,
                pin_class_id,
            } => {
                out.push_str("{\"type\":\"shapePin\",\"shapeId\":\"");
                push_json_string(out, shape_id);
                out.push_str("\",\"pinId\":\"");
                push_json_string(out, pin_id);
                out.push('"');
                if let Some(class_id) = pin_class_id {
                    out.push_str(",\"classId\":");
                    out.push_str(&class_id.to_string());
                }
                if let Some(position) = self.resolve_endpoint(endpoint) {
                    out.push_str(",\"position\":");
                    out.push_str(&point_to_json(position));
                }
                out.push_str(",\"directions\":");
                out.push_str(&self.conn_end_directions(endpoint).to_string());
                out.push('}');
            }
            Endpoint::ShapePinClass {
                shape_id,
                pin_class_id,
            } => {
                out.push_str("{\"type\":\"shapePin\",\"shapeId\":\"");
                push_json_string(out, shape_id);
                out.push_str("\",\"classId\":");
                out.push_str(&pin_class_id.to_string());
                if let Some(position) = self.resolve_endpoint(endpoint) {
                    out.push_str(",\"position\":");
                    out.push_str(&point_to_json(position));
                }
                out.push_str(",\"directions\":");
                out.push_str(&self.conn_end_directions(endpoint).to_string());
                out.push('}');
            }
            Endpoint::Junction { junction_id } => {
                out.push_str("{\"type\":\"junction\",\"junctionId\":\"");
                push_json_string(out, junction_id);
                out.push('"');
                if let Some(position) = self.resolve_endpoint(endpoint) {
                    out.push_str(",\"position\":");
                    out.push_str(&point_to_json(position));
                }
                out.push_str(",\"directions\":");
                out.push_str(&self.conn_end_directions(endpoint).to_string());
                out.push('}');
            }
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                pin_class_id,
            } => {
                out.push_str("{\"type\":\"junctionPin\",\"junctionId\":\"");
                push_json_string(out, junction_id);
                out.push_str("\",\"pinId\":\"");
                push_json_string(out, pin_id);
                out.push('"');
                if let Some(class_id) = pin_class_id {
                    out.push_str(",\"classId\":");
                    out.push_str(&class_id.to_string());
                }
                if let Some(position) = self.resolve_endpoint(endpoint) {
                    out.push_str(",\"position\":");
                    out.push_str(&point_to_json(position));
                }
                out.push_str(",\"directions\":");
                out.push_str(&self.conn_end_directions(endpoint).to_string());
                out.push('}');
            }
            Endpoint::JunctionPinClass {
                junction_id,
                pin_class_id,
            } => {
                out.push_str("{\"type\":\"junctionPin\",\"junctionId\":\"");
                push_json_string(out, junction_id);
                out.push_str("\",\"classId\":");
                out.push_str(&pin_class_id.to_string());
                if let Some(position) = self.resolve_endpoint(endpoint) {
                    out.push_str(",\"position\":");
                    out.push_str(&point_to_json(position));
                }
                out.push_str(",\"directions\":");
                out.push_str(&self.conn_end_directions(endpoint).to_string());
                out.push('}');
            }
        }
    }

    fn find_shape_position(&self, shape_id: &str) -> Option<Point> {
        let shape = self.shapes.iter().find(|shape| shape.id == shape_id)?;
        Some(Point {
            x: shape.rect.left + ((shape.rect.right - shape.rect.left) * 0.5),
            y: shape.rect.top + ((shape.rect.bottom - shape.rect.top) * 0.5),
        })
    }

    fn find_pin_point(&self, shape_id: &str, pin_id: &str) -> Option<Point> {
        let shape = self.shapes.iter().find(|shape| shape.id == shape_id)?;
        let pin = shape.pins.iter().find(|pin| pin.id == pin_id)?;
        Some(pin.resolve_position(shape.rect))
    }

    fn shape_pin_class_id(&self, shape_id: &str, pin_id: &str) -> Option<u32> {
        self.shapes
            .iter()
            .find(|shape| shape.id == shape_id)?
            .pins
            .iter()
            .find(|pin| pin.id == pin_id)
            .map(|pin| pin.class_id)
    }

    fn find_pin_class_points(&self, shape_id: &str, class_id: u32) -> Vec<Point> {
        self.shapes
            .iter()
            .find(|shape| shape.id == shape_id)
            .map(|shape| {
                shape
                    .pins
                    .iter()
                    .filter(|pin| pin.class_id == class_id)
                    .filter(|pin| !pin.exclusive || !self.is_pin_used(shape_id, &pin.id))
                    .map(|pin| pin.resolve_position(shape.rect))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn find_junction_pin_point(&self, junction_id: &str, pin_id: &str) -> Option<Point> {
        let junction = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)?;
        junction.pins.iter().find(|pin| pin.id == pin_id)?;
        Some(junction.position)
    }

    fn junction_pin_class_id(&self, junction_id: &str, pin_id: &str) -> Option<u32> {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)?
            .pins
            .iter()
            .find(|pin| pin.id == pin_id)
            .map(|pin| pin.class_id)
    }

    fn find_junction_pin_class_points(&self, junction_id: &str, class_id: u32) -> Vec<Point> {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)
            .map(|junction| {
                junction
                    .pins
                    .iter()
                    .filter(|pin| pin.class_id == class_id)
                    .filter(|pin| {
                        !pin.exclusive || !self.is_junction_pin_used(junction_id, &pin.id)
                    })
                    .map(|_| junction.position)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn containing_cluster_polygons(&self, src: Point, dst: Point) -> Vec<&Polygon> {
        if !self.cluster_routing_enabled() {
            return Vec::new();
        }
        self.clusters
            .iter()
            .filter(|cluster| {
                cluster.polygon.contains_point(src, true)
                    && cluster.polygon.contains_point(dst, true)
            })
            .map(|cluster| &cluster.polygon)
            .collect()
    }

    fn crossing_cluster_polygons(&self, src: Point, dst: Point) -> Vec<&Polygon> {
        if !self.cluster_routing_enabled() {
            return Vec::new();
        }
        self.clusters
            .iter()
            .filter(|cluster| {
                cluster.polygon.contains_point(src, true)
                    != cluster.polygon.contains_point(dst, true)
            })
            .map(|cluster| &cluster.polygon)
            .collect()
    }

    fn cluster_routing_enabled(&self) -> bool {
        self.routing_parameters
            .get(CLUSTER_CROSSING_PENALTY as usize)
            .copied()
            .unwrap_or(0.0)
            > 0.0
    }

    fn find_junction_point(&self, junction_id: &str) -> Option<Point> {
        self.junctions
            .iter()
            .find(|junction| junction.id == junction_id)
            .map(|junction| junction.position)
    }

    fn junction_bounds(&self, position: Point) -> Rect {
        let offset = self
            .routing_parameters
            .get(IDEAL_NUDGING_DISTANCE as usize)
            .copied()
            .unwrap_or(1.0)
            .min(1.0)
            .max(0.0);
        Rect::new(
            position.x - offset,
            position.y - offset,
            position.x + offset,
            position.y + offset,
        )
    }

    fn find_best_pin_id(&self, shape_id: &str, class_id: u32) -> Option<String> {
        let shape = self.shapes.iter().find(|shape| shape.id == shape_id)?;
        shape
            .pins
            .iter()
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| !pin.exclusive || !self.is_pin_used(shape_id, &pin.id))
            .min_by(|a, b| {
                a.connection_cost
                    .partial_cmp(&b.connection_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn find_best_pin_id_except(
        &self,
        shape_id: &str,
        class_id: u32,
        excluded_pin_id: &str,
    ) -> Option<String> {
        let shape = self.shapes.iter().find(|shape| shape.id == shape_id)?;
        shape
            .pins
            .iter()
            .filter(|pin| pin.id != excluded_pin_id)
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| !pin.exclusive || !self.is_pin_used(shape_id, &pin.id))
            .min_by(|a, b| {
                a.connection_cost
                    .partial_cmp(&b.connection_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn find_best_pin_id_for_connector(
        &self,
        shape_id: &str,
        class_id: u32,
        connector_index: usize,
        other_endpoint: Option<&Endpoint>,
    ) -> Option<String> {
        let shape = self.shapes.iter().find(|shape| shape.id == shape_id)?;
        let target = other_endpoint.and_then(|endpoint| self.resolve_endpoint(endpoint));
        shape
            .pins
            .iter()
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| {
                !pin.exclusive
                    || !self.is_pin_used_by_other_connector(shape_id, &pin.id, connector_index)
            })
            .min_by(|a, b| {
                self.shape_pin_selection_cost(shape.rect, a, target)
                    .partial_cmp(&self.shape_pin_selection_cost(shape.rect, b, target))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn shape_pin_selection_cost(
        &self,
        shape_rect: Rect,
        pin: &ShapeConnectionPin,
        target: Option<Point>,
    ) -> f64 {
        let mut cost = pin.connection_cost;
        if let Some(target) = target {
            let directions = normalized_conn_directions(pin.visibility_directions);
            if directions != CONN_DIR_ALL
                && !pin_visibility_range_contains_target(
                    pin.resolve_position(shape_rect),
                    target,
                    directions,
                )
            {
                cost += self.routing_parameter(PORT_DIRECTION_PENALTY).max(0.0);
            }
        }
        cost
    }

    fn is_pin_used(&self, shape_id: &str, pin_id: &str) -> bool {
        self.connectors
            .iter()
            .any(|connector| connector.references_pin(shape_id, pin_id))
    }

    fn is_pin_used_by_other_connector(
        &self,
        shape_id: &str,
        pin_id: &str,
        connector_index: usize,
    ) -> bool {
        self.connectors
            .iter()
            .enumerate()
            .any(|(index, connector)| {
                index != connector_index && connector.references_pin(shape_id, pin_id)
            })
    }

    fn find_best_junction_pin_id(&self, junction_id: &str, class_id: u32) -> Option<String> {
        let junction = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)?;
        junction
            .pins
            .iter()
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| !pin.exclusive || !self.is_junction_pin_used(junction_id, &pin.id))
            .min_by(|a, b| {
                a.connection_cost
                    .partial_cmp(&b.connection_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn find_best_junction_pin_id_except(
        &self,
        junction_id: &str,
        class_id: u32,
        excluded_pin_id: &str,
    ) -> Option<String> {
        let junction = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)?;
        junction
            .pins
            .iter()
            .filter(|pin| pin.id != excluded_pin_id)
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| !pin.exclusive || !self.is_junction_pin_used(junction_id, &pin.id))
            .min_by(|a, b| {
                a.connection_cost
                    .partial_cmp(&b.connection_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn find_best_junction_pin_id_for_connector(
        &self,
        junction_id: &str,
        class_id: u32,
        connector_index: usize,
    ) -> Option<String> {
        let junction = self
            .junctions
            .iter()
            .find(|junction| junction.id == junction_id)?;
        junction
            .pins
            .iter()
            .filter(|pin| pin.class_id == class_id)
            .filter(|pin| {
                !pin.exclusive
                    || !self.is_junction_pin_used_by_other_connector(
                        junction_id,
                        &pin.id,
                        connector_index,
                    )
            })
            .min_by(|a, b| {
                a.connection_cost
                    .partial_cmp(&b.connection_cost)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|pin| pin.id.clone())
    }

    fn is_junction_pin_used(&self, junction_id: &str, pin_id: &str) -> bool {
        self.connectors
            .iter()
            .any(|connector| connector.references_junction_pin(junction_id, pin_id))
    }

    fn is_junction_pin_used_by_other_connector(
        &self,
        junction_id: &str,
        pin_id: &str,
        connector_index: usize,
    ) -> bool {
        self.connectors
            .iter()
            .enumerate()
            .any(|(index, connector)| {
                index != connector_index && connector.references_junction_pin(junction_id, pin_id)
            })
    }
}

fn endpoint_is_shape_pin_class(endpoint: &Endpoint, shape_id: &str, class_id: u32) -> bool {
    matches!(
        endpoint,
        Endpoint::ShapePin {
            shape_id: endpoint_shape_id,
            pin_class_id: Some(endpoint_class_id),
            ..
        } | Endpoint::ShapePinClass {
            shape_id: endpoint_shape_id,
            pin_class_id: endpoint_class_id,
        } if endpoint_shape_id == shape_id && *endpoint_class_id == class_id
    )
}

fn endpoint_is_junction_pin_class(endpoint: &Endpoint, junction_id: &str, class_id: u32) -> bool {
    matches!(
        endpoint,
        Endpoint::JunctionPin {
            junction_id: endpoint_junction_id,
            pin_class_id: Some(endpoint_class_id),
            ..
        } | Endpoint::JunctionPinClass {
            junction_id: endpoint_junction_id,
            pin_class_id: endpoint_class_id,
        } if endpoint_junction_id == junction_id && *endpoint_class_id == class_id
    )
}

fn pin_visibility_range_contains_target(pin_point: Point, target: Point, directions: u32) -> bool {
    let adjacent_target = Point {
        x: target.x - pin_point.x,
        y: target.y - pin_point.y,
    };
    let angle = rotational_angle(adjacent_target);
    ((angle <= 45.0 || angle >= 315.0) && (directions & CONN_DIR_RIGHT != 0))
        || ((45.0..=135.0).contains(&angle) && (directions & CONN_DIR_DOWN != 0))
        || ((135.0..=225.0).contains(&angle) && (directions & CONN_DIR_LEFT != 0))
        || ((225.0..=315.0).contains(&angle) && (directions & CONN_DIR_UP != 0))
}

fn normalized_conn_directions(directions: u32) -> u32 {
    let directions = directions & CONN_DIR_ALL;
    if directions == CONN_DIR_NONE {
        CONN_DIR_ALL
    } else {
        directions
    }
}

fn valid_shape_pin_class_id(class_id: u32) -> bool {
    class_id != 0 && class_id != CONNECTION_PIN_UNSET
}

fn valid_shape_connection_pin_class_id(class_id: u32) -> bool {
    class_id != 0
}

fn append_segment(route: &mut Vec<Point>, segment: Vec<Point>) {
    for (index, point) in segment.into_iter().enumerate() {
        if index == 0 && route.last() == Some(&point) {
            continue;
        }
        route.push(point);
    }
}

fn point_inside_obstacle(point: Point, obstacles: &[OrthogonalObstacle]) -> bool {
    obstacles
        .iter()
        .any(|obstacle| obstacle.polygon.contains_point(point, false))
}

fn route_orthogonal_inside_clusters(
    src: Point,
    dst: Point,
    obstacles: &[OrthogonalObstacle],
    containing_clusters: &[&Polygon],
    src_directions: u32,
    dst_directions: u32,
) -> Option<Vec<Point>> {
    let mut xs = vec![src.x, dst.x];
    let mut ys = vec![src.y, dst.y];
    for cluster in containing_clusters {
        for point in &cluster.points {
            xs.push(point.x);
            ys.push(point.y);
        }
    }
    for obstacle in obstacles {
        xs.extend([obstacle.bounds.left, obstacle.bounds.right]);
        ys.extend([obstacle.bounds.top, obstacle.bounds.bottom]);
    }
    sort_dedup_numbers(&mut xs);
    sort_dedup_numbers(&mut ys);

    let mut points = Vec::new();
    for x in xs {
        for y in &ys {
            let point = Point { x, y: *y };
            if !point_inside_obstacle(point, obstacles)
                && containing_clusters
                    .iter()
                    .all(|cluster| cluster.contains_point(point, true))
            {
                points.push(point);
            }
        }
    }
    push_unique_point(&mut points, src);
    push_unique_point(&mut points, dst);

    let src_index = points.iter().position(|point| *point == src)?;
    let dst_index = points.iter().position(|point| *point == dst)?;
    let mut distances = vec![f64::INFINITY; points.len()];
    let mut previous = vec![None; points.len()];
    let mut visited = HashSet::new();
    distances[src_index] = 0.0;

    loop {
        let Some(current) = (0..points.len())
            .filter(|index| !visited.contains(index))
            .min_by(|left, right| distances[*left].total_cmp(&distances[*right]))
        else {
            break;
        };
        if !distances[current].is_finite() || current == dst_index {
            break;
        }
        visited.insert(current);
        for next in 0..points.len() {
            if current == next || visited.contains(&next) {
                continue;
            }
            if !inside_cluster_edge_is_clear(
                points[current],
                points[next],
                obstacles,
                containing_clusters,
            ) {
                continue;
            }
            if current == src_index
                && !segment_satisfies_directions(
                    points[current],
                    points[next],
                    src_directions,
                    CONN_DIR_ALL,
                )
            {
                continue;
            }
            if next == dst_index
                && !segment_satisfies_directions(
                    points[current],
                    points[next],
                    CONN_DIR_ALL,
                    dst_directions,
                )
            {
                continue;
            }
            let candidate = distances[current] + manhattan_distance(points[current], points[next]);
            if candidate + EPS < distances[next] {
                distances[next] = candidate;
                previous[next] = Some(current);
            }
        }
    }

    if !distances[dst_index].is_finite() {
        return None;
    }
    let mut route = vec![dst];
    let mut current = dst_index;
    while current != src_index {
        current = previous[current]?;
        route.push(points[current]);
    }
    route.reverse();
    Some(simplify_orthogonal_points(route))
}

fn route_orthogonal_across_cluster_boundaries(
    src: Point,
    dst: Point,
    obstacles: &[OrthogonalObstacle],
    crossing_clusters: &[&Polygon],
    clearance: f64,
    src_directions: u32,
    dst_directions: u32,
) -> Option<Vec<Point>> {
    let cluster = crossing_clusters.first()?;
    let src_inside = cluster.contains_point(src, true);
    let dst_inside = cluster.contains_point(dst, true);
    if src_inside == dst_inside {
        return None;
    }

    let inner_obstacles = obstacles
        .iter()
        .filter(|obstacle| &obstacle.polygon != *cluster)
        .cloned()
        .collect::<Vec<_>>();
    let mut candidates = cluster_boundary_candidates(cluster, src, dst);
    candidates.retain(|point| {
        cluster.contains_point(*point, true)
            && !inner_obstacles
                .iter()
                .any(|obstacle| obstacle.polygon.contains_point(*point, false))
    });
    candidates.sort_by(|a, b| {
        let a_cost = manhattan_distance(src, *a) + manhattan_distance(*a, dst);
        let b_cost = manhattan_distance(src, *b) + manhattan_distance(*b, dst);
        a_cost.total_cmp(&b_cost)
    });
    candidates.dedup_by(|a, b| (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS);

    let mut best_route = None;
    let mut best_cost = f64::INFINITY;
    for boundary in candidates {
        let (inside_start, inside_end, inside_src_dirs, inside_dst_dirs) = if src_inside {
            (src, boundary, src_directions, CONN_DIR_ALL)
        } else {
            (boundary, dst, CONN_DIR_ALL, dst_directions)
        };
        let Some(inside_route) = route_orthogonal_inside_clusters(
            inside_start,
            inside_end,
            &inner_obstacles,
            &[*cluster],
            inside_src_dirs,
            inside_dst_dirs,
        ) else {
            continue;
        };

        let (outside_start, outside_end, outside_src_dirs, outside_dst_dirs) = if src_inside {
            (boundary, dst, CONN_DIR_ALL, dst_directions)
        } else {
            (src, boundary, src_directions, CONN_DIR_ALL)
        };
        let Some(outside_route) = route_orthogonal_with_directions(
            outside_start,
            outside_end,
            &inner_obstacles,
            &[],
            clearance,
            outside_src_dirs,
            outside_dst_dirs,
            RoutingCostSettings {
                segment_penalty: sensible_routing_parameter(SEGMENT_PENALTY),
                angle_penalty: sensible_routing_parameter(ANGLE_PENALTY),
                crossing_penalty: sensible_routing_parameter(CROSSING_PENALTY),
                fixed_shared_path_penalty: sensible_routing_parameter(FIXED_SHARED_PATH_PENALTY),
                reverse_direction_penalty: sensible_routing_parameter(REVERSE_DIRECTION_PENALTY),
            },
        ) else {
            continue;
        };

        let mut route = Vec::new();
        if src_inside {
            append_segment(&mut route, inside_route);
            append_segment(&mut route, outside_route);
        } else {
            append_segment(&mut route, outside_route);
            append_segment(&mut route, inside_route);
        }
        let cost = route
            .windows(2)
            .map(|segment| manhattan_distance(segment[0], segment[1]))
            .sum();
        if cost < best_cost {
            best_cost = cost;
            best_route = Some(route);
        }
    }

    best_route
}

fn cluster_boundary_candidates(cluster: &Polygon, src: Point, dst: Point) -> Vec<Point> {
    let mut points = cluster.points.clone();
    for point in [src, dst] {
        points.extend(axis_boundary_candidates(cluster, point));
    }
    points
}

fn axis_boundary_candidates(cluster: &Polygon, point: Point) -> Vec<Point> {
    let mut candidates = Vec::new();
    for (a, b) in cluster.edges() {
        if (a.x - b.x).abs() < EPS {
            let min_y = a.y.min(b.y);
            let max_y = a.y.max(b.y);
            if point.y + EPS >= min_y && point.y - EPS <= max_y {
                candidates.push(Point { x: a.x, y: point.y });
            }
        }
        if (a.y - b.y).abs() < EPS {
            let min_x = a.x.min(b.x);
            let max_x = a.x.max(b.x);
            if point.x + EPS >= min_x && point.x - EPS <= max_x {
                candidates.push(Point { x: point.x, y: a.y });
            }
        }
    }
    candidates
}

fn inside_cluster_edge_is_clear(
    a: Point,
    b: Point,
    obstacles: &[OrthogonalObstacle],
    containing_clusters: &[&Polygon],
) -> bool {
    clear_segment(a, b, obstacles)
        && containing_clusters
            .iter()
            .all(|cluster| segment_stays_inside_polygon(a, b, cluster))
}

fn segment_stays_inside_polygon(a: Point, b: Point, polygon: &Polygon) -> bool {
    let midpoint = Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    };
    polygon.contains_point(a, true)
        && polygon.contains_point(b, true)
        && polygon.contains_point(midpoint, true)
        && !polygon
            .edges()
            .any(|(edge_start, edge_end)| segment_intersect(a, b, edge_start, edge_end))
}

fn push_unique_point(points: &mut Vec<Point>, point: Point) {
    if !points.iter().any(|existing| *existing == point) {
        points.push(point);
    }
}

fn sort_dedup_numbers(values: &mut Vec<f64>) {
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() < EPS);
}

fn simplify_orthogonal_points(path: Vec<Point>) -> Vec<Point> {
    let mut simplified = Vec::new();
    for point in path {
        simplified.push(point);
        while simplified.len() >= 3 {
            let len = simplified.len();
            let a = simplified[len - 3];
            let b = simplified[len - 2];
            let c = simplified[len - 1];
            if ((a.x - b.x).abs() < EPS && (b.x - c.x).abs() < EPS)
                || ((a.y - b.y).abs() < EPS && (b.y - c.y).abs() < EPS)
            {
                simplified.remove(len - 2);
            } else {
                break;
            }
        }
    }
    simplified
}

fn route_polyline(
    src: Point,
    dst: Point,
    obstacles: &[OrthogonalObstacle],
    src_directions: u32,
    dst_directions: u32,
    crossing_routes: &[Vec<Point>],
    cost_settings: RoutingCostSettings,
) -> Option<Vec<Point>> {
    let obstacle_polygons = obstacles
        .iter()
        .map(|obstacle| obstacle.polygon.clone())
        .collect::<Vec<_>>();
    let graph = build_static_visibility_graph(&obstacle_polygons, &[src, dst]);
    let src_index = 0;
    let dst_index = 1;
    let mut heap = BinaryHeap::new();
    let mut best: HashMap<(usize, Option<usize>), f64> = HashMap::new();
    let mut previous_state: HashMap<(usize, Option<usize>), (usize, Option<usize>)> =
        HashMap::new();
    let src_point = graph.vertices.get(src_index)?.point;
    let dst_point = graph.vertices.get(dst_index)?.point;

    heap.push(PolylineQueueState {
        cost: 0.0,
        node: src_index,
        previous: None,
    });
    best.insert((src_index, None), 0.0);

    let mut final_state = None;
    while let Some(state) = heap.pop() {
        let key = (state.node, state.previous);
        if state.cost > *best.get(&key).unwrap_or(&f64::INFINITY) + EPS {
            continue;
        }
        if state.node == dst_index {
            final_state = Some(key);
            break;
        }
        for (next, edge) in graph.neighbours(state.node) {
            let Some((from_point, to_point)) = graph.edge_points(state.node, next) else {
                continue;
            };
            if state.node == src_index
                && !segment_satisfies_directions(from_point, to_point, src_directions, CONN_DIR_ALL)
            {
                continue;
            }
            if next == dst_index
                && !segment_satisfies_directions(from_point, to_point, CONN_DIR_ALL, dst_directions)
            {
                continue;
            }
            let previous_point = state
                .previous
                .and_then(|index| graph.vertices.get(index).map(|vertex| vertex.point));
            let transition_cost = polyline_transition_cost(
                previous_point,
                from_point,
                to_point,
                edge.dist,
                src_point,
                dst_point,
                crossing_routes,
                cost_settings,
            );
            let candidate = state.cost + transition_cost;
            let next_key = (next, Some(state.node));
            if candidate + EPS < *best.get(&next_key).unwrap_or(&f64::INFINITY) {
                best.insert(next_key, candidate);
                previous_state.insert(next_key, key);
                heap.push(PolylineQueueState {
                    cost: candidate,
                    node: next,
                    previous: Some(state.node),
                });
            }
        }
    }

    let mut key = final_state?;
    let mut route = vec![graph.vertices.get(key.0)?.point];
    while key.0 != src_index {
        key = *previous_state.get(&key)?;
        route.push(graph.vertices.get(key.0)?.point);
    }
    route.reverse();
    Some(simplify_polyline(route))
}

pub(crate) fn polyline_transition_cost(
    previous: Option<Point>,
    current: Point,
    next: Point,
    edge_distance: f64,
    src: Point,
    dst: Point,
    crossing_routes: &[Vec<Point>],
    cost_settings: RoutingCostSettings,
) -> f64 {
    let mut cost = edge_distance;
    if let Some(previous) = previous {
        let turn = angle(previous, current, next);
        if turn > EPS {
            let xval = turn * 10.0 / std::f64::consts::PI;
            let yval = xval * (xval + 1.0).log10() / 10.5;
            cost += cost_settings.angle_penalty * yval;
            if (std::f64::consts::PI - turn).abs() < EPS {
                cost += 2.0 * cost_settings.segment_penalty;
            } else {
                cost += cost_settings.segment_penalty;
            }
        }
    }
    cost += route_crossing_count(current, next, crossing_routes) as f64
        * cost_settings.crossing_penalty;
    cost += route_shared_path_count(current, next, crossing_routes) as f64
        * cost_settings.fixed_shared_path_penalty;
    if segment_reverses_direction(current, next, src, dst) {
        cost += cost_settings.reverse_direction_penalty;
    }
    cost
}

fn simplify_polyline(path: Vec<Point>) -> Vec<Point> {
    let mut simplified = Vec::new();
    for point in path {
        simplified.push(point);
        while simplified.len() >= 3 {
            let len = simplified.len();
            let a = simplified[len - 3];
            let b = simplified[len - 2];
            let c = simplified[len - 1];
            if point_on_segment(b, a, c) {
                simplified.remove(len - 2);
            } else {
                break;
            }
        }
    }
    simplified
}

impl Ord for PolylineQueueState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.node.cmp(&other.node))
            .then_with(|| self.previous.cmp(&other.previous))
    }
}

impl PartialOrd for PolylineQueueState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PolylineQueueState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.node == other.node && self.previous == other.previous
    }
}

impl Eq for PolylineQueueState {}
