# libavoid Porting Inventory

<!-- GENERATED FILE: do not edit by hand.  Regenerate with `tools/audit/inventory.py`. -->

Function-level inventory of upstream libavoid (`vendor/adaptagrams/cola/libavoid`, excluding
`tests/` and `dllexport.h`) against the Rust port in `src/avoid/`.

* **Generated** by `python3 tools/audit/inventory.py`, which parses the upstream sources (own
  tokenizer, preprocessor and scope-tracking declaration parser), maps every item to Rust via the
  `// upstream: <file> <item>` comments (primary) or by name (secondary: camelCase -> snake_case `fn`,
  same-named type/const, operator -> std trait impl/derive), and applies the manual
  classifications in `tools/audit/classification.txt`.
* **Check** with `python3 tools/audit/inventory.py --check`: exits non-zero if a Rust `upstream:`
  comment names a nonexistent upstream file/item, if an upstream item is neither mapped nor
  classified, if a classification entry is stale, or if a classification contradicts a comment
  mapping.
* **Statuses**: `ported` (with mapping method), `not-ported: <category>` (`debug-output`, `timer`,
  `dead-code`, `declared-undefined`, `subsumed`, `not-applicable`; each with a reason), `MISSING`
  (genuinely absent from the port; see the reason column).
* Items in inactive preprocessor regions are listed with their condition, e.g. `dead (#if 0)`.
* Header declarations and .cpp definitions are merged; *Upstream* lists all locations, `(decl)`
  marks a declaration without body.

## Summary

| Upstream file | Items | Ported | NP: debug-output | NP: timer | NP: dead-code | NP: declared-undefined | NP: subsumed | NP: not-applicable | MISSING |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| [actioninfo.cpp/actioninfo.h](#actioninfo) | 18 | 17 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [assertions.h](#assertions) | 3 | 0 | 0 | 0 | 0 | 0 | 2 | 1 | 0 |
| [connectionpin.cpp/connectionpin.h](#connectionpin) | 34 | 33 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [connector.cpp/connector.h](#connector) | 91 | 88 | 0 | 0 | 0 | 0 | 3 | 0 | 0 |
| [connend.cpp/connend.h](#connend) | 28 | 26 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [debug.h](#debug) | 6 | 5 | 0 | 0 | 0 | 0 | 0 | 1 | 0 |
| [debughandler.h](#debughandler) | 12 | 10 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [geometry.cpp/geometry.h](#geometry) | 23 | 22 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| [geomtypes.cpp/geomtypes.h](#geomtypes) | 65 | 63 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [graph.cpp/graph.h](#graph) | 43 | 42 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [hyperedge.cpp/hyperedge.h](#hyperedge) | 24 | 23 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [hyperedgeimprover.cpp/hyperedgeimprover.h](#hyperedgeimprover) | 40 | 34 | 0 | 0 | 1 | 0 | 5 | 0 | 0 |
| [hyperedgetree.cpp/hyperedgetree.h](#hyperedgetree) | 40 | 39 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [junction.cpp/junction.h](#junction) | 15 | 14 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| [makepath.cpp/makepath.h](#makepath) | 39 | 33 | 0 | 0 | 0 | 0 | 6 | 0 | 0 |
| [mtst.cpp/mtst.h](#mtst) | 30 | 27 | 0 | 0 | 0 | 1 | 2 | 0 | 0 |
| [obstacle.cpp/obstacle.h](#obstacle) | 31 | 27 | 0 | 0 | 0 | 1 | 3 | 0 | 0 |
| [orthogonal.cpp/orthogonal.h](#orthogonal) | 99 | 89 | 0 | 0 | 0 | 2 | 8 | 0 | 0 |
| [router.cpp/router.h](#router) | 120 | 107 | 0 | 0 | 1 | 0 | 12 | 0 | 0 |
| [scanline.cpp/scanline.h](#scanline) | 35 | 31 | 0 | 0 | 0 | 0 | 4 | 0 | 0 |
| [shape.cpp/shape.h](#shape) | 13 | 11 | 0 | 0 | 0 | 1 | 1 | 0 | 0 |
| [timer.cpp/timer.h](#timer) | 23 | 23 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| [vertices.cpp/vertices.h](#vertices) | 74 | 71 | 0 | 0 | 0 | 0 | 3 | 0 | 0 |
| [viscluster.cpp/viscluster.h](#viscluster) | 11 | 9 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [visibility.cpp/visibility.h](#visibility) | 23 | 19 | 0 | 0 | 0 | 2 | 2 | 0 | 0 |
| [vpsc.cpp/vpsc.h](#vpsc) | 102 | 79 | 0 | 0 | 2 | 0 | 21 | 0 | 0 |
| **total** | **1042** | **942** | **0** | **0** | **5** | **7** | **86** | **2** | **0** |

Ported items by mapping method: `upstream:` comment 918, name match 19, manual mapping in `classification.txt` 5.

## Per-file inventory

<a id="actioninfo"></a>
### actioninfo

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ActionType: ShapeMove, ShapeAdd, ShapeRemove, JunctionMove, JunctionAdd, JunctionRemove, ConnChange, ConnectionPinChange` | enum | actioninfo.h:46 | ported (comment) | actioninfo.rs:38 `enum ActionType` |
| `ConnUpdateList` | typedef | actioninfo.h:57 | ported (comment) | actioninfo.rs:59 `type ConnUpdateList` |
| `ActionInfo` | class | actioninfo.h:59 | ported (comment) | actioninfo.rs:62 `struct ActionInfo` |
| `ActionInfo::ActionInfo(ActionType, ShapeRef *, const Polygon &, bool)` | ctor | actioninfo.h:61 (decl)<br>actioninfo.cpp:36 | ported (comment) | actioninfo.rs:98 `fn new_shape_move` |
| `ActionInfo::ActionInfo(ActionType, ShapeRef *)` | ctor | actioninfo.h:62 (decl)<br>actioninfo.cpp:46 | ported (comment) | actioninfo.rs:108 `fn new_shape` |
| `ActionInfo::ActionInfo(ActionType, JunctionRef *, const Point &)` | ctor | actioninfo.h:63 (decl)<br>actioninfo.cpp:56 | ported (comment) | actioninfo.rs:118 `fn new_junction_move` |
| `ActionInfo::ActionInfo(ActionType, JunctionRef *)` | ctor | actioninfo.h:64 (decl)<br>actioninfo.cpp:65 | ported (comment) | actioninfo.rs:127 `fn new_junction` |
| `ActionInfo::ActionInfo(ActionType, ConnRef *)` | ctor | actioninfo.h:65 (decl)<br>actioninfo.cpp:73 | ported (comment) | actioninfo.rs:139 `fn new_conn` |
| `ActionInfo::ActionInfo(ActionType, ShapeConnectionPin *)` | ctor | actioninfo.h:66 (decl)<br>actioninfo.cpp:81 | ported (comment) | actioninfo.rs:147 `fn new_pin` |
| `ActionInfo::~ActionInfo()` | dtor | actioninfo.h:67 (decl)<br>actioninfo.cpp:89 | not-ported: subsumed | empty destructor |
| `ActionInfo::obstacle() const` | method | actioninfo.h:68 (decl)<br>actioninfo.cpp:94 | ported (comment) | actioninfo.rs:154 `fn obstacle` |
| `ActionInfo::shape() const` | method | actioninfo.h:69 (decl)<br>actioninfo.cpp:103 | ported (comment) | actioninfo.rs:171 `fn shape` |
| `ActionInfo::conn() const` | method | actioninfo.h:70 (decl)<br>actioninfo.cpp:109 | ported (comment) | actioninfo.rs:180 `fn conn` |
| `ActionInfo::junction() const` | method | actioninfo.h:71 (decl)<br>actioninfo.cpp:115 | ported (comment) | actioninfo.rs:189 `fn junction` |
| `ActionInfo::addConnEndUpdate(const unsigned int, const ConnEnd &, bool)` | method | actioninfo.h:72 (decl)<br>actioninfo.cpp:121 | ported (comment) | actioninfo.rs:198 `fn add_conn_end_update` |
| `ActionInfo::operator==(const ActionInfo &) const` | operator | actioninfo.h:74 (decl)<br>actioninfo.cpp:155 | ported (comment) | actioninfo.rs:225 `fn equals` |
| `ActionInfo::operator<(const ActionInfo &) const` | operator | actioninfo.h:75 (decl)<br>actioninfo.cpp:161 | ported (comment) | actioninfo.rs:231 `fn less_than` |
| `ActionInfoList` | typedef | actioninfo.h:84 | ported (comment) | actioninfo.rs:251 `type ActionInfoList` |

<a id="assertions"></a>
### assertions

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `COLA_UNUSED(expr)` | macro | assertions.h:28 | not-ported: subsumed | unused-variable silencer; not needed in Rust |
| `COLA_ASSERT(expr)` | macro | assertions.h:32<br>assertions.h:43<br>assertions.h:52 | not-ported: subsumed | becomes `debug_assert!` (src/avoid/mod.rs conventions) |
| `NOMINMAX` | define — conditional (#ifdef _MSC_VER) | assertions.h:40 | not-ported: not-applicable | MSVC-only guard against windows.h min/max macros |

<a id="connectionpin"></a>
### connectionpin

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `CONNECTIONPIN_UNSET` | constant | connectionpin.h:44 | ported (comment) | connectionpin.rs:52 `const CONNECTIONPIN_UNSET` |
| `CONNECTIONPIN_CENTRE` | constant | connectionpin.h:45 | ported (comment) | connectionpin.rs:52 `const CONNECTIONPIN_UNSET` |
| `ConnectionPinIds` | typedef | connectionpin.h:53 | ported (comment) | connectionpin.rs:56 `type ConnectionPinIds` |
| `ATTACH_POS_TOP` | constant | connectionpin.h:57 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_CENTRE` | constant | connectionpin.h:58 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_BOTTOM` | constant | connectionpin.h:59 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_LEFT` | constant | connectionpin.h:60 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_RIGHT` | constant | connectionpin.h:61 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_MIN_OFFSET` | constant | connectionpin.h:63 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ATTACH_POS_MAX_OFFSET` | constant | connectionpin.h:64 | ported (comment) | connectionpin.rs:59 `const ATTACH_POS_TOP` |
| `ShapeConnectionPin` | class | connectionpin.h:96 | ported (comment) | connectionpin.rs:70 `struct ShapeConnectionPin` |
| `ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, const unsigned int, const double, const double, const bool, const double, const ConnDirFlags)` | ctor | connectionpin.h:169 (decl)<br>connectionpin.cpp:38 | ported (comment) | connectionpin.rs:97 `fn new_shape` |
| `ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, const unsigned int, const double, const double, const double, const ConnDirFlags)` | ctor | connectionpin.h:175 (decl)<br>connectionpin.cpp:57 | ported (comment) | connectionpin.rs:133 `fn new_shape_proportional` |
| `ShapeConnectionPin::ShapeConnectionPin(JunctionRef *, const unsigned int, const ConnDirFlags)` | ctor | connectionpin.h:197 (decl)<br>connectionpin.cpp:137 | ported (comment) | connectionpin.rs:223 `fn new_junction` |
| `ShapeConnectionPin::~ShapeConnectionPin()` | dtor | connectionpin.h:204 (decl)<br>connectionpin.cpp:171 | ported (comment) | connectionpin.rs:272 `fn delete` |
| `ShapeConnectionPin::setConnectionCost(const double)` | method | connectionpin.h:213 (decl)<br>connectionpin.cpp:215 | ported (comment) | connectionpin.rs:316 `fn set_connection_cost`<br>connectionpin.rs:634 `fn set_connection_cost` |
| `ShapeConnectionPin::position(const Polygon &) const` | method | connectionpin.h:219 (decl)<br>connectionpin.cpp:245 | ported (comment) | connectionpin.rs:349 `fn position`<br>connectionpin.rs:661 `fn position` |
| `ShapeConnectionPin::directions() const` | method | connectionpin.h:226 (decl)<br>connectionpin.cpp:331 | ported (comment) | connectionpin.rs:417 `fn directions`<br>connectionpin.rs:670 `fn directions` |
| `ShapeConnectionPin::setExclusive(const bool)` | method | connectionpin.h:236 (decl)<br>connectionpin.cpp:223 | ported (comment) | connectionpin.rs:323 `fn set_exclusive`<br>connectionpin.rs:643 `fn set_exclusive` |
| `ShapeConnectionPin::isExclusive() const` | method | connectionpin.h:243 (decl)<br>connectionpin.cpp:228 | ported (comment) | connectionpin.rs:328 `fn is_exclusive`<br>connectionpin.rs:652 `fn is_exclusive` |
| `ShapeConnectionPin::ids() const` | method | connectionpin.h:245 (decl)<br>connectionpin.cpp:388 | ported (comment) | connectionpin.rs:476 `fn ids`<br>connectionpin.rs:679 `fn ids` |
| `ShapeConnectionPin::operator==(const ShapeConnectionPin &) const` | operator | connectionpin.h:247 (decl)<br>connectionpin.cpp:399 | ported (comment) | connectionpin.rs:490 `fn equals` |
| `ShapeConnectionPin::operator<(const ShapeConnectionPin &) const` | operator | connectionpin.h:248 (decl)<br>connectionpin.cpp:432 | ported (comment) | connectionpin.rs:515 `fn less_than` |
| `ShapeConnectionPin::commonInitForShapeConnection()` | method | connectionpin.h:256 (decl)<br>connectionpin.cpp:77 | ported (comment) | connectionpin.rs:155 `fn common_init_for_shape_connection` |
| `ShapeConnectionPin::updatePosition(const Point &)` | method | connectionpin.h:257 (decl)<br>connectionpin.cpp:233 | ported (comment) | connectionpin.rs:333 `fn update_position_point` |
| `ShapeConnectionPin::updatePosition(const Polygon &)` | method | connectionpin.h:258 (decl)<br>connectionpin.cpp:239 | ported (comment) | connectionpin.rs:339 `fn update_position_poly` |
| `ShapeConnectionPin::updatePositionAndVisibility()` | method | connectionpin.h:259 (decl)<br>connectionpin.cpp:199 | ported (comment) | connectionpin.rs:296 `fn update_position_and_visibility` |
| `ShapeConnectionPin::updateVisibility()` | method | connectionpin.h:260 (decl)<br>connectionpin.cpp:206 | ported (comment) | connectionpin.rs:305 `fn update_visibility` |
| `ShapeConnectionPin::outputCode(FILE *) const` | method | connectionpin.h:261 (decl)<br>connectionpin.cpp:363 | ported (comment) | connectionpin.rs:443 `fn output_code` |
| `ShapeConnectionPin::containingObjectId() const` | method | connectionpin.h:262 (decl)<br>connectionpin.cpp:393 | ported (comment) | connectionpin.rs:481 `fn containing_object_id` |
| `CmpConnPinPtr` | class | connectionpin.h:283 | ported (comment) | connectionpin.rs:548 `fn cmp_conn_pin_ptr` |
| `CmpConnPinPtr::CmpConnPinPtr()` | ctor | connectionpin.h:286 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpConnPinPtr::operator()(const ShapeConnectionPin *, const ShapeConnectionPin *) const` | operator | connectionpin.h:289 | ported (manual) | connectionpin.rs fn cmp_conn_pin_ptr() (class and operator() ported as one fn) |
| `ShapeConnectionPinSet` | typedef | connectionpin.h:296 | ported (comment) | connectionpin.rs:562 `struct ShapeConnectionPinSet` |

<a id="connector"></a>
### connector

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ConnRefList` | typedef | connector.h:48 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `ConnType: ConnType_None, ConnType_PolyLine, ConnType_Orthogonal` | enum | connector.h:53 | ported (comment) | connector.rs:65 `enum ConnType` |
| `Checkpoint` | class | connector.h:68 | ported (comment) | connector.rs:75 `struct Checkpoint` |
| `Checkpoint::Checkpoint(const Point &)` | ctor | connector.h:77 | ported (comment) | connector.rs:96 `fn new` |
| `Checkpoint::Checkpoint(const Point &, ConnDirFlags, ConnDirFlags)` | ctor | connector.h:95 | ported (comment) | connector.rs:105 `fn with_directions` |
| `Checkpoint::Checkpoint()` | ctor | connector.h:102 | ported (comment) | connector.rs:84 `fn default` |
| `ConnRef` | class | connector.h:131 | ported (comment) | connector.rs:126 `struct ConnRef` |
| `ConnRef::ConnRef(Router *, const unsigned int)` | ctor | connector.h:155 (decl)<br>connector.cpp:48 | ported (comment) | connector.rs:197 `fn new` |
| `ConnRef::ConnRef(Router *, const ConnEnd &, const ConnEnd &, const unsigned int)` | ctor | connector.h:175 (decl)<br>connector.cpp:77 | ported (comment) | connector.rs:215 `fn new_with_ends` |
| `ConnRef::~ConnRef()` | dtor | connector.h:187 (decl)<br>connector.cpp:107 | ported (comment) | connector.rs:239 `fn delete` |
| `ConnRef::setEndpoints(const ConnEnd &, const ConnEnd &)` | method | connector.h:198 (decl)<br>connector.cpp:323 | ported (comment) | connector.rs:438 `fn set_endpoints`<br>connector.rs:1446 `fn set_endpoints` |
| `ConnRef::setSourceEndpoint(const ConnEnd &)` | method | connector.h:207 (decl)<br>connector.cpp:336 | ported (comment) | connector.rs:449 `fn set_source_endpoint`<br>connector.rs:1455 `fn set_source_endpoint` |
| `ConnRef::setDestEndpoint(const ConnEnd &)` | method | connector.h:216 (decl)<br>connector.cpp:342 | ported (comment) | connector.rs:454 `fn set_dest_endpoint`<br>connector.rs:1464 `fn set_dest_endpoint` |
| `ConnRef::id() const` | method | connector.h:220 (decl)<br>connector.cpp:677 | ported (comment) | connector.rs:795 `fn id`<br>connector.rs:1482 `fn id` |
| `ConnRef::router() const` | method | connector.h:224 (decl)<br>connector.cpp:797 | not-ported: subsumed | m_router back-pointer getter; the port passes &Router explicitly (router.rs data model) |
| `ConnRef::needsRepaint() const` | method | connector.h:235 (decl)<br>connector.cpp:671 | ported (comment) | connector.rs:789 `fn needs_repaint`<br>connector.rs:1491 `fn needs_repaint` |
| `ConnRef::route() const` | method | connector.h:248 (decl)<br>connector.cpp:590 | ported (comment) | connector.rs:705 `fn route`<br>connector.rs:1500 `fn route` |
| `ConnRef::displayRoute()` | method | connector.h:259 (decl)<br>connector.cpp:647 | ported (comment) | connector.rs:766 `fn display_route`<br>connector.rs:1509 `fn display_route` |
| `ConnRef::setCallback(void(* cb)(void *), void *)` | method | connector.h:271 (decl)<br>connector.cpp:775 | ported (comment) | connector.rs:898 `fn set_callback`<br>connector.rs:1518 `fn set_callback` |
| `ConnRef::routingType() const` | method | connector.h:276 (decl)<br>connector.cpp:170 | ported (comment) | connector.rs:293 `fn routing_type`<br>connector.rs:1522 `fn routing_type` |
| `ConnRef::setRoutingType(ConnType)` | method | connector.h:288 (decl)<br>connector.cpp:176 | ported (comment) | connector.rs:299 `fn set_routing_type`<br>connector.rs:1531 `fn set_routing_type` |
| `ConnRef::splitAtSegment(const size_t)` | method | connector.h:301 (decl)<br>connector.cpp:694 | ported (comment) | connector.rs:803 `fn split_at_segment`<br>connector.rs:1540 `fn split_at_segment` |
| `ConnRef::setRoutingCheckpoints(const std::vector<Checkpoint> &)` | method | connector.h:315 (decl)<br>connector.cpp:196 | ported (comment) | connector.rs:316 `fn set_routing_checkpoints`<br>connector.rs:1549 `fn set_routing_checkpoints` |
| `ConnRef::routingCheckpoints() const` | method | connector.h:321 (decl)<br>connector.cpp:190 | ported (comment) | connector.rs:311 `fn routing_checkpoints`<br>connector.rs:1558 `fn routing_checkpoints` |
| `ConnRef::endpointConnEnds() const` | method | connector.h:340 (decl)<br>connector.cpp:511 | ported (comment) | connector.rs:620 `fn endpoint_conn_ends`<br>connector.rs:1567 `fn endpoint_conn_ends` |
| `ConnRef::src() const` | method | connector.h:344 (decl)<br>connector.cpp:729 | ported (comment) | connector.rs:855 `fn src`<br>connector.rs:1576 `fn src` |
| `ConnRef::dst() const` | method | connector.h:348 (decl)<br>connector.cpp:735 | ported (comment) | connector.rs:861 `fn dst`<br>connector.rs:1585 `fn dst` |
| `ConnRef::setFixedRoute(const PolyLine &)` | method | connector.h:368 (decl)<br>connector.cpp:621 | ported (comment) | connector.rs:735 `fn set_fixed_route`<br>connector.rs:1594 `fn set_fixed_route` |
| `ConnRef::setFixedExistingRoute()` | method | connector.h:384 (decl)<br>connector.cpp:614 | ported (comment) | connector.rs:725 `fn set_fixed_existing_route`<br>connector.rs:1603 `fn set_fixed_existing_route` |
| `ConnRef::hasFixedRoute() const` | method | connector.h:390 (decl)<br>connector.cpp:635 | ported (comment) | connector.rs:751 `fn has_fixed_route`<br>connector.rs:1612 `fn has_fixed_route` |
| `ConnRef::clearFixedRoute()` | method | connector.h:397 (decl)<br>connector.cpp:640 | ported (comment) | connector.rs:757 `fn clear_fixed_route`<br>connector.rs:1621 `fn clear_fixed_route` |
| `ConnRef::set_route(const PolyLine &)` | method | connector.h:399 (decl)<br>connector.cpp:602 | ported (comment) | connector.rs:717 `fn set_route` |
| `ConnRef::calcRouteDist()` | method | connector.h:400 (decl)<br>connector.cpp:658 | ported (comment) | connector.rs:775 `fn calc_route_dist` |
| `ConnRef::makeActive()` | method | connector.h:401 (decl)<br>connector.cpp:550 | ported (comment) | connector.rs:664 `fn make_active` |
| `ConnRef::makeInactive()` | method | connector.h:402 (decl)<br>connector.cpp:573 | ported (comment) | connector.rs:683 `fn make_inactive` |
| `ConnRef::start()` | method | connector.h:403 (decl)<br>connector.cpp:741 | ported (comment) | connector.rs:867 `fn start` |
| `ConnRef::removeFromGraph()` | method | connector.h:404 (decl)<br>connector.cpp:761 | ported (comment) | connector.rs:887 `fn remove_from_graph` |
| `ConnRef::isInitialised() const` | method | connector.h:405 (decl)<br>connector.cpp:747 | ported (comment) | connector.rs:873 `fn is_initialised` |
| `ConnRef::makePathInvalid()` | method | connector.h:406 (decl)<br>connector.cpp:791 | ported (comment) | connector.rs:912 `fn make_path_invalid` |
| `ConnRef::setHateCrossings(bool)` | method | connector.h:407 (decl)<br>connector.cpp:1302 | ported (comment) | connector.rs:1326 `fn set_hate_crossings`<br>connector.rs:1630 `fn set_hate_crossings` |
| `ConnRef::doesHateCrossings() const` | method | connector.h:408 (decl)<br>connector.cpp:1308 | ported (comment) | connector.rs:1332 `fn does_hate_crossings`<br>connector.rs:1639 `fn does_hate_crossings` |
| `ConnRef::setEndpoint(const unsigned int, const ConnEnd &)` | method | connector.h:409 (decl)<br>connector.cpp:330 | ported (comment) | connector.rs:444 `fn set_endpoint`<br>connector.rs:1473 `fn set_endpoint` |
| `ConnRef::setEndpoint(const unsigned int, const VertID &, Point *)` | method | connector.h:410 (decl)<br>connector.cpp:519 | ported (comment) | connector.rs:628 `fn set_endpoint_vertex`<br>connector.rs:1657 `fn set_endpoint_vertex` |
| `ConnRef::possibleDstPinPoints() const` | method | connector.h:412 (decl)<br>connector.cpp:1314 | ported (comment) | connector.rs:1338 `fn possible_dst_pin_points`<br>connector.rs:1648 `fn possible_dst_pin_points` |
| `ConnRef::routeRef()` | method | connector.h:424 (decl)<br>connector.cpp:596 | ported (comment) | connector.rs:711 `fn route_ref` |
| `ConnRef::freeRoutes()` | method | connector.h:425 (decl)<br>connector.cpp:583 | ported (comment) | connector.rs:697 `fn free_routes` |
| `ConnRef::performCallback()` | method | connector.h:426 (decl)<br>connector.cpp:782 | ported (comment) | connector.rs:904 `fn perform_callback` |
| `ConnRef::generatePath()` | method | connector.h:427 (decl)<br>connector.cpp:923 | ported (comment) | connector.rs:947 `fn generate_path` |
| `ConnRef::generateCheckpointsPath(std::vector<Point> &, std::vector<VertInf *> &)` | method | connector.h:428 (decl)<br>connector.cpp:1059 | ported (comment) | connector.rs:1073 `fn generate_checkpoints_path` |
| `ConnRef::generateStandardPath(std::vector<Point> &, std::vector<VertInf *> &)` | method | connector.h:430 (decl)<br>connector.cpp:1163 | ported (comment) | connector.rs:1178 `fn generate_standard_path` |
| `ConnRef::unInitialise()` | method | connector.h:432 (decl)<br>connector.cpp:753 | ported (comment) | connector.rs:879 `fn un_initialise` |
| `ConnRef::updateEndPoint(const unsigned int, const ConnEnd &)` | method | connector.h:433 (decl)<br>connector.cpp:393 | ported (comment) | connector.rs:501 `fn update_end_point` |
| `ConnRef::common_updateEndPoint(const unsigned int, ConnEnd)` | method | connector.h:434 (decl)<br>connector.cpp:228 | ported (comment) | connector.rs:351 `fn common_update_end_point` |
| `ConnRef::freeActivePins()` | method | connector.h:435 (decl)<br>connector.cpp:560 | ported (comment) | connector.rs:673 `fn free_active_pins` |
| `ConnRef::getConnEndForEndpointVertex(VertInf *, ConnEnd &) const` | method | connector.h:436 (decl)<br>connector.cpp:351 | ported (comment) | connector.rs:463 `fn get_conn_end_for_endpoint_vertex` |
| `ConnRef::endpointAnchors() const` | method | connector.h:438 (decl)<br>connector.cpp:494 | ported (comment) | connector.rs:606 `fn endpoint_anchors` |
| `ConnRef::outputCode(FILE *) const` | method | connector.h:439 (decl)<br>connector.cpp:429 | ported (comment) | connector.rs:534 `fn output_code` |
| `ConnRef::assignConnectionPinVisibility(const bool)` | method | connector.h:440 (decl)<br>connector.cpp:895 | ported (comment) | connector.rs:918 `fn assign_connection_pin_visibility` |
| `PtConnPtrPair` | typedef | connector.h:470 | ported (comment) | connector.rs:1732 `struct PtConnPtrPair` |
| `PointRepVector` | typedef | connector.h:472 | ported (comment) | connector.rs:1745 `type PointRepVector` |
| `NodeIndexPairLinkList` | typedef | connector.h:473 | ported (comment) | connector.rs:1747 `type NodeIndexPairLinkList` |
| `PtOrder` | class | connector.h:475 | ported (comment) | connector.rs:1752 `struct PtOrder` |
| `PtOrder::PtOrder()` | ctor | connector.h:478 (decl)<br>connector.cpp:1325 | ported (comment) | connector.rs:1763 `fn new` |
| `PtOrder::~PtOrder()` | dtor | connector.h:479 (decl)<br>connector.cpp:1335 | not-ported: subsumed | empty destructor |
| `PtOrder::addPoints(const size_t, const PtConnPtrPair &, const PtConnPtrPair &)` | method | connector.h:480 (decl)<br>connector.cpp:1388 | ported (comment) | connector.rs:1808 `fn add_points` |
| `PtOrder::addOrderedPoints(const size_t, const PtConnPtrPair &, const PtConnPtrPair &, bool)` | method | connector.h:482 (decl)<br>connector.cpp:1397 | ported (comment) | connector.rs:1815 `fn add_ordered_points` |
| `PtOrder::positionFor(const size_t, const ConnRef *)` | method | connector.h:484 (decl)<br>connector.cpp:1351 | ported (comment) | connector.rs:1779 `fn position_for` |
| `PtOrder::sortedPoints(const size_t)` | method | connector.h:485 (decl)<br>connector.cpp:1340 | ported (comment) | connector.rs:1769 `fn sorted_points` |
| `PtOrder::insertPoint(const size_t, const PtConnPtrPair &)` | method | connector.h:487 (decl)<br>connector.cpp:1372 | ported (comment) | connector.rs:1795 `fn insert_point` |
| `PtOrder::sort(const size_t)` | method | connector.h:488 (decl)<br>connector.cpp:1413 | ported (comment) | connector.rs:1835 `fn sort` |
| `PtOrderMap` | typedef | connector.h:497 | ported (comment) | connector.rs:1689 `type PtOrderMap` |
| `PointSet` | typedef | connector.h:498 | ported (comment) | connector.rs:1687 `type PointSet` |
| `CROSSING_NONE` | constant | connector.h:501 | ported (comment) | connector.rs:1674 `const CROSSING_NONE` |
| `CROSSING_TOUCHES` | constant | connector.h:502 | ported (comment) | connector.rs:1674 `const CROSSING_NONE` |
| `CROSSING_SHARES_PATH` | constant | connector.h:503 | ported (comment) | connector.rs:1674 `const CROSSING_NONE` |
| `CROSSING_SHARES_PATH_AT_END` | constant | connector.h:504 | ported (comment) | connector.rs:1674 `const CROSSING_NONE` |
| `CROSSING_SHARES_FIXED_SEGMENT` | constant | connector.h:505 | ported (comment) | connector.rs:1674 `const CROSSING_NONE` |
| `CrossingsInfoPair` | typedef | connector.h:508 | ported (comment) | connector.rs:1681 `type CrossingsInfoPair` |
| `PointList` | typedef | connector.h:509 | ported (comment) | connector.rs:1683 `type PointList` |
| `SharedPathList` | typedef | connector.h:510 | ported (comment) | connector.rs:1685 `type SharedPathList` |
| `ConnectorCrossings` | class | connector.h:512 | ported (comment) | connector.rs:2155 `struct ConnectorCrossings` |
| `ConnectorCrossings::ConnectorCrossings(Avoid::Polygon &, bool, Avoid::Polygon &, ConnRef *, ConnRef *)` | ctor | connector.h:515 (decl)<br>connector.cpp:1696 | ported (comment) | connector.rs:2175 `fn new` |
| `ConnectorCrossings::clear()` | method | connector.h:518 (decl)<br>connector.cpp:1710 | ported (comment) | connector.rs:2196 `fn clear` |
| `ConnectorCrossings::countForSegment(size_t, const bool)` | method | connector.h:519 (decl)<br>connector.cpp:1748 | ported (comment) | connector.rs:2215 `fn count_for_segment` |
| `splitBranchingSegments(Avoid::Polygon &, bool, Avoid::Polygon &, const double)` | function | connector.h:538 (decl)<br>connector.cpp:1572 | ported (comment) | connector.rs:1966 `fn split_branching_segments` |
| `validateBendPoint(VertInf *, VertInf *, VertInf *)` | function | connector.h:540 (decl)<br>connector.cpp:807 | ported (comment) | connector.rs:1351 `fn validate_bend_point` |
| `midpoint(Point, Point)` | function | connector.cpp:683 | ported (comment) | connector.rs:157 `fn midpoint` |
| `midVertexNumber(const Point &, const Point &, const Point &)` | function (static) | connector.cpp:1488 | ported (comment) | connector.rs:1894 `fn mid_vertex_number` |
| `segDir(const Point &, const Point &)` | function (static) | connector.cpp:1653 | ported (comment) | connector.rs:2086 `fn seg_dir` |
| `posInlineWithConnEndSegs(const double, const size_t, const Avoid::Polygon &, const Avoid::Polygon &)` | function (static) | connector.cpp:1674 | ported (comment) | connector.rs:2101 `fn pos_inline_with_conn_end_segs` |
| `pathLength(Avoid::Point **, Avoid::Point **, size_t)` | function (static) | connector.cpp:1721 | ported (comment) | connector.rs:2123 `fn path_length` |

<a id="connend"></a>
### connend

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ConnRefList` | typedef | connend.h:50 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `ConnDirFlag: ConnDirNone, ConnDirUp, ConnDirDown, ConnDirLeft, ConnDirRight, ConnDirAll` | enum | connend.h:62 | ported (comment) | connend.rs:49<br>vertices.rs:55 `const CONN_DIR_NONE` |
| `ConnDirFlags` | typedef | connend.h:83 | ported (comment) | connend.rs:49 |
| `ConnEndType: ConnEndPoint, ConnEndShapePin, ConnEndJunction, ConnEndEmpty` | enum | connend.h:89 | ported (comment) | connend.rs:56 `enum ConnEndType` |
| `ConnEnd` | class | connend.h:110 | ported (comment) | connend.rs:108 `struct ConnEnd` |
| `ConnEnd::ConnEnd(const Point &)` | ctor | connend.h:117 (decl)<br>connend.cpp:55 | ported (comment) | connend.rs:149 `fn from_point` |
| `ConnEnd::ConnEnd(const Point &, const ConnDirFlags)` | ctor | connend.h:130 (decl)<br>connend.cpp:67 | ported (comment) | connend.rs:165 `fn from_point_dirs` |
| `ConnEnd::ConnEnd(ShapeRef *, const unsigned int)` | ctor | connend.h:162 (decl)<br>connend.cpp:79 | ported (comment) | connend.rs:181 `fn from_shape_pin` |
| `ConnEnd::ConnEnd(JunctionRef *)` | ctor | connend.h:174 (decl)<br>connend.cpp:96 | ported (comment) | connend.rs:209 `fn from_junction` |
| `ConnEnd::type() const` | method | connend.h:181 (decl)<br>connend.cpp:114 | ported (comment) | connend.rs:229 `fn type_` |
| `ConnEnd::position() const` | method | connend.h:187 (decl)<br>connend.cpp:120 | ported (comment) | connend.rs:236 `fn position` |
| `ConnEnd::directions() const` | method | connend.h:194 (decl)<br>connend.cpp:137 | ported (comment) | connend.rs:273 `fn directions` |
| `ConnEnd::shape() const` | method | connend.h:202 (decl)<br>connend.cpp:150 | ported (comment) | connend.rs:284 `fn shape` |
| `ConnEnd::junction() const` | method | connend.h:211 (decl)<br>connend.cpp:156 | ported (comment) | connend.rs:294 `fn junction` |
| `ConnEnd::pinClassId() const` | method | connend.h:220 (decl)<br>connend.cpp:162 | ported (comment) | connend.rs:303 `fn pin_class_id` |
| `ConnEnd::ConnEnd()` | ctor | connend.h:222 (decl)<br>connend.cpp:44 | ported (comment) | connend.rs:134 `fn new` |
| `ConnEnd::~ConnEnd()` | dtor | connend.h:223 (decl)<br>connend.cpp:109 | not-ported: subsumed | empty destructor |
| `ConnEnd::connect(ConnRef *)` | method | connend.h:235 (decl)<br>connend.cpp:237 | ported (comment) | connend.rs:388 `fn connect` |
| `ConnEnd::disconnect(const bool)` | method | connend.h:236 (decl)<br>connend.cpp:249 | ported (comment) | connend.rs:403 `fn disconnect` |
| `ConnEnd::usePin(ShapeConnectionPin *)` | method | connend.h:237 (decl)<br>connend.cpp:182 | ported (comment) | connend.rs:332 `fn use_pin` |
| `ConnEnd::usePinVertex(VertInf *)` | method | connend.h:238 (decl)<br>connend.cpp:195 | ported (comment) | connend.rs:345 `fn use_pin_vertex` |
| `ConnEnd::freeActivePin()` | method | connend.h:239 (decl)<br>connend.cpp:226 | ported (comment) | connend.rs:378 `fn free_active_pin` |
| `ConnEnd::endpointType() const` | method | connend.h:240 (decl)<br>connend.cpp:174 | ported (comment) | connend.rs:317 `fn endpoint_type` |
| `ConnEnd::isPinConnection() const` | method | connend.h:241 (decl)<br>connend.cpp:169 | ported (comment) | connend.rs:309 `fn is_pin_connection` |
| `ConnEnd::possiblePinPoints() const` | method | connend.h:242 (decl)<br>connend.cpp:214 | ported (comment) | connend.rs:366 `fn possible_pin_points` |
| `ConnEnd::assignPinVisibilityTo(VertInf *, VertInf *)` | method | connend.h:243 (decl)<br>connend.cpp:274 | ported (comment) | connend.rs:431 `fn assign_pin_visibility_to` |
| `ConnEnd::outputCode(FILE *, const char *) const` | method | connend.h:245 (decl)<br>connend.cpp:411 | ported (comment) | connend.rs:574 `fn output_code` |
| `ConnEnd::getHyperedgeVertex(Router *) const` | method | connend.h:246 (decl)<br>connend.cpp:373 | ported (comment) | connend.rs:541 `fn get_hyperedge_vertex` |

<a id="debug"></a>
### debug

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `NOMINMAX` | define — conditional (#if defined(_MSC_VER) && defined(USE_ATLTRACE)) | debug.h:35 | not-ported: not-applicable | MSVC-only guard against windows.h min/max macros |
| `db_printf` | define — conditional (#ifdef LIBAVOID_DEBUG) | debug.h:51 | ported (comment) | debug.rs:46 `macro_rules! db_printf` |
| `db_printf(const char *, ...)` | function (inline) | debug.h:53<br>debug.h:64 | ported (comment) | debug.rs:47 `macro_rules! db_printf` |
| `err_printf` | define — conditional (#if defined(_MSC_VER) && defined(USE_ATLTRACE)) | debug.h:73 | ported (comment) | debug.rs:61 `macro_rules! err_printf` |
| `err_printf(const char *, ...)` | function (inline) | debug.h:78 | ported (comment) | debug.rs:62 `macro_rules! err_printf` |
| `PREC` | define | debug.h:93<br>debug.h:95 | ported (comment) | debug.rs:33 `const PREC`<br>debug.rs:36 `const PREC` |

<a id="debughandler"></a>
### debughandler

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `DEBUGHANDLER` | define | debughandler.h:36 | ported (manual) | Cargo feature `debug-handler` (Cargo.toml, off by default; debughandler.rs module docs) |
| `DebugHandler` | class | debughandler.h:46 | ported (comment) | debughandler.rs:28 `trait DebugHandler` |
| `DebugHandler::DebugHandler()` | ctor | debughandler.h:49 | not-ported: subsumed | empty ctor; DebugHandler is a Rust trait (debughandler.rs) |
| `DebugHandler::~DebugHandler()` | dtor (virtual) | debughandler.h:53 | not-ported: subsumed | empty virtual dtor; DebugHandler is a Rust trait (debughandler.rs) |
| `DebugHandler::updateObstacleBoxes(std::vector<Avoid::Box>)` | method (virtual) | debughandler.h:59 | ported (comment) | debughandler.rs:31 `fn update_obstacle_boxes` |
| `DebugHandler::updateConnectorRoute(ConnRef *, int, int)` | method (virtual) | debughandler.h:68 | ported (comment) | debughandler.rs:38 `fn update_connector_route` |
| `DebugHandler::beginningSearchWithEndpoints(VertInf *, VertInf *)` | method (virtual) | debughandler.h:78 | ported (comment) | debughandler.rs:45 `fn beginning_search_with_endpoints` |
| `DebugHandler::updateCurrentSearchPath(Avoid::PolyLine)` | method (virtual) | debughandler.h:86 | ported (comment) | debughandler.rs:51 `fn update_current_search_path` |
| `DebugHandler::beginningHyperedgeReroutingWithEndpoints(std::set<VertInf *>)` | method (virtual) | debughandler.h:93 | ported (comment) | debughandler.rs:57 `fn beginning_hyperedge_rerouting_with_endpoints` |
| `DebugHandler::mtstGrowForestWithEdge(Avoid::VertInf *, Avoid::VertInf *, bool)` | method (virtual) | debughandler.h:106 | ported (comment) | debughandler.rs:66 `fn mtst_grow_forest_with_edge` |
| `DebugHandler::mtstPotentialBridgingEdge(Avoid::VertInf *, Avoid::VertInf *)` | method (virtual) | debughandler.h:116 | ported (comment) | debughandler.rs:73 `fn mtst_potential_bridging_edge` |
| `DebugHandler::mtstCommitToEdge(Avoid::VertInf *, Avoid::VertInf *, bool)` | method (virtual) | debughandler.h:127 | ported (comment) | debughandler.rs:81 `fn mtst_commit_to_edge` |

<a id="geometry"></a>
### geometry

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `euclideanDist(const Point &, const Point &)` | function | geometry.h:45 (decl)<br>geometry.cpp:294 | ported (comment) | geometry.rs:218 `fn euclidean_dist` |
| `manhattanDist(const Point &, const Point &)` | function | geometry.h:46 (decl)<br>geometry.cpp:304 | ported (comment) | geometry.rs:228 `fn manhattan_dist` |
| `totalLength(const Polygon &)` | function | geometry.h:47 (decl)<br>geometry.cpp:321 | ported (comment) | geometry.rs:245 `fn total_length` |
| `angle(const Point &, const Point &, const Point &)` | function | geometry.h:48 (decl)<br>geometry.cpp:332 | ported (comment) | geometry.rs:255 `fn angle` |
| `segmentIntersect(const Point &, const Point &, const Point &, const Point &)` | function | geometry.h:49 (decl)<br>geometry.cpp:134 | ported (comment) | geometry.rs:110 `fn segment_intersect` |
| `segmentShapeIntersect(const Point &, const Point &, const Point &, const Point &, bool &)` | function | geometry.h:51 (decl)<br>geometry.cpp:168 | ported (comment) | geometry.rs:135 `fn segment_shape_intersect` |
| `inPoly(const Polygon &, const Point &, bool)` | function | geometry.h:53 (decl)<br>geometry.cpp:351 | ported (comment) | geometry.rs:271 `fn in_poly` |
| `inPolyGen(const PolygonInterface &, const Point &)` | function | geometry.h:54 (decl)<br>geometry.cpp:382 | ported (comment) | geometry.rs:294 `fn in_poly_gen` |
| `inValidRegion(bool, const Point &, const Point &, const Point &, const Point &)` | function | geometry.h:55 (decl)<br>geometry.cpp:201 | ported (comment) | geometry.rs:163 `fn in_valid_region` |
| `cornerSide(const Point &, const Point &, const Point &, const Point &)` | function | geometry.h:57 (decl)<br>geometry.cpp:263 | ported (comment) | geometry.rs:195 `fn corner_side` |
| `pointOnLine(const Point &, const Point &, const Point &, const double)` | function | geometry.h:59 (decl)<br>geometry.cpp:107 | ported (comment) | geometry.rs:95 `fn point_on_line` |
| `colinear(const Point &, const Point &, const Point &, const double)` | function | geometry.h:61 (decl)<br>geometry.cpp:81 | ported (comment) | geometry.rs:78 `fn colinear` |
| `inBetween(const Point &, const Point &, const Point &)` | function | geometry.h:64 (decl)<br>geometry.cpp:57 | ported (comment) | geometry.rs:60 `fn in_between` |
| `vecDir(const Point &, const Point &, const Point &, const double)` | function (static, inline) | geometry.h:79 | ported (comment) | geometry.rs:27 `fn vec_dir` |
| `projection(const Point &, const Point &, const Point &)` | function (static, inline) | geometry.h:99 | ported (comment) | geometry.rs:43 `fn projection` |
| `DONT_INTERSECT` | constant | geometry.h:116 | ported (comment) | geometry.rs:19 `const DONT_INTERSECT` |
| `DO_INTERSECT` | constant | geometry.h:117 | ported (comment) | geometry.rs:19 `const DONT_INTERSECT` |
| `PARALLEL` | constant | geometry.h:118 | ported (comment) | geometry.rs:19 `const DONT_INTERSECT` |
| `segmentIntersectPoint(const Point &, const Point &, const Point &, const Point &, double *, double *)` | function | geometry.h:119 (decl)<br>geometry.cpp:478 | ported (comment) | geometry.rs:361 `fn segment_intersect_point` |
| `rayIntersectPoint(const Point &, const Point &, const Point &, const Point &, double *, double *)` | function | geometry.h:121 (decl)<br>geometry.cpp:578 | ported (comment) | geometry.rs:458 `fn ray_intersect_point` |
| `rotationalAngle(const Point &)` | function | geometry.h:123 (decl)<br>geometry.cpp:613 | ported (comment) | geometry.rs:498 `fn rotational_angle` |
| `dist(const Point &, const Point &)` | function | geometry.cpp:312 | ported (comment) | geometry.rs:235 `fn dist` |
| `SAME_SIGNS( a, b )` | macro | geometry.cpp:475 | not-ported: dead-code | macro defined but never used upstream |

<a id="geomtypes"></a>
### geomtypes

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `XDIM` | constant | geomtypes.h:42 | ported (comment) | geomtypes.rs:35 `const XDIM` |
| `YDIM` | constant | geomtypes.h:43 | ported (comment) | geomtypes.rs:35 `const XDIM` |
| `Point` | class | geomtypes.h:52 | ported (comment) | geomtypes.rs:68 `struct Point` |
| `Point::Point()` | ctor | geomtypes.h:57 (decl)<br>geomtypes.cpp:41 | ported (comment) | geomtypes.rs:84 `fn default` |
| `Point::Point(const double, const double)` | ctor | geomtypes.h:63 (decl)<br>geomtypes.cpp:48 | ported (comment) | geomtypes.rs:103 `fn new` |
| `Point::operator==(const Point &) const` | operator | geomtypes.h:70 (decl)<br>geomtypes.cpp:57 | ported (comment) | geomtypes.rs:140 `fn eq` |
| `Point::equals(const Point &, double) const` | method | geomtypes.h:79 (decl)<br>geomtypes.cpp:77 | ported (comment) | geomtypes.rs:114 `fn equals` |
| `Point::operator!=(const Point &) const` | operator | geomtypes.h:85 (decl)<br>geomtypes.cpp:67 | ported (comment) | geomtypes.rs:149 `fn ne` |
| `Point::operator<(const Point &) const` | operator | geomtypes.h:94 (decl)<br>geomtypes.cpp:88 | ported (comment) | geomtypes.rs:129 `fn less_than` |
| `Point::operator[](const size_t)` | operator | geomtypes.h:100 (decl)<br>geomtypes.cpp:98 | ported (comment) | geomtypes.rs:175 `fn index_mut` |
| `Point::operator[](const size_t) const` | operator | geomtypes.h:101 (decl)<br>geomtypes.cpp:105 | ported (comment) | geomtypes.rs:162 `fn index` |
| `Point::operator+(const Point &) const` | operator | geomtypes.h:103 (decl)<br>geomtypes.cpp:111 | ported (comment) | geomtypes.rs:189 `fn add` |
| `Point::operator-(const Point &) const` | operator | geomtypes.h:104 (decl)<br>geomtypes.cpp:117 | ported (comment) | geomtypes.rs:198 `fn sub` |
| `kUnassignedVertexNumber` | constant | geomtypes.h:120 | ported (comment) | geomtypes.rs:40 `const K_UNASSIGNED_VERTEX_NUMBER` |
| `kShapeConnectionPin` | constant | geomtypes.h:123 | ported (comment) | geomtypes.rs:44 `const K_SHAPE_CONNECTION_PIN` |
| `Vector` | typedef | geomtypes.h:128 | ported (comment) | geomtypes.rs:80 `type Vector` |
| `Box` | class | geomtypes.h:133 | ported (comment) | geomtypes.rs:235 `struct Box` |
| `Box::length(size_t) const` | method | geomtypes.h:141 (decl)<br>geomtypes.cpp:235 | ported (comment) | geomtypes.rs:243 `fn length` |
| `Box::width() const` | method | geomtypes.h:142 (decl)<br>geomtypes.cpp:244 | ported (comment) | geomtypes.rs:251 `fn width` |
| `Box::height() const` | method | geomtypes.h:143 (decl)<br>geomtypes.cpp:249 | ported (comment) | geomtypes.rs:256 `fn height` |
| `PolygonInterface` | class | geomtypes.h:150 | ported (comment) | geomtypes.rs:271 `trait PolygonInterface` |
| `PolygonInterface::PolygonInterface()` | ctor | geomtypes.h:154 | not-ported: subsumed | empty ctor of abstract base; PolygonInterface is a Rust trait |
| `PolygonInterface::~PolygonInterface()` | dtor (virtual) | geomtypes.h:156 | not-ported: subsumed | empty virtual dtor of abstract base; PolygonInterface is a Rust trait |
| `PolygonInterface::clear()` | method (virtual, pure) | geomtypes.h:158 (decl) | ported (name) | geomtypes.rs:274 |
| `PolygonInterface::empty() const` | method (virtual, pure) | geomtypes.h:160 (decl) | ported (name) | geomtypes.rs:276 |
| `PolygonInterface::size() const` | method (virtual, pure) | geomtypes.h:162 (decl) | ported (name) | geomtypes.rs:278 |
| `PolygonInterface::id() const` | method (virtual, pure) | geomtypes.h:164 (decl) | ported (name) | geomtypes.rs:280 |
| `PolygonInterface::at(size_t) const` | method (virtual, pure) | geomtypes.h:167 (decl) | ported (name) | geomtypes.rs:282 |
| `PolygonInterface::boundingRectPolygon() const` | method | geomtypes.h:171 (decl)<br>geomtypes.cpp:282 | ported (comment) | geomtypes.rs:284 `fn bounding_rect_polygon` |
| `PolygonInterface::offsetBoundingBox(double) const` | method | geomtypes.h:180 (decl)<br>geomtypes.cpp:210 | ported (comment) | geomtypes.rs:290 `fn offset_bounding_box` |
| `PolygonInterface::offsetPolygon(double) const` | method | geomtypes.h:182 (decl)<br>geomtypes.cpp:303 | ported (comment) | geomtypes.rs:314 `fn offset_polygon` |
| `Edge` | class | geomtypes.h:188 | ported (comment) | geomtypes.rs:263 `struct Edge` |
| `Polygon` | class | geomtypes.h:207 | ported (comment) | geomtypes.rs:398 `struct Polygon` |
| `Polygon::Polygon()` | ctor | geomtypes.h:211 (decl)<br>geomtypes.cpp:254 | ported (comment) | geomtypes.rs:566 `fn new` |
| `Polygon::Polygon(const int)` | ctor | geomtypes.h:226 (decl)<br>geomtypes.cpp:262 | ported (comment) | geomtypes.rs:572 `fn with_size` |
| `Polygon::Polygon(const PolygonInterface &)` | ctor | geomtypes.h:231 (decl)<br>geomtypes.cpp:270 | ported (comment) | geomtypes.rs:584 `fn from_interface` |
| `Polygon::clear()` | method | geomtypes.h:233 (decl)<br>geomtypes.cpp:365 | ported (comment) | geomtypes.rs:419 `fn clear` |
| `Polygon::empty() const` | method | geomtypes.h:235 (decl)<br>geomtypes.cpp:372 | ported (comment) | geomtypes.rs:425 `fn empty` |
| `Polygon::size() const` | method | geomtypes.h:237 (decl)<br>geomtypes.cpp:378 | ported (comment) | geomtypes.rs:430 `fn size` |
| `Polygon::id() const` | method | geomtypes.h:239 (decl)<br>geomtypes.cpp:384 | ported (comment) | geomtypes.rs:436 `fn id` |
| `Polygon::at(size_t) const` | method | geomtypes.h:242 (decl)<br>geomtypes.cpp:390 | ported (comment) | geomtypes.rs:441 `fn at` |
| `Polygon::setPoint(size_t, const Point &)` | method | geomtypes.h:246 (decl)<br>geomtypes.cpp:397 | ported (comment) | geomtypes.rs:598 `fn set_point` |
| `Polygon::simplify() const` | method | geomtypes.h:253 (decl)<br>geomtypes.cpp:532 | ported (comment) | geomtypes.rs:612 `fn simplify` |
| `Polygon::curvedPolyline(const double, const bool) const` | method | geomtypes.h:274 (decl)<br>geomtypes.cpp:640 | ported (comment) | geomtypes.rs:683 `fn curved_polyline` |
| `Polygon::translate(const double, const double)` | method | geomtypes.h:280 (decl)<br>geomtypes.cpp:522 | ported (comment) | geomtypes.rs:604 `fn translate` |
| `Polygon::checkpointsOnSegment(size_t, int) const` | method | geomtypes.h:321 (decl)<br>geomtypes.cpp:594 | ported (comment) | geomtypes.rs:653 `fn checkpoints_on_segment` |
| `PolyLine` | typedef | geomtypes.h:328 | ported (comment) | geomtypes.rs:415 `type PolyLine` |
| `ReferencingPolygon` | class | geomtypes.h:336 | ported (comment) | geomtypes.rs:817 `struct ReferencingPolygon` |
| `ReferencingPolygon::ReferencingPolygon()` | ctor | geomtypes.h:339 (decl)<br>geomtypes.cpp:159 | ported (comment) | geomtypes.rs:826 `fn new` |
| `ReferencingPolygon::ReferencingPolygon(const Polygon &, const Router *)` | ctor | geomtypes.h:340 (decl)<br>geomtypes.cpp:123 | ported (comment) | geomtypes.rs:834 `fn from_polygon` |
| `ReferencingPolygon::clear()` | method | geomtypes.h:341 (decl)<br>geomtypes.cpp:166 | ported (comment) | geomtypes.rs:865 `fn clear` |
| `ReferencingPolygon::empty() const` | method | geomtypes.h:342 (decl)<br>geomtypes.cpp:173 | ported (comment) | geomtypes.rs:871 `fn empty` |
| `ReferencingPolygon::size() const` | method | geomtypes.h:343 (decl)<br>geomtypes.cpp:179 | ported (comment) | geomtypes.rs:876 `fn size` |
| `ReferencingPolygon::id() const` | method | geomtypes.h:344 (decl)<br>geomtypes.cpp:185 | ported (comment) | geomtypes.rs:881 `fn id` |
| `ReferencingPolygon::at(size_t) const` | method | geomtypes.h:345 (decl)<br>geomtypes.cpp:191 | ported (comment) | geomtypes.rs:886 `fn at` |
| `Rectangle` | class | geomtypes.h:356 | ported (comment) | geomtypes.rs:765 `struct Rectangle` |
| `Rectangle::Rectangle(const Point &, const Point &)` | ctor | geomtypes.h:365 (decl)<br>geomtypes.cpp:727 | ported (comment) | geomtypes.rs:770 `fn new` |
| `Rectangle::Rectangle(const Point &, const double, const double)` | ctor | geomtypes.h:375 (decl)<br>geomtypes.cpp:742 | ported (comment) | geomtypes.rs:787 `fn from_centre` |
| `unitNormalForEdge(const Point &, const Point &)` | function (static) | geomtypes.cpp:289 | ported (comment) | geomtypes.rs:374 `fn unit_normal_for_edge` |
| `SHORTEN_NONE` | constant | geomtypes.cpp:405 | ported (comment) | geomtypes.rs:449 `const SHORTEN_NONE` |
| `SHORTEN_START` | constant | geomtypes.cpp:406 | ported (comment) | geomtypes.rs:449 `const SHORTEN_NONE` |
| `SHORTEN_END` | constant | geomtypes.cpp:407 | ported (comment) | geomtypes.rs:449 `const SHORTEN_NONE` |
| `SHORTEN_BOTH` | constant | geomtypes.cpp:408 | ported (comment) | geomtypes.rs:449 `const SHORTEN_NONE` |
| `shorten_line(double &, double &, double &, double &, const unsigned int, const double)` | function (static) | geomtypes.cpp:415 | ported (comment) | geomtypes.rs:455 `fn shorten_line` |
| `mid(a, b)` | macro | geomtypes.cpp:628 | ported (comment) | geomtypes.rs:554 `fn mid` |

<a id="graph"></a>
### graph

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShapeList` | typedef | graph.h:42 | ported (comment) | graph.rs:37 `type ShapeList` |
| `FlagList` | typedef | graph.h:43 | ported (comment) | graph.rs:39 `type FlagList` |
| `EdgeInf` | class | graph.h:46 | ported (comment) | graph.rs:150 `struct EdgeInf` |
| `EdgeInf::EdgeInf(VertInf *, VertInf *, const bool)` | ctor | graph.h:49 (decl)<br>graph.cpp:43 | ported (comment) | graph.rs:221 `fn new` |
| `EdgeInf::~EdgeInf()` | dtor | graph.h:50 (decl)<br>graph.cpp:68 | ported (comment) | graph.rs:246 `fn delete` |
| `EdgeInf::getDist()` | method (inline) | graph.h:51 | ported (comment) | graph.rs:254 `fn get_dist` |
| `EdgeInf::setDist(double)` | method | graph.h:55 (decl)<br>graph.cpp:259 | ported (comment) | graph.rs:407 `fn set_dist` |
| `EdgeInf::alertConns()` | method | graph.h:56 (decl)<br>graph.cpp:318 | ported (comment) | graph.rs:470 `fn alert_conns` |
| `EdgeInf::addConn(bool *)` | method | graph.h:57 (decl)<br>graph.cpp:329 | ported (comment) | graph.rs:484 `fn add_conn` |
| `EdgeInf::addCycleBlocker()` | method | graph.h:58 (decl)<br>graph.cpp:335 | ported (comment) | graph.rs:490 `fn add_cycle_blocker` |
| `EdgeInf::addBlocker(int)` | method | graph.h:59 (decl)<br>graph.cpp:342 | ported (comment) | graph.rs:496 `fn add_blocker` |
| `EdgeInf::added()` | method | graph.h:60 (decl)<br>graph.cpp:308 | ported (comment) | graph.rs:458 `fn added` |
| `EdgeInf::isOrthogonal() const` | method | graph.h:61 (decl)<br>graph.cpp:562 | ported (comment) | graph.rs:713 `fn is_orthogonal` |
| `EdgeInf::isDummyConnection() const` | method | graph.h:62 (decl)<br>graph.cpp:569 | ported (comment) | graph.rs:721 `fn is_dummy_connection` |
| `EdgeInf::isDisabled() const` | method | graph.h:63 (decl)<br>graph.cpp:293 | ported (comment) | graph.rs:440 `fn is_disabled` |
| `EdgeInf::setDisabled(const bool)` | method | graph.h:64 (decl)<br>graph.cpp:298 | ported (comment) | graph.rs:446 `fn set_disabled` |
| `EdgeInf::rotationLessThan(const VertInf *, const EdgeInf *) const` | method | graph.h:65 (decl)<br>graph.cpp:135 | ported (comment) | graph.rs:264 `fn rotation_less_than` |
| `EdgeInf::ids() const` | method | graph.h:66 (decl)<br>graph.cpp:361 | ported (comment) | graph.rs:513 `fn ids` |
| `EdgeInf::points() const` | method | graph.h:67 (decl)<br>graph.cpp:367 | ported (comment) | graph.rs:518 `fn points` |
| `EdgeInf::db_print()` | method | graph.h:68 (decl)<br>graph.cpp:373 | ported (comment) | graph.rs:525 `fn db_print` |
| `EdgeInf::checkVis()` | method | graph.h:69 (decl)<br>graph.cpp:383 | ported (comment) | graph.rs:539 `fn check_vis` |
| `EdgeInf::otherVert(const VertInf *) const` | method | graph.h:70 (decl)<br>graph.cpp:578 | ported (comment) | graph.rs:730 `fn other_vert` |
| `EdgeInf::checkEdgeVisibility(VertInf *, VertInf *, bool)` | method (static member) | graph.h:71 (decl)<br>graph.cpp:586 | ported (comment) | graph.rs:743 `fn check_edge_visibility` |
| `EdgeInf::existingEdge(VertInf *, VertInf *)` | method (static member) | graph.h:73 (decl)<br>graph.cpp:622 | ported (comment) | graph.rs:778 `fn existing_edge`<br>graph.rs:796 `fn existing_edge_walk` |
| `EdgeInf::blocker() const` | method | graph.h:74 (decl)<br>graph.cpp:313 | ported (comment) | graph.rs:464 `fn blocker` |
| `EdgeInf::isHyperedgeSegment() const` | method | graph.h:76 (decl)<br>graph.cpp:288 | ported (comment) | graph.rs:434 `fn is_hyperedge_segment` |
| `EdgeInf::setHyperedgeSegment(const bool)` | method | graph.h:77 (decl)<br>graph.cpp:303 | ported (comment) | graph.rs:452 `fn set_hyperedge_segment` |
| `EdgeInf::mtstDist() const` | method | graph.h:78 (decl)<br>graph.cpp:283 | ported (comment) | graph.rs:428 `fn mtst_dist` |
| `EdgeInf::setMtstDist(const double)` | method | graph.h:79 (decl)<br>graph.cpp:278 | ported (comment) | graph.rs:422 `fn set_mtst_dist` |
| `EdgeInf::makeActive()` | method | graph.h:87 (decl)<br>graph.cpp:185 | ported (comment) | graph.rs:316 `fn make_active` |
| `EdgeInf::makeInactive()` | method | graph.h:88 (decl)<br>graph.cpp:221 | ported (comment) | graph.rs:360 `fn make_inactive` |
| `EdgeInf::firstBlocker()` | method | graph.h:89 (decl)<br>graph.cpp:486 | ported (comment) | graph.rs:635 `fn first_blocker` |
| `EdgeInf::isBetween(VertInf *, VertInf *)` | method | graph.h:90 (decl)<br>graph.cpp:550 | ported (comment) | graph.rs:701 `fn is_between` |
| `EdgeList` | class | graph.h:109 | ported (comment) | graph.rs:872 `struct EdgeList` |
| `EdgeList::EdgeList(bool)` | ctor | graph.h:113 (decl)<br>graph.cpp:672 | ported (comment) | graph.rs:882 `fn new` |
| `EdgeList::~EdgeList()` | dtor | graph.h:114 (decl)<br>graph.cpp:681 | not-ported: subsumed | only runs at Router destruction (deletes remaining edges via clear()); arenas are dropped with the Router |
| `EdgeList::clear()` | method | graph.h:115 (decl)<br>graph.cpp:687 | ported (comment) | graph.rs:894 `fn clear` |
| `EdgeList::begin()` | method | graph.h:116 (decl)<br>graph.cpp:771 | ported (comment) | graph.rs:972 `fn begin` |
| `EdgeList::end()` | method | graph.h:117 (decl)<br>graph.cpp:777 | ported (comment) | graph.rs:978 `fn end` |
| `EdgeList::size() const` | method | graph.h:118 (decl)<br>graph.cpp:701 | ported (comment) | graph.rs:907 `fn size` |
| `EdgeList::addEdge(EdgeInf *)` | method | graph.h:120 (decl)<br>graph.cpp:707 | ported (comment) | graph.rs:913 `fn add_edge` |
| `EdgeList::removeEdge(EdgeInf *)` | method | graph.h:121 (decl)<br>graph.cpp:740 | ported (comment) | graph.rs:947 `fn remove_edge` |
| `orthogTurnOrder(const Point &, const Point &, const Point &)` | function (static, inline) | graph.cpp:85 | ported (comment) | graph.rs:177 `fn orthog_turn_order` |

<a id="hyperedge"></a>
### hyperedge

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ConnEndList` | typedef | hyperedge.h:47 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `ConnRefList` | typedef | hyperedge.h:50 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `JunctionRefList` | typedef | hyperedge.h:53 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `VertexList` | typedef | hyperedge.h:55 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `ConnRefSet` | typedef | hyperedge.h:56 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `VertexSet` | typedef | hyperedge.h:57 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `JunctionRefVector` | typedef | hyperedge.h:59 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `ConnEndListVector` | typedef | hyperedge.h:60 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `ConnRefListVector` | typedef | hyperedge.h:61 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `JunctionRefListVector` | typedef | hyperedge.h:62 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `VertexSetVector` | typedef | hyperedge.h:63 | ported (comment) | hyperedge.rs:31 `type ConnEndList` |
| `HyperedgeNewAndDeletedObjectLists` | struct | hyperedge.h:79 | ported (comment) | hyperedge.rs:48 `struct HyperedgeNewAndDeletedObjectLists` |
| `HyperedgeRerouter` | class | hyperedge.h:129 | ported (comment) | hyperedge.rs:58 `struct HyperedgeRerouter` |
| `HyperedgeRerouter::HyperedgeRerouter()` | ctor | hyperedge.h:138 (decl)<br>hyperedge.cpp:41 | ported (comment) | hyperedge.rs:76 `fn new` |
| `HyperedgeRerouter::registerHyperedgeForRerouting(ConnEndList)` | method | hyperedge.h:148 (decl)<br>hyperedge.cpp:51 | ported (comment) | hyperedge.rs:81 `fn register_hyperedge_for_rerouting` |
| `HyperedgeRerouter::registerHyperedgeForRerouting(JunctionRef *)` | method | hyperedge.h:164 (decl)<br>hyperedge.cpp:60 | ported (comment) | hyperedge.rs:89 `fn register_hyperedge_for_rerouting_junction` |
| `HyperedgeRerouter::newAndDeletedObjectLists(size_t) const` | method | hyperedge.h:182 (decl)<br>hyperedge.cpp:74 | ported (comment) | hyperedge.rs:102 `fn new_and_deleted_object_lists` |
| `HyperedgeRerouter::count() const` | method | hyperedge.h:189 (decl)<br>hyperedge.cpp:69 | ported (comment) | hyperedge.rs:97 `fn count` |
| `HyperedgeRerouter::setRouter(Router *)` | method | hyperedge.h:198 (decl)<br>hyperedge.cpp:46 | not-ported: subsumed | stores m_router; the port passes &mut Router to every HyperedgeRerouter method |
| `HyperedgeRerouter::calcHyperedgeConnectors()` | method | hyperedge.h:200 (decl)<br>hyperedge.cpp:212 | ported (comment) | hyperedge.rs:291 `fn calc_hyperedge_connectors` |
| `HyperedgeRerouter::performRerouting()` | method | hyperedge.h:202 (decl)<br>hyperedge.cpp:277 | ported (comment) | hyperedge.rs:414 `fn perform_rerouting` |
| `HyperedgeRerouter::outputInstanceToSVG(FILE *)` | method | hyperedge.h:203 (decl)<br>hyperedge.cpp:90 | ported (comment) | hyperedge.rs:132 `fn output_instance_to_svg` |
| `HyperedgeRerouter::findAttachedObjects(size_t, ConnRef *, JunctionRef *, ConnRefSet &)` | method | hyperedge.h:204 (decl)<br>hyperedge.cpp:128 | ported (comment) | hyperedge.rs:168 `fn find_attached_objects_conn` |
| `HyperedgeRerouter::findAttachedObjects(size_t, JunctionRef *, ConnRef *, ConnRefSet &)` | method | hyperedge.h:206 (decl)<br>hyperedge.cpp:178 | ported (comment) | hyperedge.rs:255 `fn find_attached_objects_junction` |

<a id="hyperedgeimprover"></a>
### hyperedgeimprover

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShiftSegmentList` | typedef | hyperedgeimprover.h:43 | ported (comment) | hyperedgeimprover.rs:78 `type ShiftSegmentList` |
| `RootSegmentsMap` | typedef | hyperedgeimprover.h:44 | ported (comment) | hyperedgeimprover.rs:81 `type RootSegmentsMap` |
| `JunctionHyperedgeTreeNodeMap` | typedef | hyperedgeimprover.h:45 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `JunctionSet` | typedef | hyperedgeimprover.h:47 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `ConnRefList` | typedef | hyperedgeimprover.h:48 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `JunctionRefList` | typedef | hyperedgeimprover.h:49 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `HyperedgeImprover` | class | hyperedgeimprover.h:51 | ported (comment) | hyperedgeimprover.rs:305 `struct HyperedgeImprover` |
| `HyperedgeImprover::HyperedgeImprover()` | ctor | hyperedgeimprover.h:55 (decl)<br>hyperedgeimprover.cpp:267 | ported (comment) | hyperedgeimprover.rs:329 `fn new` |
| `HyperedgeImprover::clear()` | method | hyperedgeimprover.h:57 (decl)<br>hyperedgeimprover.cpp:278 | ported (comment) | hyperedgeimprover.rs:336 `fn clear` |
| `HyperedgeImprover::setRouter(Router *)` | method | hyperedgeimprover.h:60 (decl)<br>hyperedgeimprover.cpp:273 | not-ported: subsumed | stores m_router; the port passes &mut Router to every HyperedgeImprover method |
| `HyperedgeImprover::newAndDeletedObjectLists() const` | method | hyperedgeimprover.h:64 (decl)<br>hyperedgeimprover.cpp:1027 | ported (comment) | hyperedgeimprover.rs:353 `fn new_and_deleted_object_lists` |
| `HyperedgeImprover::execute(bool)` | method | hyperedgeimprover.h:67 (decl)<br>hyperedgeimprover.cpp:787 | ported (comment) | hyperedgeimprover.rs:979 `fn execute`<br>hyperedgeimprover.rs:987 `fn execute_impl` |
| `HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeNode *, const size_t, HyperedgeTreeEdge *, ShiftSegmentList &)` | method | hyperedgeimprover.h:71 (decl)<br>hyperedgeimprover.cpp:293 | ported (comment) | hyperedgeimprover.rs:409 `fn create_shift_segments_for_dimension_excluding_node` |
| `HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeEdge *, const size_t, HyperedgeTreeNode *, ShiftSegmentList &)` | method | hyperedgeimprover.h:76 (decl)<br>hyperedgeimprover.cpp:310 | ported (comment) | hyperedgeimprover.rs:428 `fn create_shift_segments_for_dimension_excluding_edge` |
| `HyperedgeImprover::mergeOverlappingSegments(ShiftSegmentList &)` | method | hyperedgeimprover.h:84 (decl)<br>hyperedgeimprover.cpp:342 | ported (comment) | hyperedgeimprover.rs:503 `fn merge_overlapping_segments` |
| `HyperedgeImprover::buildHyperedgeSegments(const size_t)` | method | hyperedgeimprover.h:90 (decl)<br>hyperedgeimprover.cpp:376 | ported (comment) | hyperedgeimprover.rs:534 `fn build_hyperedge_segments` |
| `HyperedgeImprover::removeZeroLengthEdges()` | method | hyperedgeimprover.h:96 (decl)<br>hyperedgeimprover.cpp:398 | ported (comment) | hyperedgeimprover.rs:558 `fn remove_zero_length_edges_all` |
| `HyperedgeImprover::moveJunctionsAlongCommonEdges()` | method | hyperedgeimprover.h:102 (decl)<br>hyperedgeimprover.cpp:413 | ported (comment) | hyperedgeimprover.rs:574 `fn move_junctions_along_common_edges` |
| `HyperedgeImprover::nudgeHyperedgeSegments(size_t, unsigned int &)` | method | hyperedgeimprover.h:108 (decl)<br>hyperedgeimprover.cpp:555 | ported (comment) | hyperedgeimprover.rs:733 `fn nudge_hyperedge_segments` |
| `HyperedgeImprover::writeHyperedgeSegmentsBackToConnPaths()` | method | hyperedgeimprover.h:112 (decl)<br>hyperedgeimprover.cpp:619 | ported (comment) | hyperedgeimprover.rs:790 `fn write_hyperedge_segments_back_to_conn_paths` |
| `HyperedgeImprover::outputHyperedgesToSVG(unsigned int, HyperedgeShiftSegment *)` | method | hyperedgeimprover.h:116 (decl)<br>hyperedgeimprover.cpp:637 | ported (comment) | hyperedgeimprover.rs:809 `fn write_hyperedges_svg`<br>hyperedgeimprover.rs:916 `fn output_hyperedges_to_svg` |
| `HyperedgeImprover::getEndpoints(JunctionRef *, JunctionRef *, std::set<VertInf *> &)` | method | hyperedgeimprover.h:122 (decl)<br>hyperedgeimprover.cpp:743 | ported (comment) | hyperedgeimprover.rs:940 `fn get_endpoints` |
| `HyperedgeImprover::moveJunctionAlongCommonEdge(HyperedgeTreeNode *, bool &)` | method | hyperedgeimprover.h:129 (decl)<br>hyperedgeimprover.cpp:1046 | ported (comment) | hyperedgeimprover.rs:1239 `fn move_junction_along_common_edge` |
| `HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeNode *, HyperedgeTreeEdge *)` | method | hyperedgeimprover.h:134 (decl)<br>hyperedgeimprover.cpp:465 | ported (comment) | hyperedgeimprover.rs:624 `fn remove_zero_length_edges_node` |
| `HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeEdge *, HyperedgeTreeNode *)` | method | hyperedgeimprover.h:139 (decl)<br>hyperedgeimprover.cpp:448 | ported (comment) | hyperedgeimprover.rs:610 `fn remove_zero_length_edges_edge` |
| `HyperedgeShiftSegment` | class | hyperedgeimprover.cpp:41 | ported (comment) | hyperedgeimprover.rs:84 `struct HyperedgeShiftSegment` |
| `HyperedgeShiftSegment::HyperedgeShiftSegment(HyperedgeTreeNode *, HyperedgeTreeNode *, const size_t, bool)` | ctor | hyperedgeimprover.cpp:44 | ported (comment) | hyperedgeimprover.rs:105 `fn new` |
| `HyperedgeShiftSegment::~HyperedgeShiftSegment()` | dtor (virtual) | hyperedgeimprover.cpp:61 | ported (comment) | hyperedgeimprover.rs:137 `fn destruct` |
| `HyperedgeShiftSegment::lowPoint()` | method | hyperedgeimprover.cpp:70 | ported (comment) | hyperedgeimprover.rs:150 `fn low_point_node`<br>hyperedgeimprover.rs:287 `fn low_point` |
| `HyperedgeShiftSegment::highPoint()` | method | hyperedgeimprover.cpp:74 | ported (comment) | hyperedgeimprover.rs:155 `fn high_point_node`<br>hyperedgeimprover.rs:291 `fn high_point` |
| `HyperedgeShiftSegment::lowPoint() const` | method | hyperedgeimprover.cpp:78 | ported (name) | hyperedgeimprover.rs:288 |
| `HyperedgeShiftSegment::highPoint() const` | method | hyperedgeimprover.cpp:82 | ported (name) | hyperedgeimprover.rs:292 |
| `HyperedgeShiftSegment::setBalanceCount()` | method | hyperedgeimprover.cpp:90 | ported (comment) | hyperedgeimprover.rs:164 `fn set_balance_count` |
| `HyperedgeShiftSegment::balanceCount() const` | method | hyperedgeimprover.cpp:130 | ported (comment) | hyperedgeimprover.rs:194 `fn balance_count` |
| `HyperedgeShiftSegment::adjustPosition()` | method | hyperedgeimprover.cpp:135 | ported (comment) | hyperedgeimprover.rs:200 `fn adjust_position` |
| `HyperedgeShiftSegment::overlapsWith(const ShiftSegment *, const size_t) const` | method | hyperedgeimprover.cpp:185 | ported (comment) | hyperedgeimprover.rs:256 `fn overlaps_with_impl`<br>hyperedgeimprover.rs:295 `fn overlaps_with` |
| `HyperedgeShiftSegment::immovable() const` | method | hyperedgeimprover.cpp:204 | ported (comment) | hyperedgeimprover.rs:299 `fn immovable` |
| `HyperedgeShiftSegment::settled() const` | method | hyperedgeimprover.cpp:208 | ported (comment) | hyperedgeimprover.rs:274 `fn settled` |
| `HyperedgeShiftSegment::mergesWith(HyperedgeShiftSegment *)` | method | hyperedgeimprover.cpp:212 | ported (comment) | hyperedgeimprover.rs:457 `fn merges_with` |
| `CmpHyperedgeSegmentDirOrder(const ShiftSegment *, const ShiftSegment *)` | function (static) — dead (#if 0) | hyperedgeimprover.cpp:253 | not-ported: dead-code | inside `#if 0` ("UNUSED") |

<a id="hyperedgetree"></a>
### hyperedgetree

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `JunctionHyperedgeTreeNodeMap` | typedef | hyperedgetree.h:54 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap` |
| `JunctionSet` | typedef | hyperedgetree.h:56 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap` |
| `JunctionRefList` | typedef | hyperedgetree.h:57 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap` |
| `ConnRefList` | typedef | hyperedgetree.h:58 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap` |
| `OrderedHENodeSet` | typedef | hyperedgetree.h:62 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap`<br>hyperedgetree.rs:171 `struct OrderedHENodeSet` |
| `HyperedgeTreeNode` | struct | hyperedgetree.h:64 | ported (comment) | hyperedgetree.rs:98 `struct HyperedgeTreeNode` |
| `HyperedgeTreeNode::HyperedgeTreeNode()` | ctor | hyperedgetree.h:66 (decl)<br>hyperedgetree.cpp:41 | ported (comment) | hyperedgetree.rs:313 `fn new_node` |
| `HyperedgeTreeNode::~HyperedgeTreeNode()` | dtor | hyperedgetree.h:67 (decl)<br>hyperedgetree.cpp:51 | ported (comment) | hyperedgetree.rs:332 `fn delete_node` |
| `HyperedgeTreeNode::deleteEdgesExcept(HyperedgeTreeEdge *)` | method | hyperedgetree.h:69 (decl)<br>hyperedgetree.cpp:325 | ported (comment) | hyperedgetree.rs:708 `fn node_delete_edges_except` |
| `HyperedgeTreeNode::removeOtherJunctionsFrom(HyperedgeTreeEdge *, JunctionSet &)` | method | hyperedgetree.h:70 (decl)<br>hyperedgetree.cpp:85 | ported (comment) | hyperedgetree.rs:399 `fn node_remove_other_junctions_from` |
| `HyperedgeTreeNode::outputEdgesExcept(FILE *, HyperedgeTreeEdge *)` | method | hyperedgetree.h:72 (decl)<br>hyperedgetree.cpp:64 | ported (comment) | hyperedgetree.rs:371 `fn node_output_edges_except` |
| `HyperedgeTreeNode::disconnectEdge(HyperedgeTreeEdge *)` | method | hyperedgetree.h:73 (decl)<br>hyperedgetree.cpp:342 | ported (comment) | hyperedgetree.rs:722 `fn node_disconnect_edge` |
| `HyperedgeTreeNode::spliceEdgesFrom(HyperedgeTreeNode *)` | method | hyperedgetree.h:74 (decl)<br>hyperedgetree.cpp:363 | ported (comment) | hyperedgetree.rs:729 `fn node_splice_edges_from` |
| `HyperedgeTreeNode::writeEdgesToConns(HyperedgeTreeEdge *, size_t)` | method | hyperedgetree.h:75 (decl)<br>hyperedgetree.cpp:118 | ported (comment) | hyperedgetree.rs:434 `fn node_write_edges_to_conns` |
| `HyperedgeTreeNode::addConns(HyperedgeTreeEdge *, Router *, ConnRefList &, ConnRef *)` | method | hyperedgetree.h:76 (decl)<br>hyperedgetree.cpp:135 | ported (comment) | hyperedgetree.rs:535 `fn node_add_conns` |
| `HyperedgeTreeNode::updateConnEnds(HyperedgeTreeEdge *, bool, ConnRefList &)` | method | hyperedgetree.h:78 (decl)<br>hyperedgetree.cpp:199 | ported (comment) | hyperedgetree.rs:578 `fn node_update_conn_ends` |
| `HyperedgeTreeNode::listJunctionsAndConnectors(HyperedgeTreeEdge *, JunctionRefList &, ConnRefList &)` | method | hyperedgetree.h:80 (decl)<br>hyperedgetree.cpp:244 | ported (comment) | hyperedgetree.rs:626 `fn node_list_junctions_and_connectors` |
| `HyperedgeTreeNode::isImmovable() const` | method | hyperedgetree.h:82 (decl)<br>hyperedgetree.cpp:374 | ported (comment) | hyperedgetree.rs:746 `fn node_is_immovable` |
| `HyperedgeTreeNode::validateHyperedge(const HyperedgeTreeEdge *, const size_t) const` | method | hyperedgetree.h:83 (decl)<br>hyperedgetree.cpp:263 | ported (comment) | hyperedgetree.rs:647 `fn node_validate_hyperedge` |
| `HyperedgeTreeEdge` | struct | hyperedgetree.h:96 | ported (comment) | hyperedgetree.rs:115 `struct HyperedgeTreeEdge` |
| `HyperedgeTreeEdge::HyperedgeTreeEdge(HyperedgeTreeNode *, HyperedgeTreeNode *, ConnRef *)` | ctor | hyperedgetree.h:98 (decl)<br>hyperedgetree.cpp:393 | ported (comment) | hyperedgetree.rs:765 `fn new_edge` |
| `HyperedgeTreeEdge::followFrom(HyperedgeTreeNode *) const` | method | hyperedgetree.h:101 (decl)<br>hyperedgetree.cpp:410 | ported (comment) | hyperedgetree.rs:835 `fn edge_follow_from` |
| `HyperedgeTreeEdge::zeroLength() const` | method | hyperedgetree.h:102 (decl)<br>hyperedgetree.cpp:419 | ported (comment) | hyperedgetree.rs:844 `fn edge_zero_length` |
| `HyperedgeTreeEdge::splitFromNodeAtPoint(HyperedgeTreeNode *, const Point &)` | method | hyperedgetree.h:103 (decl)<br>hyperedgetree.cpp:719 | ported (comment) | hyperedgetree.rs:1122 `fn edge_split_from_node_at_point` |
| `HyperedgeTreeEdge::hasOrientation(const size_t) const` | method | hyperedgetree.h:104 (decl)<br>hyperedgetree.cpp:450 | ported (comment) | hyperedgetree.rs:852 `fn edge_has_orientation` |
| `HyperedgeTreeEdge::outputNodesExcept(FILE *, HyperedgeTreeNode *)` | method | hyperedgetree.h:105 (decl)<br>hyperedgetree.cpp:428 | ported (comment) | hyperedgetree.rs:806 `fn edge_output_nodes_except` |
| `HyperedgeTreeEdge::deleteNodesExcept(HyperedgeTreeNode *)` | method | hyperedgetree.h:106 (decl)<br>hyperedgetree.cpp:784 | ported (comment) | hyperedgetree.rs:1192 `fn edge_delete_nodes_except` |
| `HyperedgeTreeEdge::removeOtherJunctionsFrom(HyperedgeTreeNode *, JunctionSet &)` | method | hyperedgetree.h:107 (decl)<br>hyperedgetree.cpp:764 | ported (comment) | hyperedgetree.rs:1169 `fn edge_remove_other_junctions_from` |
| `HyperedgeTreeEdge::writeEdgesToConns(HyperedgeTreeNode *, size_t)` | method | hyperedgetree.h:109 (decl)<br>hyperedgetree.cpp:480 | ported (comment) | hyperedgetree.rs:875 `fn edge_write_edges_to_conns` |
| `HyperedgeTreeEdge::addConns(HyperedgeTreeNode *, Router *, ConnRefList &)` | method | hyperedgetree.h:110 (decl)<br>hyperedgetree.cpp:570 | ported (comment) | hyperedgetree.rs:962 `fn edge_add_conns` |
| `HyperedgeTreeEdge::updateConnEnds(HyperedgeTreeNode *, bool, ConnRefList &)` | method | hyperedgetree.h:112 (decl)<br>hyperedgetree.cpp:622 | ported (comment) | hyperedgetree.rs:1013 `fn edge_update_conn_ends` |
| `HyperedgeTreeEdge::disconnectEdge()` | method | hyperedgetree.h:114 (decl)<br>hyperedgetree.cpp:749 | ported (comment) | hyperedgetree.rs:1156 `fn edge_disconnect_edge` |
| `HyperedgeTreeEdge::replaceNode(HyperedgeTreeNode *, HyperedgeTreeNode *)` | method | hyperedgetree.h:115 (decl)<br>hyperedgetree.cpp:459 | ported (comment) | hyperedgetree.rs:860 `fn edge_replace_node` |
| `HyperedgeTreeEdge::listJunctionsAndConnectors(HyperedgeTreeNode *, JunctionRefList &, ConnRefList &)` | method | hyperedgetree.h:117 (decl)<br>hyperedgetree.cpp:671 | ported (comment) | hyperedgetree.rs:1076 `fn edge_list_junctions_and_connectors` |
| `HyperedgeTreeEdge::validateHyperedge(const HyperedgeTreeNode *, const size_t) const` | method | hyperedgetree.h:119 (decl)<br>hyperedgetree.cpp:693 | ported (comment) | hyperedgetree.rs:1098 `fn edge_validate_hyperedge` |
| `VertexNodeMap` | typedef | hyperedgetree.h:128 | ported (comment) | hyperedgetree.rs:87 `type JunctionHyperedgeTreeNodeMap` |
| `CmpNodesInDim` | class | hyperedgetree.h:131 | not-ported: subsumed | comparator class; its ctor and operator() are ported (hyperedgetree.rs) |
| `CmpNodesInDim::CmpNodesInDim(const size_t)` | ctor | hyperedgetree.h:134 (decl)<br>hyperedgetree.cpp:802 | ported (comment) | hyperedgetree.rs:238 `fn new_node_set` |
| `CmpNodesInDim::operator()(const HyperedgeTreeNode *, const HyperedgeTreeNode *) const` | operator | hyperedgetree.h:135 (decl)<br>hyperedgetree.cpp:810 | ported (comment) | hyperedgetree.rs:134 `fn cmp_nodes_in_dim` |
| `travellingForwardOnConnector(ConnRef *, JunctionRef *)` | function (static) | hyperedgetree.cpp:171 | ported (comment) | hyperedgetree.rs:195 `fn travelling_forward_on_connector` |

<a id="junction"></a>
### junction

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `JunctionRefList` | typedef | junction.h:47 | not-ported: subsumed | std::list&lt;JunctionRef*&gt;; Rust uses Vec&lt;JunctionRef&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `JunctionRef` | class | junction.h:57 | ported (comment) | junction.rs:34 `struct JunctionData`<br>junction.rs:43 `struct JunctionRef` |
| `JunctionRef::JunctionRef(Router *, Point, const unsigned int)` | ctor | junction.h:112 (decl)<br>junction.cpp:37 | ported (comment) | junction.rs:56 `fn new` |
| `JunctionRef::~JunctionRef()` | dtor (virtual) | junction.h:122 (decl)<br>junction.cpp:52 | ported (comment) | obstacle.rs:135 `fn delete` |
| `JunctionRef::removeJunctionAndMergeConnectors()` | method | junction.h:141 (decl)<br>junction.cpp:190 | ported (comment) | junction.rs:288 `fn remove_junction_and_merge_connectors` |
| `JunctionRef::position() const` | method | junction.h:145 (decl)<br>junction.cpp:121 | ported (comment) | junction.rs:185 `fn position`<br>obstacle.rs:227 `fn position` |
| `JunctionRef::setPositionFixed(bool)` | method | junction.h:158 (decl)<br>junction.cpp:83 | ported (comment) | junction.rs:141 `fn set_position_fixed` |
| `JunctionRef::positionFixed() const` | method | junction.h:163 (decl)<br>junction.cpp:90 | ported (comment) | junction.rs:152 `fn position_fixed` |
| `JunctionRef::recommendedPosition() const` | method | junction.h:170 (decl)<br>junction.cpp:136 | ported (comment) | junction.rs:212 `fn recommended_position` |
| `JunctionRef::makeRectangle(Router *, const Point &)` | method | junction.h:172 (decl)<br>junction.cpp:63 | ported (comment) | junction.rs:123 `fn make_rectangle` |
| `JunctionRef::preferOrthogonalDimension(const size_t)` | method | junction.h:173 (decl)<br>junction.cpp:96 | ported (comment) | junction.rs:162 `fn prefer_orthogonal_dimension` |
| `JunctionRef::outputCode(FILE *) const` | method | junction.h:181 (decl)<br>junction.cpp:148 | ported (comment) | junction.rs:234 `fn output_code` |
| `JunctionRef::setPosition(const Point &)` | method | junction.h:182 (decl)<br>junction.cpp:127 | ported (comment) | junction.rs:195 `fn set_position` |
| `JunctionRef::setRecommendedPosition(const Point &)` | method | junction.h:183 (decl)<br>junction.cpp:142 | ported (comment) | junction.rs:222 `fn set_recommended_position` |
| `JunctionRef::moveAttachedConns(const Point &)` | method | junction.h:184 (decl)<br>junction.cpp:170 | ported (comment) | junction.rs:260 `fn move_attached_conns` |

<a id="makepath"></a>
### makepath

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `AStarPath` | class | makepath.h:37 | ported (comment) | makepath.rs:1535 `struct AStarPath` |
| `AStarPath::AStarPath()` | ctor | makepath.h:40 (decl)<br>makepath.cpp:914 | ported (comment) | makepath.rs:1542 `fn new` |
| `AStarPath::~AStarPath()` | dtor | makepath.h:41 (decl)<br>makepath.cpp:919 | not-ported: subsumed | deletes the pimpl; Rust ownership drops it |
| `AStarPath::search(ConnRef *, VertInf *, VertInf *, VertInf *)` | method | makepath.h:42 (decl)<br>makepath.cpp:924 | ported (comment) | makepath.rs:1552 `fn search` |
| `ANode` | class | makepath.cpp:52 | ported (comment) | makepath.rs:43 `struct ANode` |
| `ANode::ANode(VertInf *, int)` | ctor | makepath.cpp:64 | ported (comment) | makepath.rs:62 `fn new` |
| `ANode::ANode()` | ctor | makepath.cpp:73 | ported (comment) | makepath.rs:76 `fn default` |
| `AStarPathPrivate` | class | makepath.cpp:84 | ported (comment) | makepath.rs:831 `struct AStarPathPrivate` |
| `AStarPathPrivate::AStarPathPrivate()` | ctor | makepath.cpp:87 | not-ported: subsumed | member initialisation only; AStarPathPrivate fields are initialised in makepath.rs |
| `AStarPathPrivate::~AStarPathPrivate()` | dtor | makepath.cpp:94 | not-ported: subsumed | frees the ANode blocks; Rust Vec storage is dropped |
| `AStarPathPrivate::newANode(const ANode &, const bool)` | method | makepath.cpp:104 | ported (comment) | makepath.rs:846 `fn new_a_node` |
| `AStarPathPrivate::search(ConnRef *, VertInf *, VertInf *, VertInf *)` | method | makepath.cpp:125 (decl)<br>makepath.cpp:967 | ported (comment) | makepath.rs:933 `fn search` |
| `AStarPathPrivate::determineEndPointLocation(double, VertInf *, VertInf *, VertInf *, int)` | method | makepath.cpp:129 (decl)<br>makepath.cpp:929 | ported (comment) | makepath.rs:856 `fn determine_end_point_location` |
| `AStarPathPrivate::estimatedCost(ConnRef *, const Point *, const Point &) const` | method | makepath.cpp:131 (decl)<br>makepath.cpp:856 | ported (comment) | makepath.rs:901 `fn estimated_cost` |
| `ANodeCmp` | class | makepath.cpp:153 | not-ported: subsumed | comparator class; its operator() is ported (makepath.rs) |
| `ANodeCmp::ANodeCmp()` | ctor | makepath.cpp:156 | not-ported: subsumed | empty ctor of stateless comparator |
| `ANodeCmp::operator()(const ANode *, const ANode *)` | operator | makepath.cpp:159 | ported (comment) | makepath.rs:100 `fn a_node_cmp` |
| `Dot(const Point &, const Point &)` | function (static) | makepath.cpp:182 | ported (comment) | makepath.rs:209 `fn dot` |
| `CrossLength(const Point &, const Point &)` | function (static) | makepath.cpp:187 | ported (comment) | makepath.rs:214 `fn cross_length` |
| `angleBetween(const Point &, const Point &, const Point &)` | function (static) | makepath.cpp:196 | ported (comment) | makepath.rs:221 `fn angle_between` |
| `constructPolygonPath(Polygon &, VertInf *, VertInf *, ANode *)` | function (static) | makepath.cpp:214 | ported (comment) | makepath.rs:237 `fn construct_polygon_path` |
| `dimDirection(double)` | function (static, inline) | makepath.cpp:292 | ported (comment) | makepath.rs:324 `fn dim_direction` |
| `cost(ConnRef *, const double, VertInf *, VertInf *, ANode *)` | function (static) | makepath.cpp:310 | ported (comment) | makepath.rs:339 `fn cost` |
| `CostDirectionN` | constant | makepath.cpp:489 | ported (comment) | makepath.rs:510 `const COST_DIRECTION_N` |
| `CostDirectionE` | constant | makepath.cpp:490 | ported (comment) | makepath.rs:510 `const COST_DIRECTION_N` |
| `CostDirectionS` | constant | makepath.cpp:491 | ported (comment) | makepath.rs:510 `const COST_DIRECTION_N` |
| `CostDirectionW` | constant | makepath.cpp:492 | ported (comment) | makepath.rs:510 `const COST_DIRECTION_N` |
| `printDirections(FILE *, unsigned int)` | function (static) — conditional (#ifdef ESTIMATED_COST_DEBUG) | makepath.cpp:495 | ported (comment) | makepath.rs:521 `fn print_directions` |
| `orthogonalDirectionsCount(const unsigned int)` | function (static) | makepath.cpp:517 | ported (comment) | makepath.rs:517 `fn print_directions` |
| `orthogonalDirection(const Point &, const Point &)` | function (static) | makepath.cpp:540 | ported (comment) | makepath.rs:557 `fn orthogonal_direction` |
| `dirRight(unsigned int)` | function (static) | makepath.cpp:566 | ported (comment) | makepath.rs:577 `fn dir_right` |
| `dirLeft(unsigned int)` | function (static) | makepath.cpp:591 | ported (comment) | makepath.rs:595 `fn dir_left` |
| `dirReverse(unsigned int)` | function (static) | makepath.cpp:616 | ported (comment) | makepath.rs:613 `fn dir_reverse` |
| `bends(const Point &, unsigned int, const Point &, unsigned int)` | function | makepath.cpp:648 | ported (comment) | makepath.rs:638 `fn bends` |
| `estimatedCostSpecific(ConnRef *, const Point *, const Point &, const VertInf *, const unsigned int)` | function (static) | makepath.cpp:782 | ported (comment) | makepath.rs:743 `fn estimated_cost_specific` |
| `CmpVisEdgeRotation` | class | makepath.cpp:879 | ported (comment) | makepath.rs:816 |
| `CmpVisEdgeRotation::CmpVisEdgeRotation(const VertInf *)` | ctor | makepath.cpp:882 | not-ported: subsumed | stores lastPt; the ported comparator takes it as an argument (graph.rs) |
| `CmpVisEdgeRotation::operator()(const EdgeInf *, const EdgeInf *) const` | operator | makepath.cpp:886 | ported (comment) | graph.rs:847 `fn cmp_vis_edge_rotation` |
| `pointAlignedWithOneOf(const Point &, const std::vector<Point> &, const size_t)` | function (static, inline) | makepath.cpp:901 | ported (comment) | makepath.rs:820 `fn point_aligned_with_one_of` |

<a id="mtst"></a>
### mtst

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `VertexSetList` | typedef | mtst.h:44 | ported (comment) | mtst.rs:69 `type VertexSetList` |
| `LayeredOrthogonalEdge` | typedef | mtst.h:46 | ported (comment) | mtst.rs:71 `type LayeredOrthogonalEdge` |
| `LayeredOrthogonalEdgeList` | typedef | mtst.h:47 | ported (comment) | mtst.rs:73 `type LayeredOrthogonalEdgeList` |
| `HeapCmpVertInf` | struct | mtst.h:50 | not-ported: subsumed | comparator struct; its operator() is ported (mtst.rs) |
| `HeapCmpVertInf::operator()(const VertInf *, const VertInf *) const` | operator | mtst.h:52 (decl)<br>mtst.cpp:53 | ported (comment) | mtst.rs:85 `fn heap_cmp_vert_inf` |
| `CmpEdgeInf` | struct | mtst.h:57 | not-ported: subsumed | comparator struct; its operator() is ported (mtst.rs) |
| `CmpEdgeInf::operator()(const EdgeInf *, const EdgeInf *) const` | operator | mtst.h:59 (decl)<br>mtst.cpp:60 | ported (comment) | mtst.rs:91 `fn cmp_edge_inf` |
| `MinimumTerminalSpanningTree` | class | mtst.h:66 | ported (comment) | mtst.rs:126 `struct MinimumTerminalSpanningTree` |
| `MinimumTerminalSpanningTree::MinimumTerminalSpanningTree(Router *, std::set<VertInf *>, JunctionHyperedgeTreeNodeMap *)` | ctor | mtst.h:69 (decl)<br>mtst.cpp:76 | ported (comment) | mtst.rs:158 `fn new` |
| `MinimumTerminalSpanningTree::~MinimumTerminalSpanningTree()` | dtor | mtst.h:72 (decl)<br>mtst.cpp:89 | ported (comment) | mtst.rs:183 `fn destruct` |
| `MinimumTerminalSpanningTree::constructInterleaved()` | method | mtst.h:76 (decl)<br>mtst.cpp:633 | ported (comment) | mtst.rs:722 `fn construct_interleaved` |
| `MinimumTerminalSpanningTree::constructSequential()` | method | mtst.h:78 (decl)<br>mtst.cpp:282 | ported (comment) | mtst.rs:382 `fn construct_sequential` |
| `MinimumTerminalSpanningTree::setDebuggingOutput(FILE *, unsigned int)` | method | mtst.h:80 (decl) | not-ported: declared-undefined | declared in mtst.h, never defined |
| `MinimumTerminalSpanningTree::rootJunction() const` | method | mtst.h:81 (decl)<br>mtst.cpp:98 | ported (comment) | mtst.rs:197 `fn root_junction` |
| `MinimumTerminalSpanningTree::buildHyperedgeTreeToRoot(VertInf *, HyperedgeTreeNode *, VertInf *, bool)` | method | mtst.h:84 (decl)<br>mtst.cpp:182 | ported (comment) | mtst.rs:274 `fn build_hyperedge_tree_to_root` |
| `MinimumTerminalSpanningTree::resetDistsForPath(VertInf *, VertInf **)` | method | mtst.h:87 (decl)<br>mtst.cpp:252 | ported (comment) | mtst.rs:351 `fn reset_dists_for_path` |
| `MinimumTerminalSpanningTree::rewriteRestOfHyperedge(VertInf *, VertInf **)` | method | mtst.h:88 (decl)<br>mtst.cpp:886 | ported (comment) | mtst.rs:960 `fn rewrite_rest_of_hyperedge` |
| `MinimumTerminalSpanningTree::drawForest(VertInf *, VertInf *)` | method | mtst.h:89 (decl)<br>mtst.cpp:913 | ported (comment) | mtst.rs:1010 `fn draw_forest` |
| `MinimumTerminalSpanningTree::makeSet(VertInf *)` | method | mtst.h:91 (decl)<br>mtst.cpp:104 | ported (comment) | mtst.rs:202 `fn make_set` |
| `MinimumTerminalSpanningTree::findSet(VertInf *)` | method | mtst.h:92 (decl)<br>mtst.cpp:111 | ported (comment) | mtst.rs:210 `fn find_set` |
| `MinimumTerminalSpanningTree::unionSets(VertexSetList::iterator, VertexSetList::iterator)` | method | mtst.h:93 (decl)<br>mtst.cpp:124 | ported (comment) | mtst.rs:217 `fn union_sets` |
| `MinimumTerminalSpanningTree::addNode(VertInf *, HyperedgeTreeNode *)` | method | mtst.h:94 (decl)<br>mtst.cpp:135 | ported (comment) | mtst.rs:229 `fn add_node` |
| `MinimumTerminalSpanningTree::removeInvalidBridgingEdges()` | method | mtst.h:96 (decl)<br>mtst.cpp:543 | ported (comment) | mtst.rs:625 `fn remove_invalid_bridging_edges` |
| `MinimumTerminalSpanningTree::commitToBridgingEdge(EdgeInf *)` | method | mtst.h:97 (decl)<br>mtst.cpp:991 | ported (comment) | mtst.rs:1051 `fn commit_to_bridging_edge` |
| `MinimumTerminalSpanningTree::connectsWithoutBend(VertInf *, VertInf *)` | method | mtst.h:98 (decl)<br>mtst.cpp:832 | ported (comment) | mtst.rs:910 `fn connects_without_bend` |
| `MinimumTerminalSpanningTree::getOrthogonalEdgesFromVertex(VertInf *, VertInf *)` | method | mtst.h:99 (decl)<br>mtst.cpp:577 | ported (comment) | mtst.rs:656 `fn get_orthogonal_edges_from_vertex` |
| `MinimumTerminalSpanningTree::orthogonalPartner(VertInf *, double)` | method | mtst.h:101 (decl)<br>mtst.cpp:523 | ported (comment) | mtst.rs:607 `fn orthogonal_partner` |
| `MinimumTerminalSpanningTree::realVerticesCountingPartners(EdgeInf *)` | method | mtst.h:102 (decl)<br>mtst.cpp:964 | ported (comment) | mtst.rs:979 `fn real_vertices_counting_partners` |
| `delete_vertex` | struct | mtst.cpp:66 | ported (comment) | mtst.rs:98 `fn delete_vertex` |
| `delete_vertex::operator()(VertInf *)` | operator | mtst.cpp:68 | ported (manual) | mtst.rs fn delete_vertex() (struct and operator() ported as one fn) |

<a id="obstacle"></a>
### obstacle

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ObstacleList` | typedef | obstacle.h:49 | not-ported: subsumed | std::list&lt;Obstacle*&gt;; Router::m_obstacles is a Vec&lt;ObstacleIdx&gt; (router.rs) |
| `ConnRefList` | typedef | obstacle.h:50 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `Obstacle` | class | obstacle.h:56 | ported (comment) | obstacle.rs:61 `struct Obstacle` |
| `Obstacle::Obstacle(Router *, Polygon, const unsigned int)` | ctor | obstacle.h:69 (decl)<br>obstacle.cpp:34 | ported (comment) | obstacle.rs:81 `fn new_base` |
| `Obstacle::~Obstacle()` | dtor (virtual) | obstacle.h:77 (decl)<br>obstacle.cpp:76 | ported (comment) | obstacle.rs:134 `fn delete` |
| `Obstacle::id() const` | method | obstacle.h:82 (decl)<br>obstacle.cpp:249 | ported (comment) | junction.rs:103 `fn id`<br>obstacle.rs:397 `fn id`<br>obstacle.rs:511 `fn id`<br>shape.rs:86 `fn id` |
| `Obstacle::polygon() const` | method | obstacle.h:86 (decl)<br>obstacle.cpp:255 | ported (comment) | junction.rs:113 `fn polygon`<br>obstacle.rs:403 `fn polygon`<br>obstacle.rs:520 `fn polygon` |
| `Obstacle::router() const` | method | obstacle.h:90 (decl)<br>obstacle.cpp:261 | not-ported: subsumed | m_router back-pointer getter; the port passes &Router explicitly (router.rs data model) |
| `Obstacle::position() const` | method (virtual, pure) | obstacle.h:93 (decl) | ported (comment) | obstacle.rs:529 `fn position` |
| `Obstacle::setNewPoly(const Polygon &)` | method | obstacle.h:95 (decl)<br>obstacle.cpp:100 | ported (comment) | obstacle.rs:239 `fn set_new_poly` |
| `Obstacle::firstVert()` | method | obstacle.h:96 (decl)<br>obstacle.cpp:237 | ported (comment) | obstacle.rs:385 `fn first_vert` |
| `Obstacle::lastVert()` | method | obstacle.h:97 (decl)<br>obstacle.cpp:243 | ported (comment) | obstacle.rs:391 `fn last_vert` |
| `Obstacle::routingBox() const` | method | obstacle.h:98 (decl)<br>obstacle.cpp:267 | ported (comment) | obstacle.rs:409 `fn routing_box`<br>obstacle.rs:538 `fn routing_box`<br>shape.rs:125 `fn routing_box` |
| `Obstacle::routingPolygon() const` | method | obstacle.h:99 (decl)<br>obstacle.cpp:277 | ported (comment) | obstacle.rs:420 `fn routing_polygon`<br>obstacle.rs:547 `fn routing_polygon`<br>shape.rs:135 `fn routing_polygon` |
| `Obstacle::attachedConnectors() const` | method | obstacle.h:100 (decl)<br>obstacle.cpp:340 | ported (comment) | junction.rs:346 `fn attached_connectors`<br>obstacle.rs:482 `fn attached_connectors`<br>obstacle.rs:556 `fn attached_connectors`<br>shape.rs:145 `fn attached_connectors` |
| `Obstacle::computeVisibilityNaive()` | method | obstacle.h:111 (decl)<br>visibility.cpp:52 | ported (comment) | visibility.rs:40 `fn compute_visibility_naive` |
| `Obstacle::computeVisibilitySweep()` | method | obstacle.h:112 (decl)<br>visibility.cpp:97 | ported (comment) | visibility.rs:91 `fn compute_visibility_sweep` |
| `Obstacle::outputCode(FILE *) const` | method (virtual, pure) | obstacle.h:114 (decl) | ported (comment) | obstacle.rs:500 `fn output_code` |
| `Obstacle::makeActive()` | method | obstacle.h:115 (decl)<br>obstacle.cpp:134 | ported (comment) | obstacle.rs:271 `fn make_active` |
| `Obstacle::makeInactive()` | method | obstacle.h:116 (decl)<br>obstacle.cpp:157 | ported (comment) | obstacle.rs:294 `fn make_inactive` |
| `Obstacle::isActive() const` | method | obstacle.h:117 (decl)<br>obstacle.cpp:231 | ported (comment) | obstacle.rs:379 `fn is_active` |
| `Obstacle::updatePinPolyLineVisibility()` | method | obstacle.h:118 (decl)<br>obstacle.cpp:187 | ported (comment) | obstacle.rs:329 `fn update_pin_poly_line_visibility` |
| `Obstacle::removeFromGraph()` | method | obstacle.h:119 (decl)<br>obstacle.cpp:298 | ported (comment) | obstacle.rs:441 `fn remove_from_graph` |
| `Obstacle::shapeCentre()` | method | obstacle.h:120 (decl)<br>obstacle.cpp:287 | ported (comment) | obstacle.rs:431 `fn shape_centre` |
| `Obstacle::getPointVertex(const Point &)` | method | obstacle.h:122 (decl)<br>obstacle.cpp:311 | ported (comment) | obstacle.rs:455 `fn get_point_vertex` |
| `Obstacle::addFollowingConnEnd(ConnEnd *)` | method | obstacle.h:124 (decl)<br>obstacle.cpp:328 | ported (comment) | obstacle.rs:471 `fn add_following_conn_end` |
| `Obstacle::removeFollowingConnEnd(ConnEnd *)` | method | obstacle.h:125 (decl)<br>obstacle.cpp:334 | ported (comment) | obstacle.rs:476 `fn remove_following_conn_end` |
| `Obstacle::addConnectionPin(ShapeConnectionPin *)` | method | obstacle.h:126 (decl)<br>obstacle.cpp:216 | ported (comment) | obstacle.rs:351 `fn add_connection_pin` |
| `Obstacle::removeConnectionPin(ShapeConnectionPin *)` | method | obstacle.h:127 (decl)<br>obstacle.cpp:224 | ported (comment) | obstacle.rs:366 `fn remove_connection_pin` |
| `Obstacle::assignPinVisibilityTo(const unsigned int, VertInf *)` | method | obstacle.h:128 (decl) | not-ported: declared-undefined | declared in obstacle.h, never defined |
| `Obstacle::possiblePinPoints(unsigned int) const` | method | obstacle.h:130 (decl)<br>obstacle.cpp:198 | ported (comment) | obstacle.rs:337 `fn possible_pin_points` |

<a id="orthogonal"></a>
### orthogonal

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `generateStaticOrthogonalVisGraph(Router *)` | function | orthogonal.h:33 (decl)<br>orthogonal.cpp:1730 | ported (comment) | orthogonal.rs:1348 `fn generate_static_orthogonal_vis_graph` |
| `improveOrthogonalRoutes(Router *)` | function | orthogonal.h:34 (decl)<br>orthogonal.cpp:3252 | ported (comment) | orthogonal.rs:3330 `fn improve_orthogonal_routes` |
| `freeSegmentID` | constant | orthogonal.cpp:54 | ported (comment) | orthogonal.rs:106 `const FREE_SEGMENT_ID` |
| `fixedSegmentID` | constant | orthogonal.cpp:55 | ported (comment) | orthogonal.rs:106 `const FREE_SEGMENT_ID` |
| `channelLeftID` | constant | orthogonal.cpp:56 | ported (comment) | orthogonal.rs:106 `const FREE_SEGMENT_ID` |
| `channelRightID` | constant | orthogonal.cpp:57 | ported (comment) | orthogonal.rs:106 `const FREE_SEGMENT_ID` |
| `freeWeight` | constant | orthogonal.cpp:59 | ported (comment) | orthogonal.rs:112 `const FREE_WEIGHT` |
| `strongWeight` | constant | orthogonal.cpp:60 | ported (comment) | orthogonal.rs:112 `const FREE_WEIGHT` |
| `strongerWeight` | constant | orthogonal.cpp:61 | ported (comment) | orthogonal.rs:112 `const FREE_WEIGHT` |
| `fixedWeight` | constant | orthogonal.cpp:62 | ported (comment) | orthogonal.rs:112 `const FREE_WEIGHT` |
| `UnsignedPair` | class | orthogonal.cpp:67 | ported (comment) | orthogonal.rs:121 `struct UnsignedPair` |
| `UnsignedPair::UnsignedPair(unsigned, unsigned)` | ctor | orthogonal.cpp:70 | ported (comment) | orthogonal.rs:129 `fn new` |
| `UnsignedPair::operator<(const UnsignedPair &) const` | operator | orthogonal.cpp:79 | ported (name) | orthogonal.rs:123 |
| `UnsignedPairSet` | typedef | orthogonal.cpp:91 | ported (comment) | orthogonal.rs:145 `type UnsignedPairSet` |
| `CmpIndexes` | class | orthogonal.cpp:96 | not-ported: subsumed | comparator class; its operator() is ported (orthogonal.rs) |
| `CmpIndexes::CmpIndexes(ConnRef *, size_t)` | ctor | orthogonal.cpp:99 | not-ported: subsumed | stores conn/dim; the ported comparator takes them as arguments |
| `CmpIndexes::operator()(size_t, size_t)` | operator | orthogonal.cpp:104 | ported (comment) | orthogonal.rs:1656 `fn cmp_indexes` |
| `NudgingShiftSegment` | class | orthogonal.cpp:115 | ported (comment) | orthogonal.rs:1662 `struct NudgingShiftSegment` |
| `NudgingShiftSegment::NudgingShiftSegment(ConnRef *, const size_t, const size_t, bool, bool, const size_t, double, double)` | ctor | orthogonal.cpp:119 | ported (comment) | orthogonal.rs:1682 `fn new_shiftable` |
| `NudgingShiftSegment::NudgingShiftSegment(ConnRef *, const size_t, const size_t, const size_t)` | ctor | orthogonal.cpp:138 | ported (comment) | orthogonal.rs:1715 `fn new_fixed` |
| `NudgingShiftSegment::~NudgingShiftSegment()` | dtor (virtual) | orthogonal.cpp:156 | not-ported: subsumed | empty virtual destructor |
| `NudgingShiftSegment::lowPoint()` | method | orthogonal.cpp:159 | ported (comment) | orthogonal.rs:1743 `fn low_point_of`<br>orthogonal.rs:2109 `fn low_point` |
| `NudgingShiftSegment::highPoint()` | method | orthogonal.cpp:163 | ported (comment) | orthogonal.rs:1749 `fn high_point_of`<br>orthogonal.rs:2113 `fn high_point` |
| `NudgingShiftSegment::lowPoint() const` | method | orthogonal.cpp:167 | ported (name) | orthogonal.rs:2110 |
| `NudgingShiftSegment::highPoint() const` | method | orthogonal.cpp:171 | ported (name) | orthogonal.rs:2114 |
| `NudgingShiftSegment::nudgeDistance() const` | method | orthogonal.cpp:175 | ported (comment) | orthogonal.rs:1780 `fn nudge_distance` |
| `NudgingShiftSegment::immovable() const` | method | orthogonal.cpp:179 | ported (comment) | orthogonal.rs:2121 `fn immovable` |
| `NudgingShiftSegment::createSolverVariable(const bool)` | method | orthogonal.cpp:183 | ported (comment) | orthogonal.rs:1788 `fn create_solver_variable` |
| `NudgingShiftSegment::updatePositionsFromSolver(const bool)` | method | orthogonal.cpp:235 | ported (comment) | orthogonal.rs:1830 `fn update_positions_from_solver` |
| `NudgingShiftSegment::fixedOrder(bool &) const` | method | orthogonal.cpp:266 | ported (comment) | orthogonal.rs:1864 `fn fixed_order` |
| `NudgingShiftSegment::order() const` | method | orthogonal.cpp:288 | ported (comment) | orthogonal.rs:1882 `fn order` |
| `NudgingShiftSegment::zigzag() const` | method | orthogonal.cpp:300 | ported (comment) | orthogonal.rs:1892 `fn zigzag` |
| `NudgingShiftSegment::overlapsWith(const ShiftSegment *, const size_t) const` | method | orthogonal.cpp:306 | ported (comment) | orthogonal.rs:1900 `fn nudging_overlaps_with`<br>orthogonal.rs:2117 `fn overlaps_with` |
| `NudgingShiftSegment::canAlignWith(const NudgingShiftSegment *, const size_t) const` | method | orthogonal.cpp:361 | ported (comment) | orthogonal.rs:1942 `fn can_align_with` |
| `NudgingShiftSegment::shouldAlignWith(const ShiftSegment *, const size_t) const` | method | orthogonal.cpp:383 | ported (comment) | orthogonal.rs:1962 `fn should_align_with` |
| `NudgingShiftSegment::mergeWith(const ShiftSegment *, const size_t)` | method | orthogonal.cpp:443 | ported (comment) | orthogonal.rs:2019 `fn merge_with` |
| `NudgingShiftSegment::hasCheckpointAtPosition(const double, const size_t) const` | method | orthogonal.cpp:479 | ported (comment) | orthogonal.rs:2063 `fn has_checkpoint_at_position` |
| `NudgingShiftSegment::lowC() const` | method | orthogonal.cpp:503 | ported (comment) | orthogonal.rs:2075 `fn low_c` |
| `NudgingShiftSegment::highC() const` | method | orthogonal.cpp:514 | ported (comment) | orthogonal.rs:2089 `fn high_c` |
| `ScanVisDirFlag: VisDirNone, VisDirUp, VisDirDown` | enum | orthogonal.cpp:529 | ported (comment) | orthogonal.rs:152 `const VIS_DIR_NONE` |
| `ScanVisDirFlags` | typedef | orthogonal.cpp:534 | ported (comment) | orthogonal.rs:156 `type ScanVisDirFlags` |
| `getPosVertInfDirections(VertInf *, size_t)` | function (static) | orthogonal.cpp:542 | ported (comment) | orthogonal.rs:164 `fn get_pos_vert_inf_directions` |
| `PosVertInf` | struct | orthogonal.cpp:588 | ported (comment) | orthogonal.rs:200 `struct PosVertInf` |
| `PosVertInf::PosVertInf(double, VertInf *, ScanVisDirFlags)` | ctor | orthogonal.cpp:590 | ported (comment) | orthogonal.rs:215 `fn new` |
| `PosVertInf::operator<(const PosVertInf &) const` | operator | orthogonal.cpp:597 | ported (comment) | orthogonal.rs:225 `fn less_than` |
| `CmpVertInf` | struct | orthogonal.cpp:629 | not-ported: subsumed | comparator struct; its operator() is ported (orthogonal.rs) |
| `CmpVertInf::operator()(const VertInf *, const VertInf *) const` | operator | orthogonal.cpp:631 | ported (comment) | orthogonal.rs:291 `fn cmp_vert_inf` |
| `VertSet` | typedef | orthogonal.cpp:651 | ported (comment) | orthogonal.rs:330 `type VertSet` |
| `BreakpointSet` | typedef | orthogonal.cpp:655 | ported (comment) | orthogonal.rs:335 `type BreakpointSet` |
| `LineSegment` | class | orthogonal.cpp:659 | ported (comment) | orthogonal.rs:340 `struct LineSegment` |
| `LineSegment::LineSegment(const double &, const double &, const double &, bool, VertInf *, VertInf *)` | ctor | orthogonal.cpp:662 | ported (comment) | orthogonal.rs:355 `fn new` |
| `LineSegment::LineSegment(const double &, const double &, VertInf *)` | ctor | orthogonal.cpp:680 | ported (comment) | orthogonal.rs:389 `fn new_point` |
| `LineSegment::operator<(const LineSegment &) const` | operator | orthogonal.cpp:693 | ported (comment) | orthogonal.rs:406 `fn less_than` |
| `LineSegment::overlaps(const LineSegment &) const` | method | orthogonal.cpp:711 | ported (comment) | orthogonal.rs:421 `fn overlaps` |
| `LineSegment::mergeVertInfs(const LineSegment &)` | method | orthogonal.cpp:732 | ported (comment) | orthogonal.rs:439 `fn merge_vert_infs` |
| `LineSegment::beginVertInf() const` | method | orthogonal.cpp:739 | ported (comment) | orthogonal.rs:448 `fn begin_vert_inf` |
| `LineSegment::finishVertInf() const` | method | orthogonal.cpp:754 | ported (comment) | orthogonal.rs:458 `fn finish_vert_inf` |
| `LineSegment::commitPositionX(Router *, double)` | method | orthogonal.cpp:770 | ported (comment) | orthogonal.rs:468 `fn commit_position_x` |
| `LineSegment::horiCommitBegin(Router *, VertInf *)` | method | orthogonal.cpp:790 | ported (comment) | orthogonal.rs:488 `fn hori_commit_begin` |
| `LineSegment::horiCommitFinish(Router *, VertInf *)` | method | orthogonal.cpp:809 | ported (comment) | orthogonal.rs:503 `fn hori_commit_finish` |
| `LineSegment::addSegmentsUpTo(double)` | method | orthogonal.cpp:829 | ported (comment) | orthogonal.rs:520 `fn add_segments_up_to` |
| `LineSegment::addEdgeHorizontal(Router *)` | method | orthogonal.cpp:856 | ported (comment) | orthogonal.rs:549 `fn add_edge_horizontal` |
| `LineSegment::setLongRangeVisibilityFlags(size_t)` | method | orthogonal.cpp:867 | ported (comment) | orthogonal.rs:561 `fn set_long_range_visibility_flags` |
| `LineSegment::addEdgeHorizontalTillIntersection(Router *, LineSegment &)` | method | orthogonal.cpp:956 | ported (comment) | orthogonal.rs:631 `fn add_edge_horizontal_till_intersection` |
| `LineSegment::insertBreakpointsBegin(Router *, LineSegment &)` | method | orthogonal.cpp:987 | ported (comment) | orthogonal.rs:666 `fn insert_breakpoints_begin` |
| `LineSegment::insertBreakpointsFinish(Router *, LineSegment &)` | method | orthogonal.cpp:1012 | ported (comment) | orthogonal.rs:689 `fn insert_breakpoints_finish` |
| `LineSegment::generateVisibilityEdgesFromBreakpointSet(Router *, size_t)` | method | orthogonal.cpp:1035 | ported (comment) | orthogonal.rs:711 `fn generate_visibility_edges_from_breakpoint_set` |
| `LineSegment::operator=(LineSegment const &)` | operator | orthogonal.cpp:1219 (decl) | not-ported: declared-undefined | private, never defined (MSVC workaround) |
| `LineSegment::LineSegment()` | ctor | orthogonal.cpp:1220 (decl) | not-ported: declared-undefined | private default ctor, never defined (MSVC workaround) |
| `SegmentList` | typedef | orthogonal.cpp:1223 | ported (comment) | orthogonal.rs:847 `type SegmentList` |
| `SegmentListWrapper` | class | orthogonal.cpp:1225 | ported (comment) | orthogonal.rs:864 `struct SegmentListWrapper` |
| `SegmentListWrapper::insert(LineSegment)` | method | orthogonal.cpp:1228 | ported (comment) | orthogonal.rs:877 `fn insert` |
| `SegmentListWrapper::list()` | method | orthogonal.cpp:1264 | ported (comment) | orthogonal.rs:912 `fn list` |
| `intersectSegments(Router *, SegmentList &, LineSegment &)` | function (static) | orthogonal.cpp:1276 | ported (comment) | orthogonal.rs:921 `fn intersect_segments` |
| `processEventVert(Router *, NodeSet &, SegmentListWrapper &, Event *, unsigned int)` | function (static) | orthogonal.cpp:1368 | ported (comment) | orthogonal.rs:998 `fn process_event_vert` |
| `processEventHori(Router *, NodeSet &, SegmentListWrapper &, Event *, unsigned int)` | function (static) | orthogonal.cpp:1551 | ported (comment) | orthogonal.rs:1186 `fn process_event_hori` |
| `fixConnectionPointVisibilityOnOutsideOfVisibilityGraph(Event **, size_t, ConnDirFlags)` | function | orthogonal.cpp:1691 | ported (comment) | orthogonal.rs:1311 `fn fix_connection_point_visibility_on_outside_of_visibility_graph` |
| `RectBounds` | typedef | orthogonal.cpp:1989 | ported (comment) | orthogonal.rs:2127 `type RectBounds` |
| `insideRectBounds(const Point &, const RectBounds &)` | function (static) | orthogonal.cpp:1991 | ported (comment) | orthogonal.rs:2130 `fn inside_rect_bounds` |
| `buildOrthogonalNudgingSegments(Router *, const size_t, ShiftSegmentList &)` | function (static) | orthogonal.cpp:2015 | ported (comment) | orthogonal.rs:2149 `fn build_orthogonal_nudging_segments` |
| `ConnRefVector` | typedef | orthogonal.cpp:2278 | not-ported: subsumed | local std::vector&lt;ConnRef*&gt; in improveOrthogonalRoutes; Rust Vec |
| `RouteVector` | typedef | orthogonal.cpp:2279 | not-ported: subsumed | local std::vector&lt;Polygon&gt; in improveOrthogonalRoutes; Rust Vec |
| `CmpLineOrder` | class | orthogonal.cpp:2282 | ported (comment) | orthogonal.rs:2367 `struct CmpLineOrder` |
| `CmpLineOrder::CmpLineOrder(PtOrderMap &, const size_t)` | ctor | orthogonal.cpp:2285 | not-ported: subsumed | stores orders/dimension; the ported CmpLineOrder is built from them (orthogonal.rs) |
| `CmpLineOrder::operator()(const ShiftSegment *, const ShiftSegment *, bool *) const` | operator | orthogonal.cpp:2290 | ported (comment) | orthogonal.rs:2376 `fn call` |
| `linesort(bool, ShiftSegmentList, CmpLineOrder &)` | function (static) | orthogonal.cpp:2378 | ported (comment) | orthogonal.rs:2446 `fn linesort` |
| `ShiftSegmentPtrList` | typedef | orthogonal.cpp:2459 | not-ported: subsumed | std::list&lt;ShiftSegment*&gt;; Rust Vec (orthogonal.rs) |
| `PotentialSegmentConstraint` | class | orthogonal.cpp:2461 | ported (comment) | orthogonal.rs:2518 `struct PotentialSegmentConstraint` |
| `PotentialSegmentConstraint::PotentialSegmentConstraint(size_t, size_t, const Variables &)` | ctor | orthogonal.cpp:2464 | ported (comment) | orthogonal.rs:2526 `fn new` |
| `PotentialSegmentConstraint::operator<(const PotentialSegmentConstraint) const` | operator | orthogonal.cpp:2472 | ported (comment) | orthogonal.rs:2531 `fn less_than` |
| `PotentialSegmentConstraint::sepDistance() const` | method | orthogonal.cpp:2476 | ported (comment) | orthogonal.rs:2536 `fn sep_distance` |
| `PotentialSegmentConstraint::stillValid() const` | method | orthogonal.cpp:2484 | ported (comment) | orthogonal.rs:2544 `fn still_valid` |
| `PotentialSegmentConstraint::rewriteIndex(size_t, size_t)` | method | orthogonal.cpp:2488 | ported (comment) | orthogonal.rs:2549 `fn rewrite_index` |
| `ImproveOrthogonalRoutes` | class | orthogonal.cpp:2509 | ported (comment) | orthogonal.rs:2561 `struct ImproveOrthogonalRoutes` |
| `ImproveOrthogonalRoutes::ImproveOrthogonalRoutes(Router *)` | ctor | orthogonal.cpp:2512 (decl)<br>orthogonal.cpp:2528 | ported (comment) | orthogonal.rs:2569 `fn new` |
| `ImproveOrthogonalRoutes::execute()` | method | orthogonal.cpp:2513 (decl)<br>orthogonal.cpp:2533 | ported (comment) | orthogonal.rs:2578 `fn execute` |
| `ImproveOrthogonalRoutes::simplifyOrthogonalRoutes()` | method | orthogonal.cpp:2516 (decl)<br>orthogonal.cpp:3132 | ported (comment) | orthogonal.rs:3209 `fn simplify_orthogonal_routes` |
| `ImproveOrthogonalRoutes::buildOrthogonalNudgingOrderInfo()` | method | orthogonal.cpp:2517 (decl)<br>orthogonal.cpp:3148 | ported (comment) | orthogonal.rs:3223 `fn build_orthogonal_nudging_order_info` |
| `ImproveOrthogonalRoutes::nudgeOrthogonalRoutes(size_t, bool)` | method | orthogonal.cpp:2518 (decl)<br>orthogonal.cpp:2597 | ported (comment) | orthogonal.rs:2638 `fn nudge_orthogonal_routes` |

<a id="router"></a>
### router

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `LineRep` | struct | router.h:50 | ported (comment) | router.rs:495 `struct LineRep` |
| `LineReps` | typedef | router.h:55 | ported (comment) | router.rs:495 `struct LineRep` |
| `IntList` | typedef | router.h:58 | ported (comment) | router.rs:503 `type IntList` |
| `ClusterRefList` | typedef | router.h:63 | not-ported: subsumed | std::list&lt;ClusterRef*&gt;; Router::cluster_refs is a Vec (router.rs) |
| `ObstacleList` | typedef | router.h:65 | not-ported: subsumed | std::list&lt;Obstacle*&gt;; Router::m_obstacles is a Vec&lt;ObstacleIdx&gt; (router.rs) |
| `RouterFlag: PolyLineRouting, OrthogonalRouting` | enum | router.h:70 | ported (comment) | router.rs:411 `const POLY_LINE_ROUTING` |
| `runningTo` | constant | router.h:81 | ported (comment) | router.rs:415 `const RUNNING_TO` |
| `runningFrom` | constant | router.h:82 | ported (comment) | router.rs:415 `const RUNNING_TO` |
| `runningToAndFrom` | constant | router.h:83 | ported (comment) | router.rs:415 `const RUNNING_TO` |
| `RoutingParameter: segmentPenalty, anglePenalty, crossingPenalty, clusterCrossingPenalty, fixedSharedPathPenalty, portDirectionPenalty, shapeBufferDistance, idealNudgingDistance, reverseDirectionPenalty, lastRoutingParameterMarker` | enum | router.h:88 | ported (comment) | router.rs:426 `enum RoutingParameter`<br>router.rs:439 `const LAST_ROUTING_PARAMETER_MARKER` |
| `PenaltyType` | typedef | router.h:170 | ported (comment) | router.rs:441 `type PenaltyType` |
| `RoutingOption: nudgeOrthogonalSegmentsConnectedToShapes, improveHyperedgeRoutesMovingJunctions, penaliseOrthogonalSharedPathsAtConnEnds, nudgeOrthogonalTouchingColinearSegments, performUnifyingNudgingPreprocessingStep, improveHyperedgeRoutesMovingAddingAndDeletingJunctions, nudgeSharedPathsWithCommonEndPoint, lastRoutingOptionMarker` | enum | router.h:174 | ported (comment) | router.rs:459 `enum RoutingOption`<br>router.rs:470 `const LAST_ROUTING_OPTION_MARKER` |
| `TransactionPhases: TransactionPhaseOrthogonalVisibilityGraphScanX, TransactionPhaseOrthogonalVisibilityGraphScanY, TransactionPhaseRouteSearch, TransactionPhaseCrossingDetection, TransactionPhaseRerouteSearch, TransactionPhaseOrthogonalNudgingX, TransactionPhaseOrthogonalNudgingY, TransactionPhaseCompleted` | enum | router.h:299 | ported (comment) | router.rs:485 `const TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X` |
| `ConnRerouteFlagDelegate` | class | router.h:332 | ported (comment) | router.rs:509 `struct ConnRerouteFlagDelegate` |
| `ConnRerouteFlagDelegate::ConnRerouteFlagDelegate()` | ctor | router.h:334 (decl)<br>router.cpp:3097 | not-ported: subsumed | empty constructor (Default) |
| `ConnRerouteFlagDelegate::~ConnRerouteFlagDelegate()` | dtor | router.h:335 (decl)<br>router.cpp:3101 | not-ported: subsumed | empty destructor |
| `ConnRerouteFlagDelegate::addConn(ConnRef *)` | method | router.h:336 (decl)<br>router.cpp:3105 | ported (comment) | router.rs:516 `fn add_conn` |
| `ConnRerouteFlagDelegate::removeConn(ConnRef *)` | method | router.h:337 (decl)<br>router.cpp:3111 | ported (comment) | router.rs:522 `fn remove_conn` |
| `ConnRerouteFlagDelegate::alertConns()` | method | router.h:338 (decl)<br>router.cpp:3124 | ported (comment) | router.rs:537 `fn alert_conns` |
| `zeroParamValue` | constant | router.h:343 | ported (comment) | router.rs:420 `const ZERO_PARAM_VALUE` |
| `chooseSensibleParamValue` | constant | router.h:344 | ported (comment) | router.rs:420 `const ZERO_PARAM_VALUE` |
| `TopologyAddonInterface` | class | router.h:350 | ported (comment) | router.rs:550 `trait TopologyAddonInterface` |
| `TopologyAddonInterface::TopologyAddonInterface()` | ctor | router.h:353 | not-ported: subsumed | empty ctor; TopologyAddonInterface is a Rust trait |
| `TopologyAddonInterface::~TopologyAddonInterface()` | dtor (virtual) | router.h:356 | not-ported: subsumed | empty virtual dtor; TopologyAddonInterface is a Rust trait |
| `TopologyAddonInterface::clone() const` | method (virtual) | router.h:359 | ported (manual) | router.rs trait TopologyAddonInterface::clone_box() (Box&lt;dyn&gt; clone) |
| `TopologyAddonInterface::improveOrthogonalTopology(Router *)` | method (virtual) | router.h:364 | ported (name) | router.rs:555 |
| `TopologyAddonInterface::outputCode(FILE *) const` | method (virtual) | router.h:368 | ported (name) | router.rs:560 |
| `TopologyAddonInterface::outputDeletionCode(FILE *) const` | method (virtual) | router.h:373 | ported (name) | router.rs:565 |
| `Router` | class | router.h:386 | ported (comment) | router.rs:586 `struct Router` |
| `Router::Router(const unsigned int)` | ctor | router.h:392 (decl)<br>router.cpp:47 | ported (comment) | router.rs:1021 `fn new` |
| `Router::~Router()` | dtor (virtual) | router.h:399 (decl)<br>router.cpp:108 | ported (comment) | router.rs:1108 `fn destruct` |
| `Router::setTransactionUse(const bool)` | method | router.h:453 (decl)<br>router.cpp:458 | ported (comment) | router.rs:1489 `fn set_transaction_use` |
| `Router::transactionUse() const` | method | router.h:462 (decl)<br>router.cpp:452 | ported (comment) | router.rs:1484 `fn transaction_use` |
| `Router::processTransaction()` | method | router.h:479 (decl)<br>router.cpp:641 | ported (comment) | router.rs:1677 `fn process_transaction` |
| `Router::deleteShape(ShapeRef *)` | method | router.h:495 (decl)<br>router.cpp:282 | ported (comment) | router.rs:1304 `fn delete_shape` |
| `Router::moveShape(ShapeRef *, const Polygon &, const bool)` | method | router.h:515 (decl)<br>router.cpp:361 | ported (comment) | router.rs:1385 `fn move_shape` |
| `Router::moveShape(ShapeRef *, const double, const double)` | method | router.h:535 (decl)<br>router.cpp:320 | ported (comment) | router.rs:1352 `fn move_shape_by` |
| `Router::deleteJunction(JunctionRef *)` | method | router.h:549 (decl)<br>router.cpp:686 | ported (comment) | router.rs:1760 `fn delete_junction` |
| `Router::deleteConnector(ConnRef *)` | method | router.h:563 (decl)<br>router.cpp:313 | ported (comment) | router.rs:1340 `fn delete_connector` |
| `Router::moveJunction(JunctionRef *, const Point &)` | method | router.h:578 (decl)<br>router.cpp:742 | ported (comment) | router.rs:1815 `fn move_junction` |
| `Router::moveJunction(JunctionRef *, const double, const double)` | method | router.h:597 (decl)<br>router.cpp:717 | ported (comment) | router.rs:1794 `fn move_junction_by` |
| `Router::setRoutingParameter(const RoutingParameter, const double)` | method | router.h:626 (decl)<br>router.cpp:1996 | ported (comment) | router.rs:2635 `fn set_routing_parameter` |
| `Router::routingParameter(const RoutingParameter) const` | method | router.h:635 (decl)<br>router.cpp:2039 | ported (comment) | router.rs:2656 `fn routing_parameter` |
| `Router::setRoutingOption(const RoutingOption, const bool)` | method | router.h:642 (decl)<br>router.cpp:2046 | ported (comment) | router.rs:2662 `fn set_routing_option` |
| `Router::routingOption(const RoutingOption) const` | method | router.h:649 (decl)<br>router.cpp:2054 | ported (comment) | router.rs:2668 `fn routing_option` |
| `Router::setRoutingPenalty(const RoutingParameter, const double)` | method | router.h:661 (decl)<br>router.cpp:2061 | ported (comment) | router.rs:2677 `fn set_routing_penalty` |
| `Router::hyperedgeRerouter()` | method | router.h:669 (decl)<br>router.cpp:2072 | ported (comment) | router.rs:2687 `fn hyperedge_rerouter` |
| `Router::outputInstanceToSVG(std::string)` | method | router.h:682 (decl)<br>router.cpp:2359 | ported (comment) | router.rs:3052 `fn write_instance_svg`<br>router.rs:3425 `fn output_instance_to_svg_string`<br>router.rs:3438 `fn output_instance_to_svg` |
| `Router::newObjectId() const` | method (virtual) | router.h:694 (decl)<br>router.cpp:802 | ported (comment) | router.rs:1890 `fn new_object_id` |
| `Router::objectIdIsUnused(const unsigned int) const` | method | router.h:703 (decl)<br>router.cpp:828 | ported (comment) | router.rs:1916 `fn object_id_is_unused` |
| `Router::shouldContinueTransactionWithProgress(unsigned int, unsigned int, unsigned int, double)` | method (virtual) | router.h:743 (decl)<br>router.cpp:1376 | ported (comment) | router.rs:2117 `fn should_continue_transaction_with_progress` |
| `Router::newAndDeletedObjectListsFromHyperedgeImprovement() const` | method | router.h:763 (decl)<br>router.cpp:3091 | ported (comment) | router.rs:2970 `fn new_and_deleted_object_lists_from_hyperedge_improvement` |
| `Router::setDebugHandler(DebugHandler *)` | method | router.h:765 (decl)<br>router.cpp:149 | ported (comment) | router.rs:1180 `fn set_debug_handler` |
| `Router::debugHandler() const` | method | router.h:766 (decl)<br>router.cpp:154 | ported (comment) | router.rs:1185 `fn debug_handler` |
| `Router::processActions()` | method | router.h:770 (decl)<br>router.cpp:465 | ported (comment) | router.rs:1495 `fn process_actions` |
| `Router::deleteCluster(ClusterRef *)` | method | router.h:772 (decl)<br>router.cpp:792 | ported (comment) | router.rs:1876 `fn delete_cluster` |
| `Router::attachedShapes(IntList &, const unsigned int, const unsigned int)` | method | router.h:773 (decl)<br>router.cpp:892 | ported (comment) | router.rs:1961 `fn attached_shapes` |
| `Router::attachedConns(IntList &, const unsigned int, const unsigned int)` | method | router.h:775 (decl)<br>router.cpp:868 | ported (comment) | router.rs:1944 `fn attached_conns` |
| `Router::markPolylineConnectorsNeedingReroutingForDeletedObstacle(Obstacle *)` | method | router.h:777 (decl)<br>router.cpp:1768 | ported (comment) | router.rs:2459 `fn mark_polyline_connectors_needing_rerouting_for_deleted_obstacle` |
| `Router::generateContains(VertInf *)` | method | router.h:779 (decl)<br>router.cpp:1674 | ported (comment) | router.rs:2375 `fn generate_contains` |
| `Router::printInfo()` | method | router.h:780 (decl)<br>router.cpp:2084 | ported (comment) | router.rs:2699 `fn print_info` |
| `Router::regenerateStaticBuiltGraph()` | method | router.h:781 (decl)<br>router.cpp:431 | ported (comment) | router.rs:1466 `fn regenerate_static_built_graph` |
| `Router::destroyOrthogonalVisGraph()` | method | router.h:782 (decl)<br>router.cpp:410 | ported (comment) | router.rs:1448 `fn destroy_orthogonal_vis_graph` |
| `Router::setStaticGraphInvalidated(const bool)` | method | router.h:783 (decl)<br>router.cpp:404 | ported (comment) | router.rs:1443 `fn set_static_graph_invalidated` |
| `Router::validConnType(const ConnType) const` | method | router.h:784 (decl)<br>router.cpp:1970 | ported (comment) | router.rs:2615 `fn valid_conn_type` |
| `Router::isInCrossingPenaltyReroutingStage() const` | method | router.h:785 (decl)<br>router.cpp:2078 | ported (comment) | router.rs:2692 `fn is_in_crossing_penalty_rerouting_stage` |
| `Router::markAllObstaclesAsMoved()` | method | router.h:786 (decl)<br>router.cpp:343 | ported (comment) | router.rs:1371 `fn mark_all_obstacles_as_moved` |
| `Router::shapeContainingPoint(const Point &)` | method | router.h:787 (decl)<br>router.cpp:159 | ported (comment) | router.rs:1195 `fn shape_containing_point` |
| `Router::performContinuationCheck(unsigned int, size_t, size_t)` | method | router.h:788 (decl)<br>router.cpp:1357 | ported (comment) | router.rs:2098 `fn perform_continuation_check` |
| `Router::registerSettingsChange()` | method | router.h:790 (decl)<br>router.cpp:2067 | ported (comment) | router.rs:2682 `fn register_settings_change` |
| `Router::setTopologyAddon(TopologyAddonInterface *)` | method | router.h:799 (decl)<br>router.cpp:2338 | ported (comment) | router.rs:2884 `fn set_topology_addon` |
| `Router::improveOrthogonalTopology()` | method | router.h:800 (decl)<br>router.cpp:2353 | ported (comment) | router.rs:2893 `fn improve_orthogonal_topology` |
| `Router::existsOrthogonalSegmentOverlap(const bool)` | method | router.h:803 (decl)<br>router.cpp:2175 | ported (comment) | router.rs:2798 `fn exists_orthogonal_segment_overlap` |
| `Router::existsOrthogonalFixedSegmentOverlap(const bool)` | method | router.h:804 (decl)<br>router.cpp:2208 | ported (comment) | router.rs:2808 `fn exists_orthogonal_fixed_segment_overlap` |
| `Router::existsOrthogonalTouchingPaths()` | method | router.h:805 (decl)<br>router.cpp:2242 | ported (comment) | router.rs:2817 `fn exists_orthogonal_touching_paths` |
| `Router::existsCrossings(const bool)` | method | router.h:806 (decl)<br>router.cpp:2278 | ported (comment) | router.rs:2828 `fn exists_crossings` |
| `Router::existsInvalidOrthogonalPaths()` | method | router.h:807 (decl)<br>router.cpp:2311 | ported (comment) | router.rs:2859 `fn exists_invalid_orthogonal_paths` |
| `Router::outputDiagramSVG(std::string, LineReps *)` | method | router.h:813 (decl)<br>router.cpp:2869 | ported (comment) | router.rs:3451 `fn write_diagram_svg`<br>router.rs:3523 `fn output_diagram_svg_string`<br>router.rs:3533 `fn output_diagram_svg` |
| `Router::outputDiagramText(std::string)` | method | router.h:816 (decl)<br>router.cpp:3019 | ported (comment) | router.rs:2902 `fn output_diagram_text_g`<br>router.rs:2910 `fn output_diagram_text`<br>router.rs:3557 `fn output_diagram_text_file` |
| `Router::outputDiagram(std::string)` | method | router.h:817 (decl)<br>router.cpp:3011 | ported (comment) | router.rs:3546 `fn output_diagram` |
| `Router::assignId(const unsigned int)` | method | router.h:832 (decl)<br>router.cpp:808 | ported (comment) | router.rs:1895 `fn assign_id` |
| `Router::addShape(ShapeRef *)` | method | router.h:833 (decl)<br>router.cpp:256 | ported (comment) | router.rs:1269 `fn add_shape` |
| `Router::addJunction(JunctionRef *)` | method | router.h:834 (decl)<br>router.cpp:660 | ported (comment) | router.rs:1725 `fn add_junction` |
| `Router::addCluster(ClusterRef *)` | method | router.h:835 (decl)<br>router.cpp:781 | ported (comment) | router.rs:1862 `fn add_cluster` |
| `Router::modifyConnector(ConnRef *)` | method | router.h:836 (decl)<br>router.cpp:203 | ported (comment) | router.rs:1238 `fn modify_connector` |
| `Router::modifyConnector(ConnRef *, const unsigned int, const ConnEnd &, bool)` | method | router.h:837 (decl)<br>router.cpp:177 | ported (comment) | router.rs:1212 `fn modify_connector_end` |
| `Router::modifyConnectionPin(ShapeConnectionPin *)` | method | router.h:839 (decl)<br>router.cpp:221 | ported (comment) | router.rs:1251 `fn modify_connection_pin` |
| `Router::removeObjectFromQueuedActions(const void *)` | method | router.h:841 (decl)<br>router.cpp:239 | ported (comment) | router.rs:1264 `fn remove_object_from_queued_actions` |
| `Router::newBlockingShape(const Polygon &, int)` | method | router.h:842 (decl)<br>router.cpp:1551 | ported (comment) | router.rs:2268 `fn new_blocking_shape` |
| `Router::checkAllBlockedEdges(int)` | method | router.h:843 (decl)<br>router.cpp:1616 | ported (comment) | router.rs:2320 `fn check_all_blocked_edges` |
| `Router::checkAllMissingEdges()` | method | router.h:844 (decl)<br>router.cpp:1638 | ported (comment) | router.rs:2338 `fn check_all_missing_edges` |
| `Router::adjustContainsWithAdd(const Polygon &, const int)` | method | router.h:845 (decl)<br>router.cpp:1729 | ported (comment) | router.rs:2431 `fn adjust_contains_with_add` |
| `Router::adjustContainsWithDel(const int)` | method | router.h:846 (decl)<br>router.cpp:1745 | ported (comment) | router.rs:2448 `fn adjust_contains_with_del` |
| `Router::adjustClustersWithAdd(const PolygonInterface &, const int)` | method | router.h:847 (decl)<br>router.cpp:1705 | ported (comment) | router.rs:2410 `fn adjust_clusters_with_add` |
| `Router::adjustClustersWithDel(const int)` | method | router.h:849 (decl)<br>router.cpp:1719 | ported (comment) | router.rs:2424 `fn adjust_clusters_with_del` |
| `Router::rerouteAndCallbackConnectors()` | method | router.h:850 (decl)<br>router.cpp:925 | ported (comment) | router.rs:1985 `fn reroute_and_callback_connectors` |
| `Router::improveCrossings()` | method | router.h:851 (decl)<br>router.cpp:1409 | ported (comment) | router.rs:2132 `fn improve_crossings` |
| `ConnCostRef` | typedef | router.cpp:1037 | ported (comment) | router.rs:756 `type ConnCostRef` |
| `CmpConnCostRef` | class | router.cpp:1040 | ported (comment) | router.rs:761 `type ConnCostRefSet` |
| `CmpConnCostRef::CmpConnCostRef()` | ctor | router.cpp:1043 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpConnCostRef::operator()(const ConnCostRef &, const ConnCostRef &) const` | operator | router.cpp:1046 | not-ported: subsumed | order-by-connector-id realised as the BTreeMap&lt;u32, ..&gt; key of ConnCostRefSet (router.rs) |
| `ConnCostRefSet` | typedef | router.cpp:1052 | ported (comment) | router.rs:761 `type ConnCostRefSet` |
| `ConnCostRefSetList` | typedef | router.cpp:1053 | ported (name) | router.rs:763 |
| `cheapEstimatedCost(ConnRef *)` | function (static) | router.cpp:1056 | ported (comment) | router.rs:769 `fn cheap_estimated_cost` |
| `CrossingConnectorsMap` | typedef | router.cpp:1079 | ported (comment) | router.rs:765 `type CrossingConnectorsMap` |
| `CrossingConnectorsMapList` | typedef | router.cpp:1082 | not-ported: subsumed | std::list&lt;CrossingConnectorsMap&gt;; Rust Vec (router.rs field comment) |
| `CrossingConnectorsInfo` | class | router.cpp:1087 | ported (comment) | router.rs:797 `struct CrossingConnectorsInfo` |
| `CrossingConnectorsInfo::addCrossing(ConnRef *, ConnRef *)` | method | router.cpp:1092 | ported (comment) | router.rs:809 `fn add_crossing` |
| `CrossingConnectorsInfo::crossingSetsListToRemoveCrossingsFromGroups()` | method | router.cpp:1111 | ported (comment) | router.rs:822 `fn crossing_sets_list_to_remove_crossings_from_groups` |
| `CrossingConnectorsInfo::connsKnownToCross(ConnRef *, ConnRef *)` | method | router.cpp:1205 | ported (comment) | router.rs:905 `fn conns_known_to_cross` |
| `CrossingConnectorsInfo::removeConnectorWithMostCrossings(CrossingConnectorsMap &)` | method | router.cpp:1227 | ported (comment) | router.rs:922 `fn remove_connector_with_most_crossings` |
| `CrossingConnectorsInfo::groupForConn(ConnRef *)` | method | router.cpp:1287 | ported (comment) | router.rs:976 `fn group_for_conn` |
| `CrossingConnectorsInfo::groupForCrossingConns(ConnRef *, ConnRef *)` | method | router.cpp:1310 | ported (comment) | router.rs:983 `fn group_for_crossing_conns` |
| `CmpOrderedConnCostRef` | class | router.cpp:1395 | not-ported: subsumed | comparator class; its operator() is inlined (see below) |
| `CmpOrderedConnCostRef::CmpOrderedConnCostRef()` | ctor | router.cpp:1398 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpOrderedConnCostRef::operator()(const ConnCostRef &, const ConnCostRef &) const` | operator | router.cpp:1401 | ported (manual) | router.rs Router::improve_crossings(): closure `u.0 &lt; v.0` passed to libstdcxx_list_sort |
| `ConnCostRefList` | typedef | router.cpp:1406 | not-ported: subsumed | std::list&lt;ConnCostRef&gt;; Rust Vec sorted with libstdcxx_list_sort (router.rs) |
| `AngleAFromThreeSides(const double, const double, const double)` | function (static) — conditional (#ifdef SELECTIVE_DEBUG) | router.cpp:1755 | not-ported: dead-code | only compiled with SELECTIVE_DEBUG |
| `LIMIT` | constant | router.cpp:2162 | ported (comment) | router.rs:2988 `const LIMIT` |
| `reduceRange(double &)` | function (static) | router.cpp:2164 | ported (comment) | router.rs:2991 `fn reduce_range` |

<a id="scanline"></a>
### scanline

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `CHANNEL_MAX` | constant | scanline.h:37 | ported (comment) | scanline.rs:64 `const CHANNEL_MAX` |
| `ShiftSegment` | class | scanline.h:44 | ported (comment) | scanline.rs:74 `struct ShiftSegmentBase`<br>scanline.rs:103 `trait ShiftSegment` |
| `ShiftSegment::ShiftSegment(const size_t)` | ctor | scanline.h:47 | ported (comment) | scanline.rs:90 `fn new` |
| `ShiftSegment::~ShiftSegment()` | dtor (virtual) | scanline.h:51 | not-ported: subsumed | empty virtual destructor |
| `ShiftSegment::lowPoint()` | method (virtual, pure) | scanline.h:54 (decl) | ported (comment) | scanline.rs:107 `fn low_point` |
| `ShiftSegment::highPoint()` | method (virtual, pure) | scanline.h:55 (decl) | ported (comment) | scanline.rs:109 `fn high_point` |
| `ShiftSegment::lowPoint() const` | method (virtual, pure) | scanline.h:56 (decl) | ported (name) | scanline.rs:108 |
| `ShiftSegment::highPoint() const` | method (virtual, pure) | scanline.h:57 (decl) | ported (name) | scanline.rs:110 |
| `ShiftSegment::overlapsWith(const ShiftSegment *, const size_t) const` | method (virtual, pure) | scanline.h:58 (decl) | ported (comment) | scanline.rs:111 `fn overlaps_with` |
| `ShiftSegment::immovable() const` | method (virtual, pure) | scanline.h:60 (decl) | ported (comment) | scanline.rs:113 `fn immovable` |
| `ShiftSegmentList` | typedef | scanline.h:66 | not-ported: subsumed | std::list&lt;ShiftSegment*&gt;; Rust uses Vec of segment indices (scanline.rs/orthogonal.rs) |
| `CmpNodePos` | struct | scanline.h:70 | not-ported: subsumed | comparator struct; its operator() is ported (scanline.rs) |
| `CmpNodePos::operator()(const Node *, const Node *) const` | operator | scanline.h:72 (decl)<br>scanline.cpp:37 | ported (comment) | scanline.rs:541 `fn cmp_node_pos` |
| `NodeSet` | typedef | scanline.h:76 | ported (name) | scanline.rs:578 |
| `Node` | class | scanline.h:78 | ported (comment) | scanline.rs:166 `struct Node` |
| `Node::Node(Obstacle *, const double)` | ctor | scanline.h:90 (decl)<br>scanline.cpp:53 | ported (comment) | scanline.rs:212 `fn new_obstacle_node` |
| `Node::Node(VertInf *, const double)` | ctor | scanline.h:91 (decl)<br>scanline.cpp:69 | ported (comment) | scanline.rs:235 `fn new_vert_node` |
| `Node::Node(ShiftSegment *, const double)` | ctor | scanline.h:92 (decl)<br>scanline.cpp:81 | ported (comment) | scanline.rs:260 `fn new_segment_node` |
| `Node::~Node()` | dtor (virtual) | scanline.h:93 (decl)<br>scanline.cpp:93 | not-ported: subsumed | empty destructor |
| `Node::firstObstacleAbove(size_t)` | method | scanline.h:94 (decl)<br>scanline.cpp:100 | ported (comment) | scanline.rs:289 `fn first_obstacle_above` |
| `Node::firstObstacleBelow(size_t)` | method | scanline.h:95 (decl)<br>scanline.cpp:118 | ported (comment) | scanline.rs:310 `fn first_obstacle_below` |
| `Node::markShiftSegmentsAbove(size_t)` | method | scanline.h:96 (decl)<br>scanline.cpp:135 | ported (comment) | scanline.rs:330 `fn mark_shift_segments_above` |
| `Node::markShiftSegmentsBelow(size_t)` | method | scanline.h:97 (decl)<br>scanline.cpp:151 | ported (comment) | scanline.rs:356 `fn mark_shift_segments_below` |
| `Node::findFirstPointAboveAndBelow(const size_t, const double, double &, double &, double &, double &)` | method | scanline.h:98 (decl)<br>scanline.cpp:165 | ported (comment) | scanline.rs:382 `fn find_first_point_above_and_below` |
| `Node::firstPointAbove(size_t)` | method | scanline.h:101 (decl)<br>scanline.cpp:219 | ported (comment) | scanline.rs:431 `fn first_point_above` |
| `Node::firstPointBelow(size_t)` | method | scanline.h:102 (decl)<br>scanline.cpp:241 | ported (comment) | scanline.rs:452 `fn first_point_below` |
| `Node::isInsideShape(size_t)` | method | scanline.h:103 (decl)<br>scanline.cpp:265 | ported (comment) | scanline.rs:475 `fn is_inside_shape` |
| `EventType: Open, SegOpen, ConnPoint, SegClose, Close` | enum | scanline.h:108 | ported (comment) | scanline.rs:626 `enum EventType` |
| `Event` | struct | scanline.h:117 | ported (comment) | scanline.rs:636 `struct Event` |
| `Event::Event(EventType, Node *, double)` | ctor | scanline.h:119 (decl)<br>scanline.cpp:285 | ported (comment) | scanline.rs:645 `fn new` |
| `compare_events(const void *, const void *)` | function | scanline.h:127 (decl)<br>scanline.cpp:294 | ported (comment) | scanline.rs:652 `fn compare_events` |
| `buildConnectorRouteCheckpointCache(Router *)` | function | scanline.h:128 (decl)<br>scanline.cpp:311 | ported (comment) | scanline.rs:685 `fn build_connector_route_checkpoint_cache` |
| `clearConnectorRouteCheckpointCache(Router *)` | function | scanline.h:129 (decl)<br>scanline.cpp:362 | ported (comment) | scanline.rs:723 `fn clear_connector_route_checkpoint_cache` |
| `buildOrthogonalChannelInfo(Router *, const size_t, ShiftSegmentList &)` | function | scanline.h:130 (decl)<br>scanline.cpp:462 | ported (comment) | scanline.rs:801 `fn build_orthogonal_channel_info` |
| `processShiftEvent(NodeSet &, Event *, size_t, unsigned int)` | function (static) | scanline.cpp:388 | ported (comment) | scanline.rs:745 `fn process_shift_event` |

<a id="shape"></a>
### shape

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShapeRefList` | typedef | shape.h:53 | not-ported: subsumed | std::list&lt;ShapeRef*&gt;; Rust uses Vec of handles |
| `ShapeTransformationType: TransformationType_CW90, TransformationType_CW180, TransformationType_CW270, TransformationType_FlipX, TransformationType_FlipY` | enum | shape.h:57 | ported (comment) | shape.rs:36 `enum ShapeTransformationType` |
| `ShapeRef` | class | shape.h:81 | ported (comment) | shape.rs:52 `struct ShapeRef` |
| `ShapeRef::ShapeRef(Router *, Polygon &, const unsigned int)` | ctor | shape.h:119 (decl)<br>shape.cpp:39 | ported (comment) | shape.rs:65 `fn new` |
| `ShapeRef::~ShapeRef()` | dtor (virtual) | shape.h:129 (decl)<br>shape.cpp:46 | ported (comment) | obstacle.rs:135 `fn delete` |
| `ShapeRef::polygon() const` | method | shape.h:134 (decl)<br>shape.cpp:228 | ported (comment) | shape.rs:97 `fn polygon` |
| `ShapeRef::transformConnectionPinPositions(ShapeTransformationType)` | method | shape.h:143 (decl)<br>shape.cpp:93 | ported (comment) | shape.rs:194 `fn transform_connection_pin_positions` |
| `ShapeRef::position() const` | method | shape.h:145 (decl)<br>shape.cpp:260 | ported (comment) | obstacle.rs:227 `fn position`<br>shape.rs:108 `fn position` |
| `ShapeRef::outputCode(FILE *) const` | method | shape.h:152 (decl)<br>shape.cpp:234 | ported (comment) | shape.rs:361 `fn output_code` |
| `ShapeRef::moveAttachedConns(const Polygon &)` | method | shape.h:153 (decl)<br>shape.cpp:57 | ported (comment) | shape.rs:165 `fn move_attached_conns` |
| `ShapeRef::assignPinVisibilityTo(const unsigned int, VertInf *)` | method | shape.h:154 (decl) | not-ported: declared-undefined | declared in shape.h, never defined (noted in shape.rs) |
| `ShapeRef::setCentrePos(const Point &)` | method | shape.h:156 (decl)<br>shape.cpp:273 | ported (comment) | shape.rs:348 `fn set_centre_pos` |
| `absoluteOffsetInverse(double, const Box &, size_t)` | function (static) | shape.cpp:77 | ported (comment) | shape.rs:392 `fn absolute_offset_inverse` |

<a id="timer"></a>
### timer

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `TIMER_START(r, t)` | macro | timer.h:37<br>timer.h:44 | ported (comment) | timer.rs:315 `macro_rules! timer_start` |
| `TIMER_STOP(r)` | macro | timer.h:38<br>timer.h:45 | ported (comment) | timer.rs:330 `macro_rules! timer_stop` |
| `TIMER_VAR_ADD(r, n, v)` | macro | timer.h:39<br>timer.h:46 | ported (comment) | timer.rs:343 `macro_rules! timer_var_add` |
| `TIMER_VAR_MAX(r, n, v)` | macro | timer.h:40<br>timer.h:47 | ported (comment) | timer.rs:356 `macro_rules! timer_var_max` |
| `bigclock_t` | typedef — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:49 | ported (comment) | timer.rs:22 `type BigclockT` |
| `TimerIndex: tmAdd, tmDel, tmMov, tmPth, tmOrthogGraph, tmOrthogRoute, tmOrthogCentre, tmOrthogNudge, tmHyperedgeForest, tmHyperedgeMTST, tmHyperedgeImprove, tmHyperedgeAlt, tmCount` | enum — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:51 | ported (comment) | timer.rs:32 `enum TimerIndex` |
| `timerStart` | constant — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:68 | ported (comment) | timer.rs:76 `const TIMER_START` |
| `timerDelay` | constant — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:69 | ported (comment) | timer.rs:78 `const TIMER_DELAY` |
| `TIMER_VARIABLES_COUNT` | constant — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:71 | ported (comment) | timer.rs:81 `const TIMER_VARIABLES_COUNT` |
| `Timer` | class — conditional (#else of #ifndef AVOID_PROFILE) | timer.h:73 | ported (comment) | timer.rs:131 `struct Timer` |
| `Timer::Timer()` | ctor — conditional (#ifdef AVOID_PROFILE) | timer.h:76 (decl)<br>timer.cpp:38 | ported (comment) | timer.rs:152 `fn new` |
| `Timer::initialise(const TimerIndex)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:77 (decl)<br>timer.cpp:61 | ported (comment) | timer.rs:183 `fn initialise` |
| `Timer::start()` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:78 (decl)<br>timer.cpp:69 | ported (comment) | timer.rs:191 `fn start` |
| `Timer::stop()` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:79 (decl)<br>timer.cpp:77 | ported (comment) | timer.rs:203 `fn stop` |
| `Timer::reset()` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:80 (decl)<br>timer.cpp:44 | ported (comment) | timer.rs:168 `fn reset` |
| `Timer::varIncrement(size_t, unsigned int)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:81 (decl)<br>timer.cpp:165 | ported (comment) | timer.rs:282 `fn var_increment` |
| `Timer::varMax(size_t, unsigned int)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:82 (decl)<br>timer.cpp:175 | ported (comment) | timer.rs:292 `fn var_max` |
| `Timer::print(const TimerIndex, FILE *)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:84 (decl)<br>timer.cpp:148 | ported (comment) | timer.rs:257 `fn print` |
| `Timer::printAll(FILE *)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:85 (decl)<br>timer.cpp:126 | ported (comment) | timer.rs:234 `fn print_all` |
| `Timer::printHyperedgePaper(FILE *)` | method — conditional (#ifdef AVOID_PROFILE) | timer.h:86 (decl)<br>timer.cpp:139 | ported (comment) | timer.rs:243 `fn print_hyperedge_paper` |
| `timerNames` | variable — conditional (#ifdef AVOID_PROFILE) | timer.cpp:109 | ported (comment) | timer.rs:101 `static TIMER_NAMES` |
| `toMsec(tot)` | macro — conditional (#ifdef AVOID_PROFILE) | timer.cpp:136 | ported (comment) | timer.rs:117 `fn to_msec` |
| `toAvg(tot, cnt)` | macro — conditional (#ifdef AVOID_PROFILE) | timer.cpp:137 | ported (comment) | timer.rs:122 `fn to_avg` |

<a id="vertices"></a>
### vertices

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `EdgeInfList` | typedef | vertices.h:44 | ported (name) | vertices.rs:279 |
| `VertexPair` | typedef | vertices.h:45 | ported (comment) | mtst.rs:75 `type VertexPair`<br>vertices.rs:47 `type VertexPair` |
| `ConnDirFlags` | typedef | vertices.h:47 | ported (comment) | vertices.rs:50 `type ConnDirFlags` |
| `VertIDProps` | typedef | vertices.h:48 | ported (comment) | vertices.rs:52 `type VertIDProps` |
| `VertID` | class | vertices.h:51 | ported (comment) | vertices.rs:66 `struct VertID` |
| `VertID::src` | constant | vertices.h:59<br>vertices.cpp:138 | ported (comment) | vertices.rs:76 `const SRC` |
| `VertID::tar` | constant | vertices.h:60<br>vertices.cpp:139 | ported (comment) | vertices.rs:76 `const SRC` |
| `VertID::PROP_ConnPoint` | constant | vertices.h:62<br>vertices.cpp:142 | ported (comment) | vertices.rs:80 `const PROP_CONN_POINT` |
| `VertID::PROP_OrthShapeEdge` | constant | vertices.h:63<br>vertices.cpp:143 | ported (comment) | vertices.rs:80 `const PROP_CONN_POINT` |
| `VertID::PROP_ConnectionPin` | constant | vertices.h:64<br>vertices.cpp:144 | ported (comment) | vertices.rs:80 `const PROP_CONN_POINT` |
| `VertID::PROP_ConnCheckpoint` | constant | vertices.h:65<br>vertices.cpp:145 | ported (comment) | vertices.rs:80 `const PROP_CONN_POINT` |
| `VertID::PROP_DummyPinHelper` | constant | vertices.h:66<br>vertices.cpp:146 | ported (comment) | vertices.rs:80 `const PROP_CONN_POINT` |
| `VertID::VertID()` | ctor | vertices.h:68 (decl)<br>vertices.cpp:43 | not-ported: subsumed | empty default ctor; VertID derives Default (vertices.rs) |
| `VertID::VertID(unsigned int, unsigned short, VertIDProps)` | ctor | vertices.h:69 (decl)<br>vertices.cpp:48 | ported (comment) | vertices.rs:87 `fn new` |
| `VertID::VertID(const VertID &)` | ctor | vertices.h:70 (decl)<br>vertices.cpp:56 | not-ported: subsumed | member-wise copy ctor; VertID derives Clone/Copy |
| `VertID::operator=(const VertID &)` | operator | vertices.h:71 (decl)<br>vertices.cpp:64 | not-ported: subsumed | member-wise assignment; VertID derives Copy |
| `VertID::operator==(const VertID &) const` | operator | vertices.h:72 (decl)<br>vertices.cpp:77 | ported (comment) | vertices.rs:159 `fn eq` |
| `VertID::operator!=(const VertID &) const` | operator | vertices.h:73 (decl)<br>vertices.cpp:87 | ported (comment) | vertices.rs:159 `fn eq` |
| `VertID::operator<(const VertID &) const` | operator | vertices.h:74 (decl)<br>vertices.cpp:97 | ported (comment) | vertices.rs:183 `fn cmp` |
| `VertID::operator+(const int &) const` | operator | vertices.h:75 (decl)<br>vertices.cpp:108 | ported (comment) | vertices.rs:97 `fn add` |
| `VertID::operator-(const int &) const` | operator | vertices.h:76 (decl)<br>vertices.cpp:114 | ported (comment) | vertices.rs:103 `fn sub` |
| `VertID::operator++(int)` | operator | vertices.h:77 (decl)<br>vertices.cpp:120 | ported (comment) | vertices.rs:111 `fn post_inc` |
| `VertID::print(FILE *) const` | method | vertices.h:78 (decl)<br>vertices.cpp:127 | ported (comment) | vertices.rs:118 `fn print_string` |
| `VertID::db_print() const` | method | vertices.h:79 (decl)<br>vertices.cpp:132 | ported (comment) | vertices.rs:125 `fn db_print` |
| `operator<<(ostream &, const VertID &)` | operator (friend) | vertices.h:80 (decl)<br>vertices.cpp:149 | ported (comment) | vertices.rs:190 `fn fmt` |
| `VertID::isOrthShapeEdge() const` | method (inline) | vertices.h:83 | ported (comment) | vertices.rs:131 `fn is_orth_shape_edge` |
| `VertID::isConnPt() const` | method (inline) | vertices.h:87 | ported (comment) | vertices.rs:136 `fn is_conn_pt` |
| `VertID::isConnectionPin() const` | method (inline) | vertices.h:91 | ported (comment) | vertices.rs:141 `fn is_connection_pin` |
| `VertID::isConnCheckpoint() const` | method (inline) | vertices.h:95 | ported (comment) | vertices.rs:146 `fn is_conn_checkpoint` |
| `VertID::isDummyPinHelper() const` | method (inline) | vertices.h:99 | ported (comment) | vertices.rs:151 `fn is_dummy_pin_helper` |
| `dummyOrthogID` | constant | vertices.h:110 | ported (comment) | vertices.rs:199 `const DUMMY_ORTHOG_ID` |
| `dummyOrthogShapeID` | constant | vertices.h:111 | ported (comment) | vertices.rs:199 `const DUMMY_ORTHOG_ID` |
| `VertInf` | class | vertices.h:115 | ported (comment) | vertices.rs:582 `struct VertInf` |
| `VertInf::VertInf(Router *, const VertID &, const Point &, const bool)` | ctor | vertices.h:118 (decl)<br>vertices.cpp:156 | ported (comment) | vertices.rs:621 `fn new` |
| `VertInf::~VertInf()` | dtor | vertices.h:120 (decl)<br>vertices.cpp:184 | ported (comment) | vertices.rs:673 `fn delete` |
| `VertInf::Reset(const VertID &, const Point &)` | method | vertices.h:121 (decl)<br>vertices.cpp:204 | ported (comment) | vertices.rs:698 `fn reset` |
| `VertInf::Reset(const Point &)` | method | vertices.h:122 (decl)<br>vertices.cpp:213 | ported (comment) | vertices.rs:706 `fn reset_point` |
| `VertInf::removeFromGraph(const bool)` | method | vertices.h:123 (decl)<br>vertices.cpp:228 | ported (comment) | vertices.rs:722 `fn remove_from_graph` |
| `VertInf::orphaned()` | method | vertices.h:124 (decl)<br>vertices.cpp:222 | ported (comment) | vertices.rs:715 `fn orphaned` |
| `VertInf::pathLeadsBackTo(const VertInf *) const` | method | vertices.h:126 (decl)<br>vertices.cpp:357 | ported (comment) | vertices.rs:815 `fn path_leads_back_to` |
| `VertInf::setVisibleDirections(const ConnDirFlags)` | method | vertices.h:127 (decl)<br>vertices.cpp:320 | ported (comment) | vertices.rs:793 `fn set_visible_directions` |
| `VertInf::directionFrom(const VertInf *) const` | method | vertices.h:128 (decl)<br>vertices.cpp:290 | ported (comment) | vertices.rs:768 `fn direction_from` |
| `VertInf::hasNeighbour(VertInf *, bool) const` | method | vertices.h:130 (decl)<br>vertices.cpp:190 | ported (comment) | vertices.rs:680 `fn has_neighbour` |
| `VertInf::orphan()` | method | vertices.h:131 (decl)<br>vertices.cpp:262 | ported (comment) | vertices.rs:747 `fn orphan` |
| `VertInf::makeTreeRootPointer(VertInf *)` | method | vertices.h:133 (decl)<br>vertices.cpp:381 | ported (comment) | vertices.rs:843 `fn make_tree_root_pointer` |
| `VertInf::treeRoot() const` | method | vertices.h:134 (decl)<br>vertices.cpp:388 | ported (comment) | vertices.rs:860 `fn tree_root` |
| `VertInf::treeRootPointer() const` | method | vertices.h:135 (decl)<br>vertices.cpp:393 | ported (comment) | vertices.rs:871 `fn tree_root_pointer` |
| `VertInf::setTreeRootPointer(VertInf **)` | method | vertices.h:136 (decl)<br>vertices.cpp:403 | ported (comment) | vertices.rs:887 `fn set_tree_root_pointer` |
| `VertInf::clearTreeRootPointer()` | method | vertices.h:137 (decl)<br>vertices.cpp:398 | ported (comment) | vertices.rs:882 `fn clear_tree_root_pointer` |
| `VertInf::setSPTFRoot(VertInf *)` | method | vertices.h:139 (decl)<br>vertices.cpp:408 | ported (comment) | vertices.rs:895 `fn set_sptf_root` |
| `VertInf::sptfRoot() const` | method | vertices.h:140 (decl)<br>vertices.cpp:415 | ported (comment) | vertices.rs:904 `fn sptf_root` |
| `XL_EDGE` | constant | vertices.h:176 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `XL_CONN` | constant | vertices.h:177 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `XH_EDGE` | constant | vertices.h:178 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `XH_CONN` | constant | vertices.h:179 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `YL_EDGE` | constant | vertices.h:180 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `YL_CONN` | constant | vertices.h:181 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `YH_EDGE` | constant | vertices.h:182 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `YH_CONN` | constant | vertices.h:183 | ported (comment) | vertices.rs:204 `const XL_EDGE` |
| `directVis(VertInf *, VertInf *)` | function | vertices.h:186 (decl)<br>vertices.cpp:422 | ported (comment) | vertices.rs:956 `fn direct_vis` |
| `VertInfList` | class | vertices.h:194 | ported (comment) | vertices.rs:1008 `struct VertInfList` |
| `VertInfList::VertInfList()` | ctor | vertices.h:197 (decl)<br>vertices.cpp:464 | ported (comment) | vertices.rs:1020 `fn new` |
| `VertInfList::addVertex(VertInf *)` | method | vertices.h:198 (decl)<br>vertices.cpp:494 | ported (comment) | vertices.rs:1062 `fn add_vertex` |
| `VertInfList::removeVertex(VertInf *)` | method | vertices.h:199 (decl)<br>vertices.cpp:557 | ported (comment) | vertices.rs:1114 `fn remove_vertex` |
| `VertInfList::getVertexByID(const VertID &)` | method | vertices.h:200 (decl)<br>vertices.cpp:660 | ported (comment) | vertices.rs:1195 `fn get_vertex_by_id` |
| `VertInfList::getVertexByPos(const Point &)` | method | vertices.h:201 (decl)<br>vertices.cpp:688 | ported (comment) | vertices.rs:1219 `fn get_vertex_by_pos` |
| `VertInfList::shapesBegin()` | method | vertices.h:202 (decl)<br>vertices.cpp:702 | ported (comment) | vertices.rs:1233 `fn shapes_begin` |
| `VertInfList::connsBegin()` | method | vertices.h:203 (decl)<br>vertices.cpp:708 | ported (comment) | vertices.rs:1239 `fn conns_begin` |
| `VertInfList::end()` | method | vertices.h:204 (decl)<br>vertices.cpp:719 | ported (comment) | vertices.rs:1249 `fn end` |
| `VertInfList::connsSize() const` | method | vertices.h:205 (decl)<br>vertices.cpp:725 | ported (comment) | vertices.rs:1255 `fn conns_size` |
| `VertInfList::shapesSize() const` | method | vertices.h:206 (decl)<br>vertices.cpp:731 | ported (comment) | vertices.rs:1261 `fn shapes_size` |
| `ShapeSet` | typedef | vertices.h:217 | ported (comment) | vertices.rs:214 `type ShapeSet` |
| `ContainsMap` | typedef | vertices.h:218 | ported (comment) | vertices.rs:216 `type ContainsMap` |
| `checkVertInfListConditions()` | macro | vertices.cpp:475 | ported (comment) | vertices.rs:1025 `fn check_vert_inf_list_conditions` |

<a id="viscluster"></a>
### viscluster

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ClusterRefList` | typedef | viscluster.h:40 | not-ported: subsumed | std::list&lt;ClusterRef*&gt;; Router::cluster_refs is a Vec (router.rs) |
| `ClusterRef` | class | viscluster.h:55 | ported (comment) | viscluster.rs:33 `struct ClusterRef` |
| `ClusterRef::ClusterRef(Router *, Polygon &, const unsigned int)` | ctor | viscluster.h:86 (decl)<br>viscluster.cpp:36 | ported (comment) | viscluster.rs:47 `fn new` |
| `ClusterRef::~ClusterRef()` | dtor | viscluster.h:93 (decl)<br>viscluster.cpp:49 | ported (comment) | viscluster.rs:66 `fn delete` |
| `ClusterRef::setNewPoly(Polygon &)` | method | viscluster.h:102 (decl)<br>viscluster.cpp:83 | ported (comment) | viscluster.rs:101 `fn set_new_poly`<br>viscluster.rs:140 `fn set_new_poly` |
| `ClusterRef::id() const` | method | viscluster.h:105 (decl)<br>viscluster.cpp:90 | ported (comment) | viscluster.rs:110 `fn id`<br>viscluster.rs:131 `fn id` |
| `ClusterRef::polygon()` | method | viscluster.h:109 (decl)<br>viscluster.cpp:96 | ported (comment) | viscluster.rs:116 `fn polygon` |
| `ClusterRef::rectangularPolygon()` | method | viscluster.h:113 (decl)<br>viscluster.cpp:102 | ported (comment) | viscluster.rs:122 `fn rectangular_polygon`<br>viscluster.rs:149 `fn rectangular_polygon` |
| `ClusterRef::router() const` | method | viscluster.h:116 (decl)<br>viscluster.cpp:108 | not-ported: subsumed | m_router back-pointer getter; the port passes &Router explicitly (router.rs data model) |
| `ClusterRef::makeActive()` | method | viscluster.h:118 (decl)<br>viscluster.cpp:60 | ported (comment) | viscluster.rs:76 `fn make_active` |
| `ClusterRef::makeInactive()` | method | viscluster.h:119 (decl)<br>viscluster.cpp:72 | ported (comment) | viscluster.rs:86 `fn make_inactive` |

<a id="visibility"></a>
### visibility

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `vertexVisibility(VertInf *, VertInf *, bool, const bool)` | function | visibility.h:34 (decl)<br>visibility.cpp:115 | ported (comment) | visibility.rs:110 `fn vertex_visibility` |
| `vertexSweep(VertInf *)` | function (static) | visibility.cpp:50 (decl)<br>visibility.cpp:436 | ported (comment) | visibility.rs:449 `fn vertex_sweep` |
| `PointPair` | class | visibility.cpp:171 | ported (comment) | visibility.rs:166 `struct PointPair` |
| `PointPair::PointPair(const Point &, VertInf *)` | ctor | visibility.cpp:174 | ported (comment) | visibility.rs:180 `fn new` |
| `PointPair::operator<(const PointPair &) const` | operator | visibility.cpp:181 | ported (comment) | visibility.rs:195 `fn less` |
| `VertSet` | typedef | visibility.cpp:210 | ported (comment) | visibility.rs:218 `struct VertSet` |
| `EdgePair` | class | visibility.cpp:213 | ported (comment) | visibility.rs:237 `struct EdgePair` |
| `EdgePair::EdgePair()` | ctor | visibility.cpp:216 | not-ported: subsumed | default ctor that only asserts it is never called (MSVC appeasement) |
| `EdgePair::EdgePair(const PointPair &, VertInf *)` | ctor | visibility.cpp:228 | ported (comment) | visibility.rs:255 `fn new` |
| `EdgePair::operator<(const EdgePair &) const` | operator | visibility.cpp:238 | ported (comment) | visibility.rs:273 `fn less` |
| `EdgePair::operator==(const EdgePair &) const` | operator | visibility.cpp:247 | ported (comment) | visibility.rs:282 `fn equals` |
| `EdgePair::operator!=(const EdgePair &) const` | operator | visibility.cpp:258 | ported (comment) | visibility.rs:292 `fn not_equals` |
| `EdgePair::setNegativeAngle()` | method | visibility.cpp:269 | ported (comment) | visibility.rs:302 `fn set_negative_angle` |
| `EdgePair::setCurrAngle(const PointPair &)` | method | visibility.cpp:273 | ported (comment) | visibility.rs:307 `fn set_curr_angle` |
| `SweepEdgeList` | typedef | visibility.cpp:316 | ported (comment) | visibility.rs:341 `type SweepEdgeList` |
| `AHEAD` | define | visibility.cpp:319 | ported (comment) | visibility.rs:355 `const AHEAD` |
| `BEHIND` | define | visibility.cpp:320 | ported (comment) | visibility.rs:355 `const AHEAD` |
| `isBoundingShape` | class | visibility.cpp:322 | ported (comment) | visibility.rs:359 `struct IsBoundingShape` |
| `isBoundingShape::isBoundingShape(ShapeSet &)` | ctor | visibility.cpp:326 | not-ported: subsumed | stores the ShapeSet; the ported predicate takes it as an argument (visibility.rs) |
| `isBoundingShape::operator()(const PointPair &)` | operator | visibility.cpp:330 | ported (comment) | visibility.rs:365 `fn call` |
| `isBoundingShape::operator=(isBoundingShape const &)` | operator | visibility.cpp:343 (decl) | not-ported: declared-undefined | private, never defined (MSVC workaround) |
| `isBoundingShape::isBoundingShape()` | ctor | visibility.cpp:344 (decl) | not-ported: declared-undefined | private default ctor, never defined (MSVC workaround) |
| `sweepVisible(SweepEdgeList &, const PointPair &, std::set<unsigned int> &, int *)` | function (static) | visibility.cpp:350 | ported (comment) | visibility.rs:374 `fn sweep_visible` |

<a id="vpsc"></a>
### vpsc

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `Variables` | typedef | vpsc.h:82 | not-ported: subsumed | std::vector&lt;Variable*&gt;; Rust Vec&lt;Variable&gt; (vpsc.rs) |
| `Constraints` | typedef | vpsc.h:83<br>vpsc.h:161 | not-ported: subsumed | std::vector&lt;Constraint*&gt;; Rust Vec of constraint indices (vpsc.rs) |
| `CompareConstraints` | class | vpsc.h:84 | not-ported: subsumed | comparator class; its operator() is ported (vpsc.rs) |
| `CompareConstraints::operator()(Constraint * const &, Constraint * const &) const` | operator | vpsc.h:86 (decl)<br>vpsc.cpp:1366 | ported (comment) | vpsc.rs:458 `fn compare_constraints` |
| `PositionStats` | struct | vpsc.h:88 | ported (comment) | vpsc.rs:187 `struct PositionStats` |
| `PositionStats::PositionStats()` | ctor | vpsc.h:89 | not-ported: subsumed | zero-initialisation; PositionStats derives Default (vpsc.rs) |
| `PositionStats::addVariable(Variable *)` | method | vpsc.h:90 (decl)<br>vpsc.cpp:688 | ported (comment) | vpsc.rs:197 `fn add_variable` |
| `Heap` | typedef | vpsc.h:97 | not-ported: subsumed | std::priority_queue; heap reimplemented with libstdc++ semantics (vpsc.rs module docs "Heaps") |
| `Block` | class | vpsc.h:100 | ported (comment) | vpsc.rs:208 `struct Block` |
| `Block::Vit` | typedef | vpsc.h:102 | not-ported: subsumed | iterator typedef |
| `Block::Cit` | typedef | vpsc.h:103 | not-ported: subsumed | iterator typedef |
| `Block::Cit_const` | typedef | vpsc.h:104 | not-ported: subsumed | iterator typedef |
| `operator<<(ostream &, const Block &)` | operator (friend) | vpsc.h:106 (decl)<br>vpsc.cpp:1281 | ported (comment) | vpsc.rs:959 `fn block_to_string` |
| `Block::Block(Blocks *, Variable * const)` | ctor | vpsc.h:113 (decl)<br>vpsc.cpp:721 | ported (comment) | vpsc.rs:1224 `fn block_new` |
| `Block::~Block()` | dtor | vpsc.h:114 (decl)<br>vpsc.cpp:752 | not-ported: subsumed | deletes member containers; Rust drop |
| `Block::findMinLM()` | method | vpsc.h:115 (decl)<br>vpsc.cpp:1135 | ported (comment) | vpsc.rs:1474 `fn block_find_min_lm` |
| `Block::findMinLMBetween(Variable * const, Variable * const)` | method | vpsc.h:116 (decl)<br>vpsc.cpp:1146 | ported (comment) | vpsc.rs:1485 `fn block_find_min_lm_between` |
| `Block::findMinInConstraint()` | method | vpsc.h:117 (decl)<br>vpsc.cpp:864 | ported (comment) | vpsc.rs:1357 `fn block_find_min_in_constraint` |
| `Block::findMinOutConstraint()` | method | vpsc.h:118 (decl)<br>vpsc.cpp:913 | ported (comment) | vpsc.rs:1387 `fn block_find_min_out_constraint` |
| `Block::deleteMinInConstraint()` | method | vpsc.h:119 (decl)<br>vpsc.cpp:923 | ported (comment) | vpsc.rs:1403 `fn block_delete_min_in_constraint` |
| `Block::deleteMinOutConstraint()` | method | vpsc.h:120 (decl)<br>vpsc.cpp:931 | ported (comment) | vpsc.rs:1408 `fn block_delete_min_out_constraint` |
| `Block::updateWeightedPosition()` | method | vpsc.h:121 (decl)<br>vpsc.cpp:738 | ported (comment) | vpsc.rs:1256 `fn block_update_weighted_position` |
| `Block::merge(Block *, Constraint *, double)` | method | vpsc.h:122 (decl)<br>vpsc.cpp:807 | ported (comment) | vpsc.rs:1317 `fn block_merge_dist` |
| `Block::merge(Block *, Constraint *)` | method | vpsc.h:123 (decl)<br>vpsc.cpp:781 | ported (comment) | vpsc.rs:1299 `fn block_merge` |
| `Block::mergeIn(Block *)` | method | vpsc.h:124 (decl)<br>vpsc.cpp:838 | ported (comment) | vpsc.rs:1333 `fn block_merge_in` |
| `Block::mergeOut(Block *)` | method | vpsc.h:125 (decl)<br>vpsc.cpp:855 | ported (comment) | vpsc.rs:1346 `fn block_merge_out` |
| `Block::split(Block *&, Block *&, Constraint *)` | method | vpsc.h:126 (decl)<br>vpsc.cpp:1259 | ported (comment) | vpsc.rs:1627 `fn block_split` |
| `Block::splitBetween(Variable * const, Variable * const, Block *&, Block *&)` | method | vpsc.h:127 (decl)<br>vpsc.cpp:1237 | ported (comment) | vpsc.rs:1609 `fn block_split_between` |
| `Block::setUpInConstraints()` | method | vpsc.h:128 (decl)<br>vpsc.cpp:758 | ported (comment) | vpsc.rs:1269 `fn block_set_up_in_constraints` |
| `Block::setUpOutConstraints()` | method | vpsc.h:129 (decl)<br>vpsc.cpp:761 | ported (comment) | vpsc.rs:1274 `fn block_set_up_out_constraints` |
| `Block::cost()` | method | vpsc.h:130 (decl)<br>vpsc.cpp:1273 | ported (comment) | vpsc.rs:1641 `fn block_cost` |
| `Block::getActivePathBetween(Constraints &, Variable const *, Variable const *, Variable const *) const` | method | vpsc.h:135 (decl)<br>vpsc.cpp:1183 | ported (comment) | vpsc.rs:1537 `fn block_get_active_path_between` |
| `Block::isActiveDirectedPathBetween(Variable const *, Variable const *) const` | method | vpsc.h:137 (decl)<br>vpsc.cpp:1207 | ported (comment) | vpsc.rs:1570 `fn block_is_active_directed_path_between` |
| `Block::getActiveDirectedPathBetween(Constraints &, Variable const *, Variable const *) const` | method | vpsc.h:139 (decl)<br>vpsc.cpp:1218 | ported (comment) | vpsc.rs:1587 `fn block_get_active_directed_path_between` |
| `Block::Direction: NONE, LEFT, RIGHT` | enum | vpsc.h:141 | not-ported: dead-code | only used by the commented-out Block::compute_dfdv_between() |
| `Block::Pair` | typedef | vpsc.h:142 | not-ported: dead-code | only used by the commented-out Block::compute_dfdv_between() |
| `Block::reset_active_lm(Variable * const, Variable * const)` | method | vpsc.h:143 (decl)<br>vpsc.cpp:1093 | ported (comment) | vpsc.rs:396 `fn reset_active_lm`<br>vpsc.rs:1463 `fn block_reset_active_lm` |
| `Block::list_active(Variable * const, Variable * const)` | method | vpsc.h:144 (decl)<br>vpsc.cpp:1109 | ported (comment) | vpsc.rs:1470 `fn block_find_min_lm` |
| `Block::compute_dfdv(Variable * const, Variable * const)` | method | vpsc.h:145 (decl)<br>vpsc.cpp:967 | ported (comment) | vpsc.rs:319 `fn compute_dfdv`<br>vpsc.rs:1442 `fn block_compute_dfdv` |
| `Block::compute_dfdv(Variable * const, Variable * const, Constraint *&)` | method | vpsc.h:146 (decl)<br>vpsc.cpp:946 | ported (comment) | vpsc.rs:282 `fn compute_dfdv_min`<br>vpsc.rs:1425 `fn block_compute_dfdv_min` |
| `Block::split_path(Variable *, Variable * const, Variable * const, Constraint *&, bool)` | method | vpsc.h:147 (decl)<br>vpsc.cpp:995 | ported (comment) | vpsc.rs:349 `fn split_path`<br>vpsc.rs:1447 `fn block_split_path` |
| `Block::canFollowLeft(Constraint const *, Variable const *) const` | method (inline) | vpsc.h:149 (decl)<br>vpsc.cpp:934 | ported (comment) | vpsc.rs:270 `fn can_follow_left`<br>vpsc.rs:1413 `fn can_follow_left` |
| `Block::canFollowRight(Constraint const *, Variable const *) const` | method (inline) | vpsc.h:150 (decl)<br>vpsc.cpp:937 | ported (comment) | vpsc.rs:276 `fn can_follow_right`<br>vpsc.rs:1419 `fn can_follow_right` |
| `Block::populateSplitBlock(Block *, Variable *, Variable const *)` | method | vpsc.h:151 (decl)<br>vpsc.cpp:1169 | ported (comment) | vpsc.rs:1507 `fn block_populate_split_block` |
| `Block::addVariable(Variable *)` | method | vpsc.h:152 (decl)<br>vpsc.cpp:704 | ported (comment) | vpsc.rs:1243 `fn block_add_variable` |
| `Block::setUpConstraintHeap(Heap *&, bool)` | method | vpsc.h:153 (decl)<br>vpsc.cpp:764 | ported (comment) | vpsc.rs:1279 `fn block_set_up_constraint_heap` |
| `Variable` | class | vpsc.h:162 | ported (comment) | vpsc.rs:72 `struct Variable` |
| `operator<<(std::ostream &, const Variable &)` | operator (friend) | vpsc.h:164 (decl)<br>vpsc.cpp:1389 | ported (comment) | vpsc.rs:948 `fn variable_to_string` |
| `Variable::Variable(const int, const double, const double, const double)` | ctor (inline) | vpsc.h:181 | ported (comment) | vpsc.rs:96 `fn new` |
| `Variable::dfdv() const` | method | vpsc.h:193 | ported (comment) | vpsc.rs:437 `fn dfdv_of`<br>vpsc.rs:937 `fn dfdv` |
| `Variable::position() const` | method (inline) | vpsc.h:197 | ported (comment) | vpsc.rs:421 `fn position_of`<br>vpsc.rs:932 `fn position` |
| `Variable::unscaledPosition() const` | method (inline) | vpsc.h:200 | ported (comment) | vpsc.rs:428 `fn unscaled_position_of` |
| `Constraint` | class | vpsc.h:208 | ported (comment) | vpsc.rs:125 `struct Constraint` |
| `Constraint::Constraint(Variable *, Variable *, double, bool)` | ctor | vpsc.h:211 (decl)<br>vpsc.cpp:1293 | ported (comment) | vpsc.rs:165 `fn new` |
| `Constraint::~Constraint()` | dtor | vpsc.h:212 (decl)<br>vpsc.cpp:1309 | not-ported: subsumed | empty destructor (body commented out) |
| `Constraint::slack() const` | method (inline) | vpsc.h:213 | ported (comment) | vpsc.rs:442 `fn slack_of`<br>vpsc.rs:927 `fn slack` |
| `Constraint::toString() const` | method | vpsc.h:228 (decl)<br>vpsc.cpp:1323 | ported (comment) | vpsc.rs:152 `fn to_string` |
| `operator<<(std::ostream &, const Constraint &)` | operator (friend) | vpsc.h:230 (decl)<br>vpsc.cpp:1340 | ported (comment) | vpsc.rs:974 `fn constraint_to_string` |
| `Blocks` | class | vpsc.h:247 | ported (comment) | vpsc.rs:222 `struct Blocks` |
| `Blocks::Blocks(Variables const &)` | ctor | vpsc.h:250 (decl)<br>vpsc.cpp:491 | ported (comment) | vpsc.rs:1080 `fn blocks_new` |
| `Blocks::~Blocks()` | dtor | vpsc.h:251 (decl)<br>vpsc.cpp:498 | not-ported: subsumed | deletes blocks and resets blockTimeCtr of the dying object; Rust drop |
| `Blocks::mergeLeft(Block *)` | method | vpsc.h:252 (decl)<br>vpsc.cpp:546 | ported (comment) | vpsc.rs:1121 `fn blocks_merge_left` |
| `Blocks::mergeRight(Block *)` | method | vpsc.h:253 (decl)<br>vpsc.cpp:580 | ported (comment) | vpsc.rs:1154 `fn blocks_merge_right` |
| `Blocks::split(Block *, Block *&, Block *&, Constraint *)` | method | vpsc.h:254 (decl)<br>vpsc.cpp:651 | ported (comment) | vpsc.rs:1191 `fn blocks_split` |
| `Blocks::totalOrder()` | method | vpsc.h:255 (decl)<br>vpsc.cpp:513 | ported (comment) | vpsc.rs:1091 `fn blocks_total_order` |
| `Blocks::cleanup()` | method | vpsc.h:256 (decl)<br>vpsc.cpp:614 | ported (comment) | vpsc.rs:1184 `fn blocks_cleanup` |
| `Blocks::cost()` | method | vpsc.h:257 (decl)<br>vpsc.cpp:678 | ported (comment) | vpsc.rs:1211 `fn blocks_cost` |
| `Blocks::size() const` | method (inline) | vpsc.h:259 (decl)<br>vpsc.h:273 | not-ported: subsumed | inline `m_blocks.size()`; Rust uses bs.m_blocks.len() directly |
| `Blocks::at(size_t) const` | method (inline) | vpsc.h:260 (decl)<br>vpsc.h:278 | not-ported: subsumed | inline `m_blocks[index]`; Rust indexes bs.m_blocks directly |
| `Blocks::insert(Block *)` | method (inline) | vpsc.h:261 (decl)<br>vpsc.h:283 | not-ported: subsumed | inline `m_blocks.push_back()`; Rust pushes to bs.m_blocks directly |
| `Blocks::dfsVisit(Variable *, list<Variable *> *)` | method | vpsc.h:265 (decl)<br>vpsc.cpp:527 | ported (comment) | vpsc.rs:1107 `fn blocks_dfs_visit` |
| `Blocks::removeBlock(Block *)` | method | vpsc.h:266 (decl)<br>vpsc.cpp:608 | ported (comment) | vpsc.rs:1179 `fn blocks_remove_block` |
| `UnsatisfiableException` | struct | vpsc.h:288 | ported (comment) | vpsc.rs:234 `struct UnsatisfiableException` |
| `UnsatisfiedConstraint` | struct | vpsc.h:291 | ported (name) | vpsc.rs:243 |
| `UnsatisfiedConstraint::UnsatisfiedConstraint(Constraint &)` | ctor | vpsc.h:292 | not-ported: subsumed | stores the constraint; Rust struct literal (vpsc.rs UnsatisfiedConstraint) |
| `IncSolver` | class | vpsc.h:298 | ported (comment) | vpsc.rs:569 `struct IncSolver` |
| `IncSolver::satisfy()` | method | vpsc.h:301 (decl)<br>vpsc.cpp:274 | ported (comment) | vpsc.rs:780 `fn satisfy` |
| `IncSolver::solve()` | method | vpsc.h:302 (decl)<br>vpsc.cpp:243 | ported (comment) | vpsc.rs:763 `fn solve` |
| `IncSolver::moveBlocks()` | method | vpsc.h:303 (decl)<br>vpsc.cpp:367 | ported (comment) | vpsc.rs:850 `fn move_blocks` |
| `IncSolver::splitBlocks()` | method | vpsc.h:304 (decl)<br>vpsc.cpp:383 | ported (comment) | vpsc.rs:859 `fn split_blocks` |
| `IncSolver::IncSolver(Variables const &, Constraints const &)` | ctor | vpsc.h:305 (decl)<br>vpsc.cpp:61 | ported (comment) | vpsc.rs:591 `fn new` |
| `IncSolver::~IncSolver()` | dtor | vpsc.h:307 (decl)<br>vpsc.cpp:92 | not-ported: subsumed | deletes the Blocks; Rust ownership drops them |
| `IncSolver::addConstraint(Constraint *)` | method | vpsc.h:308 (decl)<br>vpsc.cpp:96 | ported (comment) | vpsc.rs:629 `fn add_constraint` |
| `IncSolver::getVariables()` | method | vpsc.h:309 | ported (comment) | vpsc.rs:647 `fn get_variables` |
| `IncSolver::printBlocks()` | method | vpsc.h:318 (decl)<br>vpsc.cpp:107 | ported (comment) | vpsc.rs:1014 `fn print_blocks` |
| `IncSolver::copyResult()` | method | vpsc.h:319 (decl)<br>vpsc.cpp:124 | ported (comment) | vpsc.rs:657 `fn copy_result` |
| `IncSolver::constraintGraphIsCyclic(const unsigned, Variable * const)` | method | vpsc.h:321 (decl)<br>vpsc.cpp:137 | ported (comment) | vpsc.rs:666 `fn constraint_graph_is_cyclic` |
| `IncSolver::blockGraphIsCyclic()` | method | vpsc.h:322 (decl)<br>vpsc.cpp:184 | ported (comment) | vpsc.rs:710 `fn block_graph_is_cyclic` |
| `IncSolver::mostViolated(Constraints &)` | method | vpsc.h:325 (decl)<br>vpsc.cpp:431 | ported (comment) | vpsc.rs:888 `fn most_violated` |
| `delete_object` | struct | vpsc.h:328 | not-ported: subsumed | `delete ptr` functor for std::for_each; Rust drop |
| `delete_object::operator()(T *)` | operator (template) | vpsc.h:331 | not-ported: subsumed | `delete ptr` functor for std::for_each; Rust drop |
| `constraintsRemovingRedundantEqualities(Variables const &, Constraints const &)` | function | vpsc.h:334 (decl)<br>vpsc.cpp:1468 | ported (comment) | vpsc.rs:1728 `fn constraints_removing_redundant_equalities` |
| `ZERO_UPPERBOUND` | constant | vpsc.cpp:57 | ported (comment) | vpsc.rs:56 `const ZERO_UPPERBOUND` |
| `LAGRANGIAN_TOLERANCE` | constant | vpsc.cpp:58 | ported (comment) | vpsc.rs:58 `const LAGRANGIAN_TOLERANCE` |
| `node` | struct | vpsc.cpp:132 | not-ported: subsumed | helper struct of constraintGraphIsCyclic; Rust uses index maps (vpsc.rs) |
| `__NOTNAN(p)` | macro | vpsc.cpp:488 | not-ported: subsumed | NaN self-compare inside COLA_ASSERT; `debug_assert!(!x.is_nan())` in vpsc.rs |
| `VarOffsetMapList` | typedef | vpsc.cpp:1397 | not-ported: subsumed | std::list&lt;std::map&lt;Variable*,double&gt;&gt;; EqualityConstraintSet::variable_groups is Vec&lt;BTreeMap&lt;VarIdx, f64&gt;&gt; (vpsc.rs) |
| `EqualityConstraintSet` | class | vpsc.cpp:1399 | ported (comment) | vpsc.rs:1658 `struct EqualityConstraintSet` |
| `EqualityConstraintSet::EqualityConstraintSet(Variables)` | ctor | vpsc.cpp:1402 | ported (comment) | vpsc.rs:1668 `fn new` |
| `EqualityConstraintSet::isRedundant(Variable *, Variable *, double)` | method | vpsc.cpp:1411 | ported (comment) | vpsc.rs:1679 `fn is_redundant` |
| `EqualityConstraintSet::mergeSets(Variable *, Variable *, double)` | method | vpsc.cpp:1426 | ported (comment) | vpsc.rs:1695 `fn merge_sets` |
| `EqualityConstraintSet::setForVar(Variable *)` | method | vpsc.cpp:1452 | ported (comment) | vpsc.rs:1722 `fn set_for_var` |

## Upstream Fixture Index

Generated from `vendor/adaptagrams/cola/libavoid/tests/*.cpp` and the `fixtures!` list in
`tests/fixtures_ops.rs` (op scripts in `tests/fixtures/*.ops`):
`golden` = ops-driven, compared byte-for-byte against `tests/golden-bump/`; `smoke` = does not
compile against the current upstream API, run as a smoke test only; `slow golden` / `slow smoke`
= the same, gated behind `LIBAVOID_SLOW_FIXTURES=1` (upstream takes minutes).  Status of the
last audit: every golden fixture passes (`cargo test --release`, `tools/fixtures/run_all.sh`).

golden: 77, slow golden: 3, slow smoke: 1, smoke: 2 (of 83 upstream fixtures).

| Fixture | Status |
| --- | --- |
| `2junctions.cpp` | golden |
| `buildOrthogonalChannelInfo1.cpp` | golden |
| `checkpointNudging1.cpp` | golden |
| `checkpointNudging2.cpp` | golden |
| `checkpointNudging3.cpp` | golden |
| `checkpoints01.cpp` | golden |
| `checkpoints02.cpp` | golden |
| `checkpoints03.cpp` | golden |
| `complex.cpp` | golden |
| `connectionpin01.cpp` | golden |
| `connectionpin02.cpp` | golden |
| `connectionpin03.cpp` | golden |
| `connendmove.cpp` | golden |
| `corneroverlap01.cpp` | smoke |
| `endlessLoop01.cpp` | golden |
| `example.cpp` | golden |
| `finalSegmentNudging1.cpp` | golden |
| `finalSegmentNudging2.cpp` | golden |
| `finalSegmentNudging3.cpp` | golden |
| `forwardFlowingConnectors01.cpp` | golden |
| `freeFloatingDirection01.cpp` | golden |
| `hola01.cpp` | golden |
| `hyperedge01.cpp` | golden |
| `hyperedge02.cpp` | golden |
| `hyperedgeLoop1.cpp` | golden |
| `hyperedgeRerouting01.cpp` | golden |
| `improveHyperedge01.cpp` | golden |
| `improveHyperedge02.cpp` | golden |
| `improveHyperedge03.cpp` | golden |
| `improveHyperedge04.cpp` | golden |
| `improveHyperedge05.cpp` | golden |
| `improveHyperedge06.cpp` | golden |
| `infinity.cpp` | golden |
| `inline.cpp` | golden |
| `inlineOverlap09.cpp` | golden |
| `inlineOverlap10.cpp` | golden |
| `inlineOverlap11.cpp` | golden |
| `inlineShapes.cpp` | golden |
| `inlineoverlap01.cpp` | golden |
| `inlineoverlap02.cpp` | golden |
| `inlineoverlap03.cpp` | golden |
| `inlineoverlap04.cpp` | golden |
| `inlineoverlap05.cpp` | golden |
| `inlineoverlap06.cpp` | golden |
| `inlineoverlap07.cpp` | golden |
| `inlineoverlap08.cpp` | golden |
| `junction01.cpp` | golden |
| `junction02.cpp` | golden |
| `junction03.cpp` | golden |
| `junction04.cpp` | golden |
| `latesetup.cpp` | golden |
| `lineSegWrapperCrash1.cpp` | golden |
| `lineSegWrapperCrash2.cpp` | golden |
| `lineSegWrapperCrash3.cpp` | golden |
| `lineSegWrapperCrash4.cpp` | golden |
| `lineSegWrapperCrash5.cpp` | golden |
| `lineSegWrapperCrash6.cpp` | golden |
| `lineSegWrapperCrash7.cpp` | golden |
| `lineSegWrapperCrash8.cpp` | golden |
| `multiconnact.cpp` | golden |
| `node1.cpp` | golden |
| `nudgeCrossing01.cpp` | smoke |
| `nudgeintobug.cpp` | golden |
| `nudgeold.cpp` | golden |
| `nudgingSkipsCheckpoint01.cpp` | golden |
| `nudgingSkipsCheckpoint02.cpp` | golden |
| `orderassertion.cpp` | golden |
| `orthordering01.cpp` | golden |
| `orthordering02.cpp` | golden |
| `overlappingRects.cpp` | golden |
| `penaltyRerouting01.cpp` | golden |
| `performance01.cpp` | slow golden |
| `reallyslowrouting.cpp` | slow smoke |
| `removeJunctions01.cpp` | golden |
| `restrictedNudging.cpp` | golden |
| `slowrouting.cpp` | slow golden |
| `tjunct.cpp` | golden |
| `treeRootCrash01.cpp` | golden |
| `treeRootCrash02.cpp` | golden |
| `unsatisfiableRangeAssertion.cpp` | slow golden |
| `validPaths01.cpp` | golden |
| `validPaths02.cpp` | golden |
| `vertlineassertion.cpp` | golden |
