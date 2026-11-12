# libavoid Porting Inventory

<!-- GENERATED FILE: do not edit by hand.  Regenerate with `tools/audit/inventory.py`. -->

Function-level inventory of upstream libavoid (`vendor/adaptagrams/cola/libavoid`, excluding
`tests/`, `debughandler.h`, `timer.{h,cpp}` and `dllexport.h`) against the Rust port in `src/avoid/`.

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
| [connectionpin.cpp/connectionpin.h](#connectionpin) | 34 | 32 | 1 | 0 | 0 | 0 | 1 | 0 | 0 |
| [connector.cpp/connector.h](#connector) | 91 | 87 | 1 | 0 | 0 | 0 | 3 | 0 | 0 |
| [connend.cpp/connend.h](#connend) | 28 | 25 | 1 | 0 | 0 | 0 | 2 | 0 | 0 |
| [debug.h](#debug) | 6 | 0 | 5 | 0 | 0 | 0 | 0 | 1 | 0 |
| [geometry.cpp/geometry.h](#geometry) | 23 | 22 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| [geomtypes.cpp/geomtypes.h](#geomtypes) | 65 | 63 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [graph.cpp/graph.h](#graph) | 43 | 41 | 1 | 0 | 0 | 0 | 1 | 0 | 0 |
| [hyperedge.cpp/hyperedge.h](#hyperedge) | 24 | 22 | 1 | 0 | 0 | 0 | 1 | 0 | 0 |
| [hyperedgeimprover.cpp/hyperedgeimprover.h](#hyperedgeimprover) | 40 | 33 | 1 | 0 | 1 | 0 | 5 | 0 | 0 |
| [hyperedgetree.cpp/hyperedgetree.h](#hyperedgetree) | 40 | 37 | 2 | 0 | 0 | 0 | 1 | 0 | 0 |
| [junction.cpp/junction.h](#junction) | 15 | 13 | 1 | 0 | 0 | 0 | 1 | 0 | 0 |
| [makepath.cpp/makepath.h](#makepath) | 39 | 32 | 1 | 0 | 0 | 0 | 6 | 0 | 0 |
| [mtst.cpp/mtst.h](#mtst) | 30 | 26 | 1 | 0 | 0 | 1 | 2 | 0 | 0 |
| [obstacle.cpp/obstacle.h](#obstacle) | 31 | 26 | 1 | 0 | 0 | 1 | 3 | 0 | 0 |
| [orthogonal.cpp/orthogonal.h](#orthogonal) | 99 | 89 | 0 | 0 | 0 | 2 | 8 | 0 | 0 |
| [router.cpp/router.h](#router) | 120 | 100 | 6 | 0 | 1 | 0 | 13 | 0 | 0 |
| [scanline.cpp/scanline.h](#scanline) | 35 | 31 | 0 | 0 | 0 | 0 | 4 | 0 | 0 |
| [shape.cpp/shape.h](#shape) | 13 | 10 | 1 | 0 | 0 | 1 | 1 | 0 | 0 |
| [vertices.cpp/vertices.h](#vertices) | 74 | 71 | 0 | 0 | 0 | 0 | 3 | 0 | 0 |
| [viscluster.cpp/viscluster.h](#viscluster) | 11 | 9 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| [visibility.cpp/visibility.h](#visibility) | 23 | 19 | 0 | 0 | 0 | 2 | 2 | 0 | 0 |
| [vpsc.cpp/vpsc.h](#vpsc) | 102 | 74 | 5 | 0 | 2 | 0 | 21 | 0 | 0 |
| **total** | **1007** | **879** | **29** | **0** | **5** | **7** | **85** | **2** | **0** |

Ported items by mapping method: `upstream:` comment 856, name match 19, manual mapping in `classification.txt` 4.

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
| `CONNECTIONPIN_UNSET` | constant | connectionpin.h:44 | ported (comment) | connectionpin.rs:51 `const CONNECTIONPIN_UNSET` |
| `CONNECTIONPIN_CENTRE` | constant | connectionpin.h:45 | ported (comment) | connectionpin.rs:51 `const CONNECTIONPIN_UNSET` |
| `ConnectionPinIds` | typedef | connectionpin.h:53 | ported (comment) | connectionpin.rs:55 `type ConnectionPinIds` |
| `ATTACH_POS_TOP` | constant | connectionpin.h:57 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_CENTRE` | constant | connectionpin.h:58 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_BOTTOM` | constant | connectionpin.h:59 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_LEFT` | constant | connectionpin.h:60 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_RIGHT` | constant | connectionpin.h:61 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_MIN_OFFSET` | constant | connectionpin.h:63 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ATTACH_POS_MAX_OFFSET` | constant | connectionpin.h:64 | ported (comment) | connectionpin.rs:58 `const ATTACH_POS_TOP` |
| `ShapeConnectionPin` | class | connectionpin.h:96 | ported (comment) | connectionpin.rs:69 `struct ShapeConnectionPin` |
| `ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, const unsigned int, const double, const double, const bool, const double, const ConnDirFlags)` | ctor | connectionpin.h:169 (decl)<br>connectionpin.cpp:38 | ported (comment) | connectionpin.rs:96 `fn new_shape` |
| `ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, const unsigned int, const double, const double, const double, const ConnDirFlags)` | ctor | connectionpin.h:175 (decl)<br>connectionpin.cpp:57 | ported (comment) | connectionpin.rs:132 `fn new_shape_proportional` |
| `ShapeConnectionPin::ShapeConnectionPin(JunctionRef *, const unsigned int, const ConnDirFlags)` | ctor | connectionpin.h:197 (decl)<br>connectionpin.cpp:137 | ported (comment) | connectionpin.rs:219 `fn new_junction` |
| `ShapeConnectionPin::~ShapeConnectionPin()` | dtor | connectionpin.h:204 (decl)<br>connectionpin.cpp:171 | ported (comment) | connectionpin.rs:268 `fn delete` |
| `ShapeConnectionPin::setConnectionCost(const double)` | method | connectionpin.h:213 (decl)<br>connectionpin.cpp:215 | ported (comment) | connectionpin.rs:312 `fn set_connection_cost`<br>connectionpin.rs:587 `fn set_connection_cost` |
| `ShapeConnectionPin::position(const Polygon &) const` | method | connectionpin.h:219 (decl)<br>connectionpin.cpp:245 | ported (comment) | connectionpin.rs:345 `fn position`<br>connectionpin.rs:614 `fn position` |
| `ShapeConnectionPin::directions() const` | method | connectionpin.h:226 (decl)<br>connectionpin.cpp:331 | ported (comment) | connectionpin.rs:413 `fn directions`<br>connectionpin.rs:623 `fn directions` |
| `ShapeConnectionPin::setExclusive(const bool)` | method | connectionpin.h:236 (decl)<br>connectionpin.cpp:223 | ported (comment) | connectionpin.rs:319 `fn set_exclusive`<br>connectionpin.rs:596 `fn set_exclusive` |
| `ShapeConnectionPin::isExclusive() const` | method | connectionpin.h:243 (decl)<br>connectionpin.cpp:228 | ported (comment) | connectionpin.rs:324 `fn is_exclusive`<br>connectionpin.rs:605 `fn is_exclusive` |
| `ShapeConnectionPin::ids() const` | method | connectionpin.h:245 (decl)<br>connectionpin.cpp:388 | ported (comment) | connectionpin.rs:440 `fn ids`<br>connectionpin.rs:632 `fn ids` |
| `ShapeConnectionPin::operator==(const ShapeConnectionPin &) const` | operator | connectionpin.h:247 (decl)<br>connectionpin.cpp:399 | ported (comment) | connectionpin.rs:454 `fn equals` |
| `ShapeConnectionPin::operator<(const ShapeConnectionPin &) const` | operator | connectionpin.h:248 (decl)<br>connectionpin.cpp:432 | ported (comment) | connectionpin.rs:479 `fn less_than` |
| `ShapeConnectionPin::commonInitForShapeConnection()` | method | connectionpin.h:256 (decl)<br>connectionpin.cpp:77 | ported (comment) | connectionpin.rs:154 `fn common_init_for_shape_connection` |
| `ShapeConnectionPin::updatePosition(const Point &)` | method | connectionpin.h:257 (decl)<br>connectionpin.cpp:233 | ported (comment) | connectionpin.rs:329 `fn update_position_point` |
| `ShapeConnectionPin::updatePosition(const Polygon &)` | method | connectionpin.h:258 (decl)<br>connectionpin.cpp:239 | ported (comment) | connectionpin.rs:335 `fn update_position_poly` |
| `ShapeConnectionPin::updatePositionAndVisibility()` | method | connectionpin.h:259 (decl)<br>connectionpin.cpp:199 | ported (comment) | connectionpin.rs:292 `fn update_position_and_visibility` |
| `ShapeConnectionPin::updateVisibility()` | method | connectionpin.h:260 (decl)<br>connectionpin.cpp:206 | ported (comment) | connectionpin.rs:301 `fn update_visibility` |
| `ShapeConnectionPin::outputCode(FILE *) const` | method | connectionpin.h:261 (decl)<br>connectionpin.cpp:363 | not-ported: debug-output | emits C++ reproduction code (noted in connectionpin.rs) |
| `ShapeConnectionPin::containingObjectId() const` | method | connectionpin.h:262 (decl)<br>connectionpin.cpp:393 | ported (comment) | connectionpin.rs:445 `fn containing_object_id` |
| `CmpConnPinPtr` | class | connectionpin.h:283 | ported (comment) | connectionpin.rs:512 `fn cmp_conn_pin_ptr` |
| `CmpConnPinPtr::CmpConnPinPtr()` | ctor | connectionpin.h:286 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpConnPinPtr::operator()(const ShapeConnectionPin *, const ShapeConnectionPin *) const` | operator | connectionpin.h:289 | ported (manual) | connectionpin.rs fn cmp_conn_pin_ptr() (class and operator() ported as one fn) |
| `ShapeConnectionPinSet` | typedef | connectionpin.h:296 | ported (comment) | connectionpin.rs:526 `struct ShapeConnectionPinSet` |

<a id="connector"></a>
### connector

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ConnRefList` | typedef | connector.h:48 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `ConnType: ConnType_None, ConnType_PolyLine, ConnType_Orthogonal` | enum | connector.h:53 | ported (comment) | connector.rs:64 `enum ConnType` |
| `Checkpoint` | class | connector.h:68 | ported (comment) | connector.rs:74 `struct Checkpoint` |
| `Checkpoint::Checkpoint(const Point &)` | ctor | connector.h:77 | ported (comment) | connector.rs:95 `fn new` |
| `Checkpoint::Checkpoint(const Point &, ConnDirFlags, ConnDirFlags)` | ctor | connector.h:95 | ported (comment) | connector.rs:104 `fn with_directions` |
| `Checkpoint::Checkpoint()` | ctor | connector.h:102 | ported (comment) | connector.rs:83 `fn default` |
| `ConnRef` | class | connector.h:131 | ported (comment) | connector.rs:125 `struct ConnRef` |
| `ConnRef::ConnRef(Router *, const unsigned int)` | ctor | connector.h:155 (decl)<br>connector.cpp:48 | ported (comment) | connector.rs:196 `fn new` |
| `ConnRef::ConnRef(Router *, const ConnEnd &, const ConnEnd &, const unsigned int)` | ctor | connector.h:175 (decl)<br>connector.cpp:77 | ported (comment) | connector.rs:214 `fn new_with_ends` |
| `ConnRef::~ConnRef()` | dtor | connector.h:187 (decl)<br>connector.cpp:107 | ported (comment) | connector.rs:238 `fn delete` |
| `ConnRef::setEndpoints(const ConnEnd &, const ConnEnd &)` | method | connector.h:198 (decl)<br>connector.cpp:323 | ported (comment) | connector.rs:435 `fn set_endpoints`<br>connector.rs:1287 `fn set_endpoints` |
| `ConnRef::setSourceEndpoint(const ConnEnd &)` | method | connector.h:207 (decl)<br>connector.cpp:336 | ported (comment) | connector.rs:446 `fn set_source_endpoint`<br>connector.rs:1296 `fn set_source_endpoint` |
| `ConnRef::setDestEndpoint(const ConnEnd &)` | method | connector.h:216 (decl)<br>connector.cpp:342 | ported (comment) | connector.rs:451 `fn set_dest_endpoint`<br>connector.rs:1305 `fn set_dest_endpoint` |
| `ConnRef::id() const` | method | connector.h:220 (decl)<br>connector.cpp:677 | ported (comment) | connector.rs:717 `fn id`<br>connector.rs:1323 `fn id` |
| `ConnRef::router() const` | method | connector.h:224 (decl)<br>connector.cpp:797 | not-ported: subsumed | m_router back-pointer getter; the port passes &Router explicitly (router.rs data model) |
| `ConnRef::needsRepaint() const` | method | connector.h:235 (decl)<br>connector.cpp:671 | ported (comment) | connector.rs:711 `fn needs_repaint`<br>connector.rs:1332 `fn needs_repaint` |
| `ConnRef::route() const` | method | connector.h:248 (decl)<br>connector.cpp:590 | ported (comment) | connector.rs:631 `fn route`<br>connector.rs:1341 `fn route` |
| `ConnRef::displayRoute()` | method | connector.h:259 (decl)<br>connector.cpp:647 | ported (comment) | connector.rs:688 `fn display_route`<br>connector.rs:1350 `fn display_route` |
| `ConnRef::setCallback(void(* cb)(void *), void *)` | method | connector.h:271 (decl)<br>connector.cpp:775 | ported (comment) | connector.rs:809 `fn set_callback`<br>connector.rs:1359 `fn set_callback` |
| `ConnRef::routingType() const` | method | connector.h:276 (decl)<br>connector.cpp:170 | ported (comment) | connector.rs:292 `fn routing_type`<br>connector.rs:1363 `fn routing_type` |
| `ConnRef::setRoutingType(ConnType)` | method | connector.h:288 (decl)<br>connector.cpp:176 | ported (comment) | connector.rs:298 `fn set_routing_type`<br>connector.rs:1372 `fn set_routing_type` |
| `ConnRef::splitAtSegment(const size_t)` | method | connector.h:301 (decl)<br>connector.cpp:694 | ported (comment) | connector.rs:725 `fn split_at_segment`<br>connector.rs:1381 `fn split_at_segment` |
| `ConnRef::setRoutingCheckpoints(const std::vector<Checkpoint> &)` | method | connector.h:315 (decl)<br>connector.cpp:196 | ported (comment) | connector.rs:315 `fn set_routing_checkpoints`<br>connector.rs:1390 `fn set_routing_checkpoints` |
| `ConnRef::routingCheckpoints() const` | method | connector.h:321 (decl)<br>connector.cpp:190 | ported (comment) | connector.rs:310 `fn routing_checkpoints`<br>connector.rs:1399 `fn routing_checkpoints` |
| `ConnRef::endpointConnEnds() const` | method | connector.h:340 (decl)<br>connector.cpp:511 | ported (comment) | connector.rs:546 `fn endpoint_conn_ends`<br>connector.rs:1408 `fn endpoint_conn_ends` |
| `ConnRef::src() const` | method | connector.h:344 (decl)<br>connector.cpp:729 | ported (comment) | connector.rs:766 `fn src`<br>connector.rs:1417 `fn src` |
| `ConnRef::dst() const` | method | connector.h:348 (decl)<br>connector.cpp:735 | ported (comment) | connector.rs:772 `fn dst`<br>connector.rs:1426 `fn dst` |
| `ConnRef::setFixedRoute(const PolyLine &)` | method | connector.h:368 (decl)<br>connector.cpp:621 | ported (comment) | connector.rs:657 `fn set_fixed_route`<br>connector.rs:1435 `fn set_fixed_route` |
| `ConnRef::setFixedExistingRoute()` | method | connector.h:384 (decl)<br>connector.cpp:614 | ported (comment) | connector.rs:650 `fn set_fixed_existing_route`<br>connector.rs:1444 `fn set_fixed_existing_route` |
| `ConnRef::hasFixedRoute() const` | method | connector.h:390 (decl)<br>connector.cpp:635 | ported (comment) | connector.rs:673 `fn has_fixed_route`<br>connector.rs:1453 `fn has_fixed_route` |
| `ConnRef::clearFixedRoute()` | method | connector.h:397 (decl)<br>connector.cpp:640 | ported (comment) | connector.rs:679 `fn clear_fixed_route`<br>connector.rs:1462 `fn clear_fixed_route` |
| `ConnRef::set_route(const PolyLine &)` | method | connector.h:399 (decl)<br>connector.cpp:602 | ported (comment) | connector.rs:643 `fn set_route` |
| `ConnRef::calcRouteDist()` | method | connector.h:400 (decl)<br>connector.cpp:658 | ported (comment) | connector.rs:697 `fn calc_route_dist` |
| `ConnRef::makeActive()` | method | connector.h:401 (decl)<br>connector.cpp:550 | ported (comment) | connector.rs:590 `fn make_active` |
| `ConnRef::makeInactive()` | method | connector.h:402 (decl)<br>connector.cpp:573 | ported (comment) | connector.rs:609 `fn make_inactive` |
| `ConnRef::start()` | method | connector.h:403 (decl)<br>connector.cpp:741 | ported (comment) | connector.rs:778 `fn start` |
| `ConnRef::removeFromGraph()` | method | connector.h:404 (decl)<br>connector.cpp:761 | ported (comment) | connector.rs:798 `fn remove_from_graph` |
| `ConnRef::isInitialised() const` | method | connector.h:405 (decl)<br>connector.cpp:747 | ported (comment) | connector.rs:784 `fn is_initialised` |
| `ConnRef::makePathInvalid()` | method | connector.h:406 (decl)<br>connector.cpp:791 | ported (comment) | connector.rs:823 `fn make_path_invalid` |
| `ConnRef::setHateCrossings(bool)` | method | connector.h:407 (decl)<br>connector.cpp:1302 | ported (comment) | connector.rs:1188 `fn set_hate_crossings`<br>connector.rs:1471 `fn set_hate_crossings` |
| `ConnRef::doesHateCrossings() const` | method | connector.h:408 (decl)<br>connector.cpp:1308 | ported (comment) | connector.rs:1194 `fn does_hate_crossings`<br>connector.rs:1480 `fn does_hate_crossings` |
| `ConnRef::setEndpoint(const unsigned int, const ConnEnd &)` | method | connector.h:409 (decl)<br>connector.cpp:330 | ported (comment) | connector.rs:441 `fn set_endpoint`<br>connector.rs:1314 `fn set_endpoint` |
| `ConnRef::setEndpoint(const unsigned int, const VertID &, Point *)` | method | connector.h:410 (decl)<br>connector.cpp:519 | ported (comment) | connector.rs:554 `fn set_endpoint_vertex`<br>connector.rs:1498 `fn set_endpoint_vertex` |
| `ConnRef::possibleDstPinPoints() const` | method | connector.h:412 (decl)<br>connector.cpp:1314 | ported (comment) | connector.rs:1200 `fn possible_dst_pin_points`<br>connector.rs:1489 `fn possible_dst_pin_points` |
| `ConnRef::routeRef()` | method | connector.h:424 (decl)<br>connector.cpp:596 | ported (comment) | connector.rs:637 `fn route_ref` |
| `ConnRef::freeRoutes()` | method | connector.h:425 (decl)<br>connector.cpp:583 | ported (comment) | connector.rs:623 `fn free_routes` |
| `ConnRef::performCallback()` | method | connector.h:426 (decl)<br>connector.cpp:782 | ported (comment) | connector.rs:815 `fn perform_callback` |
| `ConnRef::generatePath()` | method | connector.h:427 (decl)<br>connector.cpp:923 | ported (comment) | connector.rs:858 `fn generate_path` |
| `ConnRef::generateCheckpointsPath(std::vector<Point> &, std::vector<VertInf *> &)` | method | connector.h:428 (decl)<br>connector.cpp:1059 | ported (comment) | connector.rs:971 `fn generate_checkpoints_path` |
| `ConnRef::generateStandardPath(std::vector<Point> &, std::vector<VertInf *> &)` | method | connector.h:430 (decl)<br>connector.cpp:1163 | ported (comment) | connector.rs:1075 `fn generate_standard_path` |
| `ConnRef::unInitialise()` | method | connector.h:432 (decl)<br>connector.cpp:753 | ported (comment) | connector.rs:790 `fn un_initialise` |
| `ConnRef::updateEndPoint(const unsigned int, const ConnEnd &)` | method | connector.h:433 (decl)<br>connector.cpp:393 | ported (comment) | connector.rs:498 `fn update_end_point` |
| `ConnRef::common_updateEndPoint(const unsigned int, ConnEnd)` | method | connector.h:434 (decl)<br>connector.cpp:228 | ported (comment) | connector.rs:348 `fn common_update_end_point` |
| `ConnRef::freeActivePins()` | method | connector.h:435 (decl)<br>connector.cpp:560 | ported (comment) | connector.rs:599 `fn free_active_pins` |
| `ConnRef::getConnEndForEndpointVertex(VertInf *, ConnEnd &) const` | method | connector.h:436 (decl)<br>connector.cpp:351 | ported (comment) | connector.rs:460 `fn get_conn_end_for_endpoint_vertex` |
| `ConnRef::endpointAnchors() const` | method | connector.h:438 (decl)<br>connector.cpp:494 | ported (comment) | connector.rs:532 `fn endpoint_anchors` |
| `ConnRef::outputCode(FILE *) const` | method | connector.h:439 (decl)<br>connector.cpp:429 | not-ported: debug-output | emits C++ reproduction code (noted in connector.rs) |
| `ConnRef::assignConnectionPinVisibility(const bool)` | method | connector.h:440 (decl)<br>connector.cpp:895 | ported (comment) | connector.rs:829 `fn assign_connection_pin_visibility` |
| `PtConnPtrPair` | typedef | connector.h:470 | ported (comment) | connector.rs:1573 `struct PtConnPtrPair` |
| `PointRepVector` | typedef | connector.h:472 | ported (comment) | connector.rs:1586 `type PointRepVector` |
| `NodeIndexPairLinkList` | typedef | connector.h:473 | ported (comment) | connector.rs:1588 `type NodeIndexPairLinkList` |
| `PtOrder` | class | connector.h:475 | ported (comment) | connector.rs:1593 `struct PtOrder` |
| `PtOrder::PtOrder()` | ctor | connector.h:478 (decl)<br>connector.cpp:1325 | ported (comment) | connector.rs:1604 `fn new` |
| `PtOrder::~PtOrder()` | dtor | connector.h:479 (decl)<br>connector.cpp:1335 | not-ported: subsumed | empty destructor |
| `PtOrder::addPoints(const size_t, const PtConnPtrPair &, const PtConnPtrPair &)` | method | connector.h:480 (decl)<br>connector.cpp:1388 | ported (comment) | connector.rs:1649 `fn add_points` |
| `PtOrder::addOrderedPoints(const size_t, const PtConnPtrPair &, const PtConnPtrPair &, bool)` | method | connector.h:482 (decl)<br>connector.cpp:1397 | ported (comment) | connector.rs:1656 `fn add_ordered_points` |
| `PtOrder::positionFor(const size_t, const ConnRef *)` | method | connector.h:484 (decl)<br>connector.cpp:1351 | ported (comment) | connector.rs:1620 `fn position_for` |
| `PtOrder::sortedPoints(const size_t)` | method | connector.h:485 (decl)<br>connector.cpp:1340 | ported (comment) | connector.rs:1610 `fn sorted_points` |
| `PtOrder::insertPoint(const size_t, const PtConnPtrPair &)` | method | connector.h:487 (decl)<br>connector.cpp:1372 | ported (comment) | connector.rs:1636 `fn insert_point` |
| `PtOrder::sort(const size_t)` | method | connector.h:488 (decl)<br>connector.cpp:1413 | ported (comment) | connector.rs:1676 `fn sort` |
| `PtOrderMap` | typedef | connector.h:497 | ported (comment) | connector.rs:1530 `type PtOrderMap` |
| `PointSet` | typedef | connector.h:498 | ported (comment) | connector.rs:1528 `type PointSet` |
| `CROSSING_NONE` | constant | connector.h:501 | ported (comment) | connector.rs:1515 `const CROSSING_NONE` |
| `CROSSING_TOUCHES` | constant | connector.h:502 | ported (comment) | connector.rs:1515 `const CROSSING_NONE` |
| `CROSSING_SHARES_PATH` | constant | connector.h:503 | ported (comment) | connector.rs:1515 `const CROSSING_NONE` |
| `CROSSING_SHARES_PATH_AT_END` | constant | connector.h:504 | ported (comment) | connector.rs:1515 `const CROSSING_NONE` |
| `CROSSING_SHARES_FIXED_SEGMENT` | constant | connector.h:505 | ported (comment) | connector.rs:1515 `const CROSSING_NONE` |
| `CrossingsInfoPair` | typedef | connector.h:508 | ported (comment) | connector.rs:1522 `type CrossingsInfoPair` |
| `PointList` | typedef | connector.h:509 | ported (comment) | connector.rs:1524 `type PointList` |
| `SharedPathList` | typedef | connector.h:510 | ported (comment) | connector.rs:1526 `type SharedPathList` |
| `ConnectorCrossings` | class | connector.h:512 | ported (comment) | connector.rs:1937 `struct ConnectorCrossings` |
| `ConnectorCrossings::ConnectorCrossings(Avoid::Polygon &, bool, Avoid::Polygon &, ConnRef *, ConnRef *)` | ctor | connector.h:515 (decl)<br>connector.cpp:1696 | ported (comment) | connector.rs:1957 `fn new` |
| `ConnectorCrossings::clear()` | method | connector.h:518 (decl)<br>connector.cpp:1710 | ported (comment) | connector.rs:1978 `fn clear` |
| `ConnectorCrossings::countForSegment(size_t, const bool)` | method | connector.h:519 (decl)<br>connector.cpp:1748 | ported (comment) | connector.rs:1997 `fn count_for_segment` |
| `splitBranchingSegments(Avoid::Polygon &, bool, Avoid::Polygon &, const double)` | function | connector.h:538 (decl)<br>connector.cpp:1572 | ported (comment) | connector.rs:1801 `fn split_branching_segments` |
| `validateBendPoint(VertInf *, VertInf *, VertInf *)` | function | connector.h:540 (decl)<br>connector.cpp:807 | ported (comment) | connector.rs:1213 `fn validate_bend_point` |
| `midpoint(Point, Point)` | function | connector.cpp:683 | ported (comment) | connector.rs:156 `fn midpoint` |
| `midVertexNumber(const Point &, const Point &, const Point &)` | function (static) | connector.cpp:1488 | ported (comment) | connector.rs:1735 `fn mid_vertex_number` |
| `segDir(const Point &, const Point &)` | function (static) | connector.cpp:1653 | ported (comment) | connector.rs:1868 `fn seg_dir` |
| `posInlineWithConnEndSegs(const double, const size_t, const Avoid::Polygon &, const Avoid::Polygon &)` | function (static) | connector.cpp:1674 | ported (comment) | connector.rs:1883 `fn pos_inline_with_conn_end_segs` |
| `pathLength(Avoid::Point **, Avoid::Point **, size_t)` | function (static) | connector.cpp:1721 | ported (comment) | connector.rs:1905 `fn path_length` |

<a id="connend"></a>
### connend

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ConnRefList` | typedef | connend.h:50 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `ConnDirFlag: ConnDirNone, ConnDirUp, ConnDirDown, ConnDirLeft, ConnDirRight, ConnDirAll` | enum | connend.h:62 | ported (comment) | connend.rs:48<br>vertices.rs:55 `const CONN_DIR_NONE` |
| `ConnDirFlags` | typedef | connend.h:83 | ported (comment) | connend.rs:48 |
| `ConnEndType: ConnEndPoint, ConnEndShapePin, ConnEndJunction, ConnEndEmpty` | enum | connend.h:89 | ported (comment) | connend.rs:55 `enum ConnEndType` |
| `ConnEnd` | class | connend.h:110 | ported (comment) | connend.rs:107 `struct ConnEnd` |
| `ConnEnd::ConnEnd(const Point &)` | ctor | connend.h:117 (decl)<br>connend.cpp:55 | ported (comment) | connend.rs:148 `fn from_point` |
| `ConnEnd::ConnEnd(const Point &, const ConnDirFlags)` | ctor | connend.h:130 (decl)<br>connend.cpp:67 | ported (comment) | connend.rs:164 `fn from_point_dirs` |
| `ConnEnd::ConnEnd(ShapeRef *, const unsigned int)` | ctor | connend.h:162 (decl)<br>connend.cpp:79 | ported (comment) | connend.rs:180 `fn from_shape_pin` |
| `ConnEnd::ConnEnd(JunctionRef *)` | ctor | connend.h:174 (decl)<br>connend.cpp:96 | ported (comment) | connend.rs:207 `fn from_junction` |
| `ConnEnd::type() const` | method | connend.h:181 (decl)<br>connend.cpp:114 | ported (comment) | connend.rs:227 `fn type_` |
| `ConnEnd::position() const` | method | connend.h:187 (decl)<br>connend.cpp:120 | ported (comment) | connend.rs:234 `fn position` |
| `ConnEnd::directions() const` | method | connend.h:194 (decl)<br>connend.cpp:137 | ported (comment) | connend.rs:271 `fn directions` |
| `ConnEnd::shape() const` | method | connend.h:202 (decl)<br>connend.cpp:150 | ported (comment) | connend.rs:282 `fn shape` |
| `ConnEnd::junction() const` | method | connend.h:211 (decl)<br>connend.cpp:156 | ported (comment) | connend.rs:292 `fn junction` |
| `ConnEnd::pinClassId() const` | method | connend.h:220 (decl)<br>connend.cpp:162 | ported (comment) | connend.rs:301 `fn pin_class_id` |
| `ConnEnd::ConnEnd()` | ctor | connend.h:222 (decl)<br>connend.cpp:44 | ported (comment) | connend.rs:133 `fn new` |
| `ConnEnd::~ConnEnd()` | dtor | connend.h:223 (decl)<br>connend.cpp:109 | not-ported: subsumed | empty destructor |
| `ConnEnd::connect(ConnRef *)` | method | connend.h:235 (decl)<br>connend.cpp:237 | ported (comment) | connend.rs:386 `fn connect` |
| `ConnEnd::disconnect(const bool)` | method | connend.h:236 (decl)<br>connend.cpp:249 | ported (comment) | connend.rs:401 `fn disconnect` |
| `ConnEnd::usePin(ShapeConnectionPin *)` | method | connend.h:237 (decl)<br>connend.cpp:182 | ported (comment) | connend.rs:330 `fn use_pin` |
| `ConnEnd::usePinVertex(VertInf *)` | method | connend.h:238 (decl)<br>connend.cpp:195 | ported (comment) | connend.rs:343 `fn use_pin_vertex` |
| `ConnEnd::freeActivePin()` | method | connend.h:239 (decl)<br>connend.cpp:226 | ported (comment) | connend.rs:376 `fn free_active_pin` |
| `ConnEnd::endpointType() const` | method | connend.h:240 (decl)<br>connend.cpp:174 | ported (comment) | connend.rs:315 `fn endpoint_type` |
| `ConnEnd::isPinConnection() const` | method | connend.h:241 (decl)<br>connend.cpp:169 | ported (comment) | connend.rs:307 `fn is_pin_connection` |
| `ConnEnd::possiblePinPoints() const` | method | connend.h:242 (decl)<br>connend.cpp:214 | ported (comment) | connend.rs:364 `fn possible_pin_points` |
| `ConnEnd::assignPinVisibilityTo(VertInf *, VertInf *)` | method | connend.h:243 (decl)<br>connend.cpp:274 | ported (comment) | connend.rs:429 `fn assign_pin_visibility_to` |
| `ConnEnd::outputCode(FILE *, const char *) const` | method | connend.h:245 (decl)<br>connend.cpp:411 | not-ported: debug-output | emits C++ reproduction code (noted in connend.rs) |
| `ConnEnd::getHyperedgeVertex(Router *) const` | method | connend.h:246 (decl)<br>connend.cpp:373 | ported (comment) | connend.rs:539 `fn get_hyperedge_vertex` |

<a id="debug"></a>
### debug

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `NOMINMAX` | define — conditional (#if defined(_MSC_VER) && defined(USE_ATLTRACE)) | debug.h:35 | not-ported: not-applicable | MSVC-only guard against windows.h min/max macros |
| `db_printf` | define — conditional (#ifdef LIBAVOID_DEBUG) | debug.h:51 | not-ported: debug-output | debug printf (no-op unless LIBAVOID_DEBUG); not ported (mod.rs) |
| `db_printf(const char *, ...)` | function (inline) | debug.h:53<br>debug.h:64 | not-ported: debug-output | debug printf (no-op unless LIBAVOID_DEBUG); not ported (mod.rs) |
| `err_printf` | define — conditional (#if defined(_MSC_VER) && defined(USE_ATLTRACE)) | debug.h:73 | not-ported: debug-output | stderr diagnostics macro; the warnings upstream prints through it are reproduced with eprint! at the call sites that matter (e.g. ConnEnd::assignPinVisibilityTo), others are dropped |
| `err_printf(const char *, ...)` | function (inline) | debug.h:78 | not-ported: debug-output | stderr diagnostics macro; the warnings upstream prints through it are reproduced with eprint! at the call sites that matter (e.g. ConnEnd::assignPinVisibilityTo), others are dropped |
| `PREC` | define | debug.h:93<br>debug.h:95 | not-ported: debug-output | printf precision for SVG debug output |

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
| `Point::Point(const double, const double)` | ctor | geomtypes.h:63 (decl)<br>geomtypes.cpp:48 | ported (comment) | geomtypes.rs:96 `fn new` |
| `Point::operator==(const Point &) const` | operator | geomtypes.h:70 (decl)<br>geomtypes.cpp:57 | ported (comment) | geomtypes.rs:133 `fn eq` |
| `Point::equals(const Point &, double) const` | method | geomtypes.h:79 (decl)<br>geomtypes.cpp:77 | ported (comment) | geomtypes.rs:107 `fn equals` |
| `Point::operator!=(const Point &) const` | operator | geomtypes.h:85 (decl)<br>geomtypes.cpp:67 | ported (comment) | geomtypes.rs:142 `fn ne` |
| `Point::operator<(const Point &) const` | operator | geomtypes.h:94 (decl)<br>geomtypes.cpp:88 | ported (comment) | geomtypes.rs:122 `fn less_than` |
| `Point::operator[](const size_t)` | operator | geomtypes.h:100 (decl)<br>geomtypes.cpp:98 | ported (comment) | geomtypes.rs:168 `fn index_mut` |
| `Point::operator[](const size_t) const` | operator | geomtypes.h:101 (decl)<br>geomtypes.cpp:105 | ported (comment) | geomtypes.rs:155 `fn index` |
| `Point::operator+(const Point &) const` | operator | geomtypes.h:103 (decl)<br>geomtypes.cpp:111 | ported (comment) | geomtypes.rs:182 `fn add` |
| `Point::operator-(const Point &) const` | operator | geomtypes.h:104 (decl)<br>geomtypes.cpp:117 | ported (comment) | geomtypes.rs:191 `fn sub` |
| `kUnassignedVertexNumber` | constant | geomtypes.h:120 | ported (comment) | geomtypes.rs:40 `const K_UNASSIGNED_VERTEX_NUMBER` |
| `kShapeConnectionPin` | constant | geomtypes.h:123 | ported (comment) | geomtypes.rs:44 `const K_SHAPE_CONNECTION_PIN` |
| `Vector` | typedef | geomtypes.h:128 | ported (comment) | geomtypes.rs:80 `type Vector` |
| `Box` | class | geomtypes.h:133 | ported (comment) | geomtypes.rs:228 `struct Box` |
| `Box::length(size_t) const` | method | geomtypes.h:141 (decl)<br>geomtypes.cpp:235 | ported (comment) | geomtypes.rs:236 `fn length` |
| `Box::width() const` | method | geomtypes.h:142 (decl)<br>geomtypes.cpp:244 | ported (comment) | geomtypes.rs:244 `fn width` |
| `Box::height() const` | method | geomtypes.h:143 (decl)<br>geomtypes.cpp:249 | ported (comment) | geomtypes.rs:249 `fn height` |
| `PolygonInterface` | class | geomtypes.h:150 | ported (comment) | geomtypes.rs:264 `trait PolygonInterface` |
| `PolygonInterface::PolygonInterface()` | ctor | geomtypes.h:154 | not-ported: subsumed | empty ctor of abstract base; PolygonInterface is a Rust trait |
| `PolygonInterface::~PolygonInterface()` | dtor (virtual) | geomtypes.h:156 | not-ported: subsumed | empty virtual dtor of abstract base; PolygonInterface is a Rust trait |
| `PolygonInterface::clear()` | method (virtual, pure) | geomtypes.h:158 (decl) | ported (name) | geomtypes.rs:267 |
| `PolygonInterface::empty() const` | method (virtual, pure) | geomtypes.h:160 (decl) | ported (name) | geomtypes.rs:269 |
| `PolygonInterface::size() const` | method (virtual, pure) | geomtypes.h:162 (decl) | ported (name) | geomtypes.rs:271 |
| `PolygonInterface::id() const` | method (virtual, pure) | geomtypes.h:164 (decl) | ported (name) | geomtypes.rs:273 |
| `PolygonInterface::at(size_t) const` | method (virtual, pure) | geomtypes.h:167 (decl) | ported (name) | geomtypes.rs:275 |
| `PolygonInterface::boundingRectPolygon() const` | method | geomtypes.h:171 (decl)<br>geomtypes.cpp:282 | ported (comment) | geomtypes.rs:277 `fn bounding_rect_polygon` |
| `PolygonInterface::offsetBoundingBox(double) const` | method | geomtypes.h:180 (decl)<br>geomtypes.cpp:210 | ported (comment) | geomtypes.rs:283 `fn offset_bounding_box` |
| `PolygonInterface::offsetPolygon(double) const` | method | geomtypes.h:182 (decl)<br>geomtypes.cpp:303 | ported (comment) | geomtypes.rs:307 `fn offset_polygon` |
| `Edge` | class | geomtypes.h:188 | ported (comment) | geomtypes.rs:256 `struct Edge` |
| `Polygon` | class | geomtypes.h:207 | ported (comment) | geomtypes.rs:391 `struct Polygon` |
| `Polygon::Polygon()` | ctor | geomtypes.h:211 (decl)<br>geomtypes.cpp:254 | ported (comment) | geomtypes.rs:559 `fn new` |
| `Polygon::Polygon(const int)` | ctor | geomtypes.h:226 (decl)<br>geomtypes.cpp:262 | ported (comment) | geomtypes.rs:565 `fn with_size` |
| `Polygon::Polygon(const PolygonInterface &)` | ctor | geomtypes.h:231 (decl)<br>geomtypes.cpp:270 | ported (comment) | geomtypes.rs:577 `fn from_interface` |
| `Polygon::clear()` | method | geomtypes.h:233 (decl)<br>geomtypes.cpp:365 | ported (comment) | geomtypes.rs:412 `fn clear` |
| `Polygon::empty() const` | method | geomtypes.h:235 (decl)<br>geomtypes.cpp:372 | ported (comment) | geomtypes.rs:418 `fn empty` |
| `Polygon::size() const` | method | geomtypes.h:237 (decl)<br>geomtypes.cpp:378 | ported (comment) | geomtypes.rs:423 `fn size` |
| `Polygon::id() const` | method | geomtypes.h:239 (decl)<br>geomtypes.cpp:384 | ported (comment) | geomtypes.rs:429 `fn id` |
| `Polygon::at(size_t) const` | method | geomtypes.h:242 (decl)<br>geomtypes.cpp:390 | ported (comment) | geomtypes.rs:434 `fn at` |
| `Polygon::setPoint(size_t, const Point &)` | method | geomtypes.h:246 (decl)<br>geomtypes.cpp:397 | ported (comment) | geomtypes.rs:591 `fn set_point` |
| `Polygon::simplify() const` | method | geomtypes.h:253 (decl)<br>geomtypes.cpp:532 | ported (comment) | geomtypes.rs:605 `fn simplify` |
| `Polygon::curvedPolyline(const double, const bool) const` | method | geomtypes.h:274 (decl)<br>geomtypes.cpp:640 | ported (comment) | geomtypes.rs:676 `fn curved_polyline` |
| `Polygon::translate(const double, const double)` | method | geomtypes.h:280 (decl)<br>geomtypes.cpp:522 | ported (comment) | geomtypes.rs:597 `fn translate` |
| `Polygon::checkpointsOnSegment(size_t, int) const` | method | geomtypes.h:321 (decl)<br>geomtypes.cpp:594 | ported (comment) | geomtypes.rs:646 `fn checkpoints_on_segment` |
| `PolyLine` | typedef | geomtypes.h:328 | ported (comment) | geomtypes.rs:408 `type PolyLine` |
| `ReferencingPolygon` | class | geomtypes.h:336 | ported (comment) | geomtypes.rs:810 `struct ReferencingPolygon` |
| `ReferencingPolygon::ReferencingPolygon()` | ctor | geomtypes.h:339 (decl)<br>geomtypes.cpp:159 | ported (comment) | geomtypes.rs:819 `fn new` |
| `ReferencingPolygon::ReferencingPolygon(const Polygon &, const Router *)` | ctor | geomtypes.h:340 (decl)<br>geomtypes.cpp:123 | ported (comment) | geomtypes.rs:827 `fn from_polygon` |
| `ReferencingPolygon::clear()` | method | geomtypes.h:341 (decl)<br>geomtypes.cpp:166 | ported (comment) | geomtypes.rs:858 `fn clear` |
| `ReferencingPolygon::empty() const` | method | geomtypes.h:342 (decl)<br>geomtypes.cpp:173 | ported (comment) | geomtypes.rs:864 `fn empty` |
| `ReferencingPolygon::size() const` | method | geomtypes.h:343 (decl)<br>geomtypes.cpp:179 | ported (comment) | geomtypes.rs:869 `fn size` |
| `ReferencingPolygon::id() const` | method | geomtypes.h:344 (decl)<br>geomtypes.cpp:185 | ported (comment) | geomtypes.rs:874 `fn id` |
| `ReferencingPolygon::at(size_t) const` | method | geomtypes.h:345 (decl)<br>geomtypes.cpp:191 | ported (comment) | geomtypes.rs:879 `fn at` |
| `Rectangle` | class | geomtypes.h:356 | ported (comment) | geomtypes.rs:758 `struct Rectangle` |
| `Rectangle::Rectangle(const Point &, const Point &)` | ctor | geomtypes.h:365 (decl)<br>geomtypes.cpp:727 | ported (comment) | geomtypes.rs:763 `fn new` |
| `Rectangle::Rectangle(const Point &, const double, const double)` | ctor | geomtypes.h:375 (decl)<br>geomtypes.cpp:742 | ported (comment) | geomtypes.rs:780 `fn from_centre` |
| `unitNormalForEdge(const Point &, const Point &)` | function (static) | geomtypes.cpp:289 | ported (comment) | geomtypes.rs:367 `fn unit_normal_for_edge` |
| `SHORTEN_NONE` | constant | geomtypes.cpp:405 | ported (comment) | geomtypes.rs:442 `const SHORTEN_NONE` |
| `SHORTEN_START` | constant | geomtypes.cpp:406 | ported (comment) | geomtypes.rs:442 `const SHORTEN_NONE` |
| `SHORTEN_END` | constant | geomtypes.cpp:407 | ported (comment) | geomtypes.rs:442 `const SHORTEN_NONE` |
| `SHORTEN_BOTH` | constant | geomtypes.cpp:408 | ported (comment) | geomtypes.rs:442 `const SHORTEN_NONE` |
| `shorten_line(double &, double &, double &, double &, const unsigned int, const double)` | function (static) | geomtypes.cpp:415 | ported (comment) | geomtypes.rs:448 `fn shorten_line` |
| `mid(a, b)` | macro | geomtypes.cpp:628 | ported (comment) | geomtypes.rs:547 `fn mid` |

<a id="graph"></a>
### graph

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShapeList` | typedef | graph.h:42 | ported (comment) | graph.rs:36 `type ShapeList` |
| `FlagList` | typedef | graph.h:43 | ported (comment) | graph.rs:38 `type FlagList` |
| `EdgeInf` | class | graph.h:46 | ported (comment) | graph.rs:42 `struct EdgeInf` |
| `EdgeInf::EdgeInf(VertInf *, VertInf *, const bool)` | ctor | graph.h:49 (decl)<br>graph.cpp:43 | ported (comment) | graph.rs:113 `fn new` |
| `EdgeInf::~EdgeInf()` | dtor | graph.h:50 (decl)<br>graph.cpp:68 | ported (comment) | graph.rs:138 `fn delete` |
| `EdgeInf::getDist()` | method (inline) | graph.h:51 | ported (comment) | graph.rs:146 `fn get_dist` |
| `EdgeInf::setDist(double)` | method | graph.h:55 (decl)<br>graph.cpp:259 | ported (comment) | graph.rs:297 `fn set_dist` |
| `EdgeInf::alertConns()` | method | graph.h:56 (decl)<br>graph.cpp:318 | ported (comment) | graph.rs:360 `fn alert_conns` |
| `EdgeInf::addConn(bool *)` | method | graph.h:57 (decl)<br>graph.cpp:329 | ported (comment) | graph.rs:374 `fn add_conn` |
| `EdgeInf::addCycleBlocker()` | method | graph.h:58 (decl)<br>graph.cpp:335 | ported (comment) | graph.rs:380 `fn add_cycle_blocker` |
| `EdgeInf::addBlocker(int)` | method | graph.h:59 (decl)<br>graph.cpp:342 | ported (comment) | graph.rs:386 `fn add_blocker` |
| `EdgeInf::added()` | method | graph.h:60 (decl)<br>graph.cpp:308 | ported (comment) | graph.rs:348 `fn added` |
| `EdgeInf::isOrthogonal() const` | method | graph.h:61 (decl)<br>graph.cpp:562 | ported (comment) | graph.rs:567 `fn is_orthogonal` |
| `EdgeInf::isDummyConnection() const` | method | graph.h:62 (decl)<br>graph.cpp:569 | ported (comment) | graph.rs:575 `fn is_dummy_connection` |
| `EdgeInf::isDisabled() const` | method | graph.h:63 (decl)<br>graph.cpp:293 | ported (comment) | graph.rs:330 `fn is_disabled` |
| `EdgeInf::setDisabled(const bool)` | method | graph.h:64 (decl)<br>graph.cpp:298 | ported (comment) | graph.rs:336 `fn set_disabled` |
| `EdgeInf::rotationLessThan(const VertInf *, const EdgeInf *) const` | method | graph.h:65 (decl)<br>graph.cpp:135 | ported (comment) | graph.rs:156 `fn rotation_less_than` |
| `EdgeInf::ids() const` | method | graph.h:66 (decl)<br>graph.cpp:361 | ported (comment) | graph.rs:403 `fn ids` |
| `EdgeInf::points() const` | method | graph.h:67 (decl)<br>graph.cpp:367 | ported (comment) | graph.rs:408 `fn points` |
| `EdgeInf::db_print()` | method | graph.h:68 (decl)<br>graph.cpp:373 | not-ported: debug-output | db_printf debug dump |
| `EdgeInf::checkVis()` | method | graph.h:69 (decl)<br>graph.cpp:383 | ported (comment) | graph.rs:413 `fn check_vis` |
| `EdgeInf::otherVert(const VertInf *) const` | method | graph.h:70 (decl)<br>graph.cpp:578 | ported (comment) | graph.rs:584 `fn other_vert` |
| `EdgeInf::checkEdgeVisibility(VertInf *, VertInf *, bool)` | method (static member) | graph.h:71 (decl)<br>graph.cpp:586 | ported (comment) | graph.rs:597 `fn check_edge_visibility` |
| `EdgeInf::existingEdge(VertInf *, VertInf *)` | method (static member) | graph.h:73 (decl)<br>graph.cpp:622 | ported (comment) | graph.rs:627 `fn existing_edge` |
| `EdgeInf::blocker() const` | method | graph.h:74 (decl)<br>graph.cpp:313 | ported (comment) | graph.rs:354 `fn blocker` |
| `EdgeInf::isHyperedgeSegment() const` | method | graph.h:76 (decl)<br>graph.cpp:288 | ported (comment) | graph.rs:324 `fn is_hyperedge_segment` |
| `EdgeInf::setHyperedgeSegment(const bool)` | method | graph.h:77 (decl)<br>graph.cpp:303 | ported (comment) | graph.rs:342 `fn set_hyperedge_segment` |
| `EdgeInf::mtstDist() const` | method | graph.h:78 (decl)<br>graph.cpp:283 | ported (comment) | graph.rs:318 `fn mtst_dist` |
| `EdgeInf::setMtstDist(const double)` | method | graph.h:79 (decl)<br>graph.cpp:278 | ported (comment) | graph.rs:312 `fn set_mtst_dist` |
| `EdgeInf::makeActive()` | method | graph.h:87 (decl)<br>graph.cpp:185 | ported (comment) | graph.rs:208 `fn make_active` |
| `EdgeInf::makeInactive()` | method | graph.h:88 (decl)<br>graph.cpp:221 | ported (comment) | graph.rs:251 `fn make_inactive` |
| `EdgeInf::firstBlocker()` | method | graph.h:89 (decl)<br>graph.cpp:486 | ported (comment) | graph.rs:490 `fn first_blocker` |
| `EdgeInf::isBetween(VertInf *, VertInf *)` | method | graph.h:90 (decl)<br>graph.cpp:550 | ported (comment) | graph.rs:555 `fn is_between` |
| `EdgeList` | class | graph.h:109 | ported (comment) | graph.rs:692 `struct EdgeList` |
| `EdgeList::EdgeList(bool)` | ctor | graph.h:113 (decl)<br>graph.cpp:672 | ported (comment) | graph.rs:702 `fn new` |
| `EdgeList::~EdgeList()` | dtor | graph.h:114 (decl)<br>graph.cpp:681 | not-ported: subsumed | only runs at Router destruction (deletes remaining edges via clear()); arenas are dropped with the Router |
| `EdgeList::clear()` | method | graph.h:115 (decl)<br>graph.cpp:687 | ported (comment) | graph.rs:714 `fn clear` |
| `EdgeList::begin()` | method | graph.h:116 (decl)<br>graph.cpp:771 | ported (comment) | graph.rs:792 `fn begin` |
| `EdgeList::end()` | method | graph.h:117 (decl)<br>graph.cpp:777 | ported (comment) | graph.rs:798 `fn end` |
| `EdgeList::size() const` | method | graph.h:118 (decl)<br>graph.cpp:701 | ported (comment) | graph.rs:727 `fn size` |
| `EdgeList::addEdge(EdgeInf *)` | method | graph.h:120 (decl)<br>graph.cpp:707 | ported (comment) | graph.rs:733 `fn add_edge` |
| `EdgeList::removeEdge(EdgeInf *)` | method | graph.h:121 (decl)<br>graph.cpp:740 | ported (comment) | graph.rs:767 `fn remove_edge` |
| `orthogTurnOrder(const Point &, const Point &, const Point &)` | function (static, inline) | graph.cpp:85 | ported (comment) | graph.rs:69 `fn orthog_turn_order` |

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
| `HyperedgeRerouter::HyperedgeRerouter()` | ctor | hyperedge.h:138 (decl)<br>hyperedge.cpp:41 | ported (comment) | hyperedge.rs:72 `fn new` |
| `HyperedgeRerouter::registerHyperedgeForRerouting(ConnEndList)` | method | hyperedge.h:148 (decl)<br>hyperedge.cpp:51 | ported (comment) | hyperedge.rs:77 `fn register_hyperedge_for_rerouting` |
| `HyperedgeRerouter::registerHyperedgeForRerouting(JunctionRef *)` | method | hyperedge.h:164 (decl)<br>hyperedge.cpp:60 | ported (comment) | hyperedge.rs:85 `fn register_hyperedge_for_rerouting_junction` |
| `HyperedgeRerouter::newAndDeletedObjectLists(size_t) const` | method | hyperedge.h:182 (decl)<br>hyperedge.cpp:74 | ported (comment) | hyperedge.rs:98 `fn new_and_deleted_object_lists` |
| `HyperedgeRerouter::count() const` | method | hyperedge.h:189 (decl)<br>hyperedge.cpp:69 | ported (comment) | hyperedge.rs:93 `fn count` |
| `HyperedgeRerouter::setRouter(Router *)` | method | hyperedge.h:198 (decl)<br>hyperedge.cpp:46 | not-ported: subsumed | stores m_router; the port passes &mut Router to every HyperedgeRerouter method |
| `HyperedgeRerouter::calcHyperedgeConnectors()` | method | hyperedge.h:200 (decl)<br>hyperedge.cpp:212 | ported (comment) | hyperedge.rs:212 `fn calc_hyperedge_connectors` |
| `HyperedgeRerouter::performRerouting()` | method | hyperedge.h:202 (decl)<br>hyperedge.cpp:277 | ported (comment) | hyperedge.rs:278 `fn perform_rerouting` |
| `HyperedgeRerouter::outputInstanceToSVG(FILE *)` | method | hyperedge.h:203 (decl)<br>hyperedge.cpp:90 | not-ported: debug-output | SVG debug output |
| `HyperedgeRerouter::findAttachedObjects(size_t, ConnRef *, JunctionRef *, ConnRefSet &)` | method | hyperedge.h:204 (decl)<br>hyperedge.cpp:128 | ported (comment) | hyperedge.rs:115 `fn find_attached_objects_conn` |
| `HyperedgeRerouter::findAttachedObjects(size_t, JunctionRef *, ConnRef *, ConnRefSet &)` | method | hyperedge.h:206 (decl)<br>hyperedge.cpp:178 | ported (comment) | hyperedge.rs:176 `fn find_attached_objects_junction` |

<a id="hyperedgeimprover"></a>
### hyperedgeimprover

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShiftSegmentList` | typedef | hyperedgeimprover.h:43 | ported (comment) | hyperedgeimprover.rs:75 `type ShiftSegmentList` |
| `RootSegmentsMap` | typedef | hyperedgeimprover.h:44 | ported (comment) | hyperedgeimprover.rs:78 `type RootSegmentsMap` |
| `JunctionHyperedgeTreeNodeMap` | typedef | hyperedgeimprover.h:45 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `JunctionSet` | typedef | hyperedgeimprover.h:47 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `ConnRefList` | typedef | hyperedgeimprover.h:48 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `JunctionRefList` | typedef | hyperedgeimprover.h:49 | not-ported: subsumed | duplicate of the hyperedgetree.h typedef; Rust alias in hyperedgetree.rs |
| `HyperedgeImprover` | class | hyperedgeimprover.h:51 | ported (comment) | hyperedgeimprover.rs:302 `struct HyperedgeImprover` |
| `HyperedgeImprover::HyperedgeImprover()` | ctor | hyperedgeimprover.h:55 (decl)<br>hyperedgeimprover.cpp:267 | ported (comment) | hyperedgeimprover.rs:326 `fn new` |
| `HyperedgeImprover::clear()` | method | hyperedgeimprover.h:57 (decl)<br>hyperedgeimprover.cpp:278 | ported (comment) | hyperedgeimprover.rs:333 `fn clear` |
| `HyperedgeImprover::setRouter(Router *)` | method | hyperedgeimprover.h:60 (decl)<br>hyperedgeimprover.cpp:273 | not-ported: subsumed | stores m_router; the port passes &mut Router to every HyperedgeImprover method |
| `HyperedgeImprover::newAndDeletedObjectLists() const` | method | hyperedgeimprover.h:64 (decl)<br>hyperedgeimprover.cpp:1027 | ported (comment) | hyperedgeimprover.rs:350 `fn new_and_deleted_object_lists` |
| `HyperedgeImprover::execute(bool)` | method | hyperedgeimprover.h:67 (decl)<br>hyperedgeimprover.cpp:787 | ported (comment) | hyperedgeimprover.rs:804 `fn execute`<br>hyperedgeimprover.rs:812 `fn execute_impl` |
| `HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeNode *, const size_t, HyperedgeTreeEdge *, ShiftSegmentList &)` | method | hyperedgeimprover.h:71 (decl)<br>hyperedgeimprover.cpp:293 | ported (comment) | hyperedgeimprover.rs:406 `fn create_shift_segments_for_dimension_excluding_node` |
| `HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeEdge *, const size_t, HyperedgeTreeNode *, ShiftSegmentList &)` | method | hyperedgeimprover.h:76 (decl)<br>hyperedgeimprover.cpp:310 | ported (comment) | hyperedgeimprover.rs:425 `fn create_shift_segments_for_dimension_excluding_edge` |
| `HyperedgeImprover::mergeOverlappingSegments(ShiftSegmentList &)` | method | hyperedgeimprover.h:84 (decl)<br>hyperedgeimprover.cpp:342 | ported (comment) | hyperedgeimprover.rs:500 `fn merge_overlapping_segments` |
| `HyperedgeImprover::buildHyperedgeSegments(const size_t)` | method | hyperedgeimprover.h:90 (decl)<br>hyperedgeimprover.cpp:376 | ported (comment) | hyperedgeimprover.rs:531 `fn build_hyperedge_segments` |
| `HyperedgeImprover::removeZeroLengthEdges()` | method | hyperedgeimprover.h:96 (decl)<br>hyperedgeimprover.cpp:398 | ported (comment) | hyperedgeimprover.rs:555 `fn remove_zero_length_edges_all` |
| `HyperedgeImprover::moveJunctionsAlongCommonEdges()` | method | hyperedgeimprover.h:102 (decl)<br>hyperedgeimprover.cpp:413 | ported (comment) | hyperedgeimprover.rs:571 `fn move_junctions_along_common_edges` |
| `HyperedgeImprover::nudgeHyperedgeSegments(size_t, unsigned int &)` | method | hyperedgeimprover.h:108 (decl)<br>hyperedgeimprover.cpp:555 | ported (comment) | hyperedgeimprover.rs:692 `fn nudge_hyperedge_segments` |
| `HyperedgeImprover::writeHyperedgeSegmentsBackToConnPaths()` | method | hyperedgeimprover.h:112 (decl)<br>hyperedgeimprover.cpp:619 | ported (comment) | hyperedgeimprover.rs:748 `fn write_hyperedge_segments_back_to_conn_paths` |
| `HyperedgeImprover::outputHyperedgesToSVG(unsigned int, HyperedgeShiftSegment *)` | method | hyperedgeimprover.h:116 (decl)<br>hyperedgeimprover.cpp:637 | not-ported: debug-output | SVG debug output (MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG only) |
| `HyperedgeImprover::getEndpoints(JunctionRef *, JunctionRef *, std::set<VertInf *> &)` | method | hyperedgeimprover.h:122 (decl)<br>hyperedgeimprover.cpp:743 | ported (comment) | hyperedgeimprover.rs:765 `fn get_endpoints` |
| `HyperedgeImprover::moveJunctionAlongCommonEdge(HyperedgeTreeNode *, bool &)` | method | hyperedgeimprover.h:129 (decl)<br>hyperedgeimprover.cpp:1046 | ported (comment) | hyperedgeimprover.rs:1018 `fn move_junction_along_common_edge` |
| `HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeNode *, HyperedgeTreeEdge *)` | method | hyperedgeimprover.h:134 (decl)<br>hyperedgeimprover.cpp:465 | ported (comment) | hyperedgeimprover.rs:621 `fn remove_zero_length_edges_node` |
| `HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeEdge *, HyperedgeTreeNode *)` | method | hyperedgeimprover.h:139 (decl)<br>hyperedgeimprover.cpp:448 | ported (comment) | hyperedgeimprover.rs:607 `fn remove_zero_length_edges_edge` |
| `HyperedgeShiftSegment` | class | hyperedgeimprover.cpp:41 | ported (comment) | hyperedgeimprover.rs:81 `struct HyperedgeShiftSegment` |
| `HyperedgeShiftSegment::HyperedgeShiftSegment(HyperedgeTreeNode *, HyperedgeTreeNode *, const size_t, bool)` | ctor | hyperedgeimprover.cpp:44 | ported (comment) | hyperedgeimprover.rs:102 `fn new` |
| `HyperedgeShiftSegment::~HyperedgeShiftSegment()` | dtor (virtual) | hyperedgeimprover.cpp:61 | ported (comment) | hyperedgeimprover.rs:134 `fn destruct` |
| `HyperedgeShiftSegment::lowPoint()` | method | hyperedgeimprover.cpp:70 | ported (comment) | hyperedgeimprover.rs:147 `fn low_point_node`<br>hyperedgeimprover.rs:284 `fn low_point` |
| `HyperedgeShiftSegment::highPoint()` | method | hyperedgeimprover.cpp:74 | ported (comment) | hyperedgeimprover.rs:152 `fn high_point_node`<br>hyperedgeimprover.rs:288 `fn high_point` |
| `HyperedgeShiftSegment::lowPoint() const` | method | hyperedgeimprover.cpp:78 | ported (name) | hyperedgeimprover.rs:285 |
| `HyperedgeShiftSegment::highPoint() const` | method | hyperedgeimprover.cpp:82 | ported (name) | hyperedgeimprover.rs:289 |
| `HyperedgeShiftSegment::setBalanceCount()` | method | hyperedgeimprover.cpp:90 | ported (comment) | hyperedgeimprover.rs:161 `fn set_balance_count` |
| `HyperedgeShiftSegment::balanceCount() const` | method | hyperedgeimprover.cpp:130 | ported (comment) | hyperedgeimprover.rs:191 `fn balance_count` |
| `HyperedgeShiftSegment::adjustPosition()` | method | hyperedgeimprover.cpp:135 | ported (comment) | hyperedgeimprover.rs:197 `fn adjust_position` |
| `HyperedgeShiftSegment::overlapsWith(const ShiftSegment *, const size_t) const` | method | hyperedgeimprover.cpp:185 | ported (comment) | hyperedgeimprover.rs:253 `fn overlaps_with_impl`<br>hyperedgeimprover.rs:292 `fn overlaps_with` |
| `HyperedgeShiftSegment::immovable() const` | method | hyperedgeimprover.cpp:204 | ported (comment) | hyperedgeimprover.rs:296 `fn immovable` |
| `HyperedgeShiftSegment::settled() const` | method | hyperedgeimprover.cpp:208 | ported (comment) | hyperedgeimprover.rs:271 `fn settled` |
| `HyperedgeShiftSegment::mergesWith(HyperedgeShiftSegment *)` | method | hyperedgeimprover.cpp:212 | ported (comment) | hyperedgeimprover.rs:454 `fn merges_with` |
| `CmpHyperedgeSegmentDirOrder(const ShiftSegment *, const ShiftSegment *)` | function (static) — dead (#if 0) | hyperedgeimprover.cpp:253 | not-ported: dead-code | inside `#if 0` ("UNUSED") |

<a id="hyperedgetree"></a>
### hyperedgetree

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `JunctionHyperedgeTreeNodeMap` | typedef | hyperedgetree.h:54 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap` |
| `JunctionSet` | typedef | hyperedgetree.h:56 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap` |
| `JunctionRefList` | typedef | hyperedgetree.h:57 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap` |
| `ConnRefList` | typedef | hyperedgetree.h:58 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap` |
| `OrderedHENodeSet` | typedef | hyperedgetree.h:62 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap`<br>hyperedgetree.rs:168 `struct OrderedHENodeSet` |
| `HyperedgeTreeNode` | struct | hyperedgetree.h:64 | ported (comment) | hyperedgetree.rs:95 `struct HyperedgeTreeNode` |
| `HyperedgeTreeNode::HyperedgeTreeNode()` | ctor | hyperedgetree.h:66 (decl)<br>hyperedgetree.cpp:41 | ported (comment) | hyperedgetree.rs:302 `fn new_node` |
| `HyperedgeTreeNode::~HyperedgeTreeNode()` | dtor | hyperedgetree.h:67 (decl)<br>hyperedgetree.cpp:51 | ported (comment) | hyperedgetree.rs:321 `fn delete_node` |
| `HyperedgeTreeNode::deleteEdgesExcept(HyperedgeTreeEdge *)` | method | hyperedgetree.h:69 (decl)<br>hyperedgetree.cpp:325 | ported (comment) | hyperedgetree.rs:545 `fn node_delete_edges_except` |
| `HyperedgeTreeNode::removeOtherJunctionsFrom(HyperedgeTreeEdge *, JunctionSet &)` | method | hyperedgetree.h:70 (decl)<br>hyperedgetree.cpp:85 | ported (comment) | hyperedgetree.rs:360 `fn node_remove_other_junctions_from` |
| `HyperedgeTreeNode::outputEdgesExcept(FILE *, HyperedgeTreeEdge *)` | method | hyperedgetree.h:72 (decl)<br>hyperedgetree.cpp:64 | not-ported: debug-output | SVG debug output |
| `HyperedgeTreeNode::disconnectEdge(HyperedgeTreeEdge *)` | method | hyperedgetree.h:73 (decl)<br>hyperedgetree.cpp:342 | ported (comment) | hyperedgetree.rs:559 `fn node_disconnect_edge` |
| `HyperedgeTreeNode::spliceEdgesFrom(HyperedgeTreeNode *)` | method | hyperedgetree.h:74 (decl)<br>hyperedgetree.cpp:363 | ported (comment) | hyperedgetree.rs:566 `fn node_splice_edges_from` |
| `HyperedgeTreeNode::writeEdgesToConns(HyperedgeTreeEdge *, size_t)` | method | hyperedgetree.h:75 (decl)<br>hyperedgetree.cpp:118 | ported (comment) | hyperedgetree.rs:395 `fn node_write_edges_to_conns` |
| `HyperedgeTreeNode::addConns(HyperedgeTreeEdge *, Router *, ConnRefList &, ConnRef *)` | method | hyperedgetree.h:76 (decl)<br>hyperedgetree.cpp:135 | ported (comment) | hyperedgetree.rs:415 `fn node_add_conns` |
| `HyperedgeTreeNode::updateConnEnds(HyperedgeTreeEdge *, bool, ConnRefList &)` | method | hyperedgetree.h:78 (decl)<br>hyperedgetree.cpp:199 | ported (comment) | hyperedgetree.rs:458 `fn node_update_conn_ends` |
| `HyperedgeTreeNode::listJunctionsAndConnectors(HyperedgeTreeEdge *, JunctionRefList &, ConnRefList &)` | method | hyperedgetree.h:80 (decl)<br>hyperedgetree.cpp:244 | ported (comment) | hyperedgetree.rs:496 `fn node_list_junctions_and_connectors` |
| `HyperedgeTreeNode::isImmovable() const` | method | hyperedgetree.h:82 (decl)<br>hyperedgetree.cpp:374 | ported (comment) | hyperedgetree.rs:574 `fn node_is_immovable` |
| `HyperedgeTreeNode::validateHyperedge(const HyperedgeTreeEdge *, const size_t) const` | method | hyperedgetree.h:83 (decl)<br>hyperedgetree.cpp:263 | ported (comment) | hyperedgetree.rs:517 `fn node_validate_hyperedge` |
| `HyperedgeTreeEdge` | struct | hyperedgetree.h:96 | ported (comment) | hyperedgetree.rs:112 `struct HyperedgeTreeEdge` |
| `HyperedgeTreeEdge::HyperedgeTreeEdge(HyperedgeTreeNode *, HyperedgeTreeNode *, ConnRef *)` | ctor | hyperedgetree.h:98 (decl)<br>hyperedgetree.cpp:393 | ported (comment) | hyperedgetree.rs:593 `fn new_edge` |
| `HyperedgeTreeEdge::followFrom(HyperedgeTreeNode *) const` | method | hyperedgetree.h:101 (decl)<br>hyperedgetree.cpp:410 | ported (comment) | hyperedgetree.rs:624 `fn edge_follow_from` |
| `HyperedgeTreeEdge::zeroLength() const` | method | hyperedgetree.h:102 (decl)<br>hyperedgetree.cpp:419 | ported (comment) | hyperedgetree.rs:633 `fn edge_zero_length` |
| `HyperedgeTreeEdge::splitFromNodeAtPoint(HyperedgeTreeNode *, const Point &)` | method | hyperedgetree.h:103 (decl)<br>hyperedgetree.cpp:719 | ported (comment) | hyperedgetree.rs:870 `fn edge_split_from_node_at_point` |
| `HyperedgeTreeEdge::hasOrientation(const size_t) const` | method | hyperedgetree.h:104 (decl)<br>hyperedgetree.cpp:450 | ported (comment) | hyperedgetree.rs:641 `fn edge_has_orientation` |
| `HyperedgeTreeEdge::outputNodesExcept(FILE *, HyperedgeTreeNode *)` | method | hyperedgetree.h:105 (decl)<br>hyperedgetree.cpp:428 | not-ported: debug-output | SVG debug output |
| `HyperedgeTreeEdge::deleteNodesExcept(HyperedgeTreeNode *)` | method | hyperedgetree.h:106 (decl)<br>hyperedgetree.cpp:784 | ported (comment) | hyperedgetree.rs:940 `fn edge_delete_nodes_except` |
| `HyperedgeTreeEdge::removeOtherJunctionsFrom(HyperedgeTreeNode *, JunctionSet &)` | method | hyperedgetree.h:107 (decl)<br>hyperedgetree.cpp:764 | ported (comment) | hyperedgetree.rs:917 `fn edge_remove_other_junctions_from` |
| `HyperedgeTreeEdge::writeEdgesToConns(HyperedgeTreeNode *, size_t)` | method | hyperedgetree.h:109 (decl)<br>hyperedgetree.cpp:480 | ported (comment) | hyperedgetree.rs:664 `fn edge_write_edges_to_conns` |
| `HyperedgeTreeEdge::addConns(HyperedgeTreeNode *, Router *, ConnRefList &)` | method | hyperedgetree.h:110 (decl)<br>hyperedgetree.cpp:570 | ported (comment) | hyperedgetree.rs:737 `fn edge_add_conns` |
| `HyperedgeTreeEdge::updateConnEnds(HyperedgeTreeNode *, bool, ConnRefList &)` | method | hyperedgetree.h:112 (decl)<br>hyperedgetree.cpp:622 | ported (comment) | hyperedgetree.rs:788 `fn edge_update_conn_ends` |
| `HyperedgeTreeEdge::disconnectEdge()` | method | hyperedgetree.h:114 (decl)<br>hyperedgetree.cpp:749 | ported (comment) | hyperedgetree.rs:904 `fn edge_disconnect_edge` |
| `HyperedgeTreeEdge::replaceNode(HyperedgeTreeNode *, HyperedgeTreeNode *)` | method | hyperedgetree.h:115 (decl)<br>hyperedgetree.cpp:459 | ported (comment) | hyperedgetree.rs:649 `fn edge_replace_node` |
| `HyperedgeTreeEdge::listJunctionsAndConnectors(HyperedgeTreeNode *, JunctionRefList &, ConnRefList &)` | method | hyperedgetree.h:117 (decl)<br>hyperedgetree.cpp:671 | ported (comment) | hyperedgetree.rs:834 `fn edge_list_junctions_and_connectors` |
| `HyperedgeTreeEdge::validateHyperedge(const HyperedgeTreeNode *, const size_t) const` | method | hyperedgetree.h:119 (decl)<br>hyperedgetree.cpp:693 | ported (comment) | hyperedgetree.rs:856 `fn edge_validate_hyperedge` |
| `VertexNodeMap` | typedef | hyperedgetree.h:128 | ported (comment) | hyperedgetree.rs:84 `type JunctionHyperedgeTreeNodeMap` |
| `CmpNodesInDim` | class | hyperedgetree.h:131 | not-ported: subsumed | comparator class; its ctor and operator() are ported (hyperedgetree.rs) |
| `CmpNodesInDim::CmpNodesInDim(const size_t)` | ctor | hyperedgetree.h:134 (decl)<br>hyperedgetree.cpp:802 | ported (comment) | hyperedgetree.rs:227 `fn new_node_set` |
| `CmpNodesInDim::operator()(const HyperedgeTreeNode *, const HyperedgeTreeNode *) const` | operator | hyperedgetree.h:135 (decl)<br>hyperedgetree.cpp:810 | ported (comment) | hyperedgetree.rs:131 `fn cmp_nodes_in_dim` |
| `travellingForwardOnConnector(ConnRef *, JunctionRef *)` | function (static) | hyperedgetree.cpp:171 | ported (comment) | hyperedgetree.rs:185 `fn travelling_forward_on_connector` |

<a id="junction"></a>
### junction

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `JunctionRefList` | typedef | junction.h:47 | not-ported: subsumed | std::list&lt;JunctionRef*&gt;; Rust uses Vec&lt;JunctionRef&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `JunctionRef` | class | junction.h:57 | ported (comment) | junction.rs:33 `struct JunctionData`<br>junction.rs:42 `struct JunctionRef` |
| `JunctionRef::JunctionRef(Router *, Point, const unsigned int)` | ctor | junction.h:112 (decl)<br>junction.cpp:37 | ported (comment) | junction.rs:55 `fn new` |
| `JunctionRef::~JunctionRef()` | dtor (virtual) | junction.h:122 (decl)<br>junction.cpp:52 | ported (comment) | obstacle.rs:129 `fn delete` |
| `JunctionRef::removeJunctionAndMergeConnectors()` | method | junction.h:141 (decl)<br>junction.cpp:190 | ported (comment) | junction.rs:254 `fn remove_junction_and_merge_connectors` |
| `JunctionRef::position() const` | method | junction.h:145 (decl)<br>junction.cpp:121 | ported (comment) | junction.rs:176 `fn position`<br>obstacle.rs:219 `fn position` |
| `JunctionRef::setPositionFixed(bool)` | method | junction.h:158 (decl)<br>junction.cpp:83 | ported (comment) | junction.rs:132 `fn set_position_fixed` |
| `JunctionRef::positionFixed() const` | method | junction.h:163 (decl)<br>junction.cpp:90 | ported (comment) | junction.rs:143 `fn position_fixed` |
| `JunctionRef::recommendedPosition() const` | method | junction.h:170 (decl)<br>junction.cpp:136 | ported (comment) | junction.rs:203 `fn recommended_position` |
| `JunctionRef::makeRectangle(Router *, const Point &)` | method | junction.h:172 (decl)<br>junction.cpp:63 | ported (comment) | junction.rs:114 `fn make_rectangle` |
| `JunctionRef::preferOrthogonalDimension(const size_t)` | method | junction.h:173 (decl)<br>junction.cpp:96 | ported (comment) | junction.rs:153 `fn prefer_orthogonal_dimension` |
| `JunctionRef::outputCode(FILE *) const` | method | junction.h:181 (decl)<br>junction.cpp:148 | not-ported: debug-output | emits C++ reproduction code (noted in junction.rs) |
| `JunctionRef::setPosition(const Point &)` | method | junction.h:182 (decl)<br>junction.cpp:127 | ported (comment) | junction.rs:186 `fn set_position` |
| `JunctionRef::setRecommendedPosition(const Point &)` | method | junction.h:183 (decl)<br>junction.cpp:142 | ported (comment) | junction.rs:213 `fn set_recommended_position` |
| `JunctionRef::moveAttachedConns(const Point &)` | method | junction.h:184 (decl)<br>junction.cpp:170 | ported (comment) | junction.rs:226 `fn move_attached_conns` |

<a id="makepath"></a>
### makepath

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `AStarPath` | class | makepath.h:37 | ported (comment) | makepath.rs:1395 `struct AStarPath` |
| `AStarPath::AStarPath()` | ctor | makepath.h:40 (decl)<br>makepath.cpp:914 | ported (comment) | makepath.rs:1402 `fn new` |
| `AStarPath::~AStarPath()` | dtor | makepath.h:41 (decl)<br>makepath.cpp:919 | not-ported: subsumed | deletes the pimpl; Rust ownership drops it |
| `AStarPath::search(ConnRef *, VertInf *, VertInf *, VertInf *)` | method | makepath.h:42 (decl)<br>makepath.cpp:924 | ported (comment) | makepath.rs:1412 `fn search` |
| `ANode` | class | makepath.cpp:52 | ported (comment) | makepath.rs:43 `struct ANode` |
| `ANode::ANode(VertInf *, int)` | ctor | makepath.cpp:64 | ported (comment) | makepath.rs:62 `fn new` |
| `ANode::ANode()` | ctor | makepath.cpp:73 | ported (comment) | makepath.rs:76 `fn default` |
| `AStarPathPrivate` | class | makepath.cpp:84 | ported (comment) | makepath.rs:813 `struct AStarPathPrivate` |
| `AStarPathPrivate::AStarPathPrivate()` | ctor | makepath.cpp:87 | not-ported: subsumed | member initialisation only; AStarPathPrivate fields are initialised in makepath.rs |
| `AStarPathPrivate::~AStarPathPrivate()` | dtor | makepath.cpp:94 | not-ported: subsumed | frees the ANode blocks; Rust Vec storage is dropped |
| `AStarPathPrivate::newANode(const ANode &, const bool)` | method | makepath.cpp:104 | ported (comment) | makepath.rs:828 `fn new_a_node` |
| `AStarPathPrivate::search(ConnRef *, VertInf *, VertInf *, VertInf *)` | method | makepath.cpp:125 (decl)<br>makepath.cpp:967 | ported (comment) | makepath.rs:893 `fn search` |
| `AStarPathPrivate::determineEndPointLocation(double, VertInf *, VertInf *, VertInf *, int)` | method | makepath.cpp:129 (decl)<br>makepath.cpp:929 | ported (comment) | makepath.rs:838 `fn determine_end_point_location` |
| `AStarPathPrivate::estimatedCost(ConnRef *, const Point *, const Point &) const` | method | makepath.cpp:131 (decl)<br>makepath.cpp:856 | ported (comment) | makepath.rs:861 `fn estimated_cost` |
| `ANodeCmp` | class | makepath.cpp:153 | not-ported: subsumed | comparator class; its operator() is ported (makepath.rs) |
| `ANodeCmp::ANodeCmp()` | ctor | makepath.cpp:156 | not-ported: subsumed | empty ctor of stateless comparator |
| `ANodeCmp::operator()(const ANode *, const ANode *)` | operator | makepath.cpp:159 | ported (comment) | makepath.rs:100 `fn a_node_cmp` |
| `Dot(const Point &, const Point &)` | function (static) | makepath.cpp:182 | ported (comment) | makepath.rs:209 `fn dot` |
| `CrossLength(const Point &, const Point &)` | function (static) | makepath.cpp:187 | ported (comment) | makepath.rs:214 `fn cross_length` |
| `angleBetween(const Point &, const Point &, const Point &)` | function (static) | makepath.cpp:196 | ported (comment) | makepath.rs:221 `fn angle_between` |
| `constructPolygonPath(Polygon &, VertInf *, VertInf *, ANode *)` | function (static) | makepath.cpp:214 | ported (comment) | makepath.rs:237 `fn construct_polygon_path` |
| `dimDirection(double)` | function (static, inline) | makepath.cpp:292 | ported (comment) | makepath.rs:324 `fn dim_direction` |
| `cost(ConnRef *, const double, VertInf *, VertInf *, ANode *)` | function (static) | makepath.cpp:310 | ported (comment) | makepath.rs:339 `fn cost` |
| `CostDirectionN` | constant | makepath.cpp:489 | ported (comment) | makepath.rs:516 `const COST_DIRECTION_N` |
| `CostDirectionE` | constant | makepath.cpp:490 | ported (comment) | makepath.rs:516 `const COST_DIRECTION_N` |
| `CostDirectionS` | constant | makepath.cpp:491 | ported (comment) | makepath.rs:516 `const COST_DIRECTION_N` |
| `CostDirectionW` | constant | makepath.cpp:492 | ported (comment) | makepath.rs:516 `const COST_DIRECTION_N` |
| `printDirections(FILE *, unsigned int)` | function (static) — conditional (#ifdef ESTIMATED_COST_DEBUG) | makepath.cpp:495 | not-ported: debug-output | ESTIMATED_COST_DEBUG-only debug print |
| `orthogonalDirectionsCount(const unsigned int)` | function (static) | makepath.cpp:517 | ported (comment) | makepath.rs:523 `fn orthogonal_directions_count` |
| `orthogonalDirection(const Point &, const Point &)` | function (static) | makepath.cpp:540 | ported (comment) | makepath.rs:542 `fn orthogonal_direction` |
| `dirRight(unsigned int)` | function (static) | makepath.cpp:566 | ported (comment) | makepath.rs:562 `fn dir_right` |
| `dirLeft(unsigned int)` | function (static) | makepath.cpp:591 | ported (comment) | makepath.rs:580 `fn dir_left` |
| `dirReverse(unsigned int)` | function (static) | makepath.cpp:616 | ported (comment) | makepath.rs:598 `fn dir_reverse` |
| `bends(const Point &, unsigned int, const Point &, unsigned int)` | function | makepath.cpp:648 | ported (comment) | makepath.rs:623 `fn bends` |
| `estimatedCostSpecific(ConnRef *, const Point *, const Point &, const VertInf *, const unsigned int)` | function (static) | makepath.cpp:782 | ported (comment) | makepath.rs:728 `fn estimated_cost_specific` |
| `CmpVisEdgeRotation` | class | makepath.cpp:879 | ported (comment) | makepath.rs:798 |
| `CmpVisEdgeRotation::CmpVisEdgeRotation(const VertInf *)` | ctor | makepath.cpp:882 | not-ported: subsumed | stores lastPt; the ported comparator takes it as an argument (graph.rs) |
| `CmpVisEdgeRotation::operator()(const EdgeInf *, const EdgeInf *) const` | operator | makepath.cpp:886 | ported (comment) | graph.rs:667 `fn cmp_vis_edge_rotation` |
| `pointAlignedWithOneOf(const Point &, const std::vector<Point> &, const size_t)` | function (static, inline) | makepath.cpp:901 | ported (comment) | makepath.rs:802 `fn point_aligned_with_one_of` |

<a id="mtst"></a>
### mtst

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `VertexSetList` | typedef | mtst.h:44 | ported (comment) | mtst.rs:65 `type VertexSetList` |
| `LayeredOrthogonalEdge` | typedef | mtst.h:46 | ported (comment) | mtst.rs:67 `type LayeredOrthogonalEdge` |
| `LayeredOrthogonalEdgeList` | typedef | mtst.h:47 | ported (comment) | mtst.rs:69 `type LayeredOrthogonalEdgeList` |
| `HeapCmpVertInf` | struct | mtst.h:50 | not-ported: subsumed | comparator struct; its operator() is ported (mtst.rs) |
| `HeapCmpVertInf::operator()(const VertInf *, const VertInf *) const` | operator | mtst.h:52 (decl)<br>mtst.cpp:53 | ported (comment) | mtst.rs:81 `fn heap_cmp_vert_inf` |
| `CmpEdgeInf` | struct | mtst.h:57 | not-ported: subsumed | comparator struct; its operator() is ported (mtst.rs) |
| `CmpEdgeInf::operator()(const EdgeInf *, const EdgeInf *) const` | operator | mtst.h:59 (decl)<br>mtst.cpp:60 | ported (comment) | mtst.rs:87 `fn cmp_edge_inf` |
| `MinimumTerminalSpanningTree` | class | mtst.h:66 | ported (comment) | mtst.rs:122 `struct MinimumTerminalSpanningTree` |
| `MinimumTerminalSpanningTree::MinimumTerminalSpanningTree(Router *, std::set<VertInf *>, JunctionHyperedgeTreeNodeMap *)` | ctor | mtst.h:69 (decl)<br>mtst.cpp:76 | ported (comment) | mtst.rs:154 `fn new` |
| `MinimumTerminalSpanningTree::~MinimumTerminalSpanningTree()` | dtor | mtst.h:72 (decl)<br>mtst.cpp:89 | ported (comment) | mtst.rs:179 `fn destruct` |
| `MinimumTerminalSpanningTree::constructInterleaved()` | method | mtst.h:76 (decl)<br>mtst.cpp:633 | ported (comment) | mtst.rs:692 `fn construct_interleaved` |
| `MinimumTerminalSpanningTree::constructSequential()` | method | mtst.h:78 (decl)<br>mtst.cpp:282 | ported (comment) | mtst.rs:372 `fn construct_sequential` |
| `MinimumTerminalSpanningTree::setDebuggingOutput(FILE *, unsigned int)` | method | mtst.h:80 (decl) | not-ported: declared-undefined | declared in mtst.h, never defined |
| `MinimumTerminalSpanningTree::rootJunction() const` | method | mtst.h:81 (decl)<br>mtst.cpp:98 | ported (comment) | mtst.rs:190 `fn root_junction` |
| `MinimumTerminalSpanningTree::buildHyperedgeTreeToRoot(VertInf *, HyperedgeTreeNode *, VertInf *, bool)` | method | mtst.h:84 (decl)<br>mtst.cpp:182 | ported (comment) | mtst.rs:267 `fn build_hyperedge_tree_to_root` |
| `MinimumTerminalSpanningTree::resetDistsForPath(VertInf *, VertInf **)` | method | mtst.h:87 (decl)<br>mtst.cpp:252 | ported (comment) | mtst.rs:341 `fn reset_dists_for_path` |
| `MinimumTerminalSpanningTree::rewriteRestOfHyperedge(VertInf *, VertInf **)` | method | mtst.h:88 (decl)<br>mtst.cpp:886 | ported (comment) | mtst.rs:908 `fn rewrite_rest_of_hyperedge` |
| `MinimumTerminalSpanningTree::drawForest(VertInf *, VertInf *)` | method | mtst.h:89 (decl)<br>mtst.cpp:913 | not-ported: debug-output | debug drawing, only called under DEBUGHANDLER (its fprintf is commented out) |
| `MinimumTerminalSpanningTree::makeSet(VertInf *)` | method | mtst.h:91 (decl)<br>mtst.cpp:104 | ported (comment) | mtst.rs:195 `fn make_set` |
| `MinimumTerminalSpanningTree::findSet(VertInf *)` | method | mtst.h:92 (decl)<br>mtst.cpp:111 | ported (comment) | mtst.rs:203 `fn find_set` |
| `MinimumTerminalSpanningTree::unionSets(VertexSetList::iterator, VertexSetList::iterator)` | method | mtst.h:93 (decl)<br>mtst.cpp:124 | ported (comment) | mtst.rs:210 `fn union_sets` |
| `MinimumTerminalSpanningTree::addNode(VertInf *, HyperedgeTreeNode *)` | method | mtst.h:94 (decl)<br>mtst.cpp:135 | ported (comment) | mtst.rs:222 `fn add_node` |
| `MinimumTerminalSpanningTree::removeInvalidBridgingEdges()` | method | mtst.h:96 (decl)<br>mtst.cpp:543 | ported (comment) | mtst.rs:595 `fn remove_invalid_bridging_edges` |
| `MinimumTerminalSpanningTree::commitToBridgingEdge(EdgeInf *)` | method | mtst.h:97 (decl)<br>mtst.cpp:991 | ported (comment) | mtst.rs:952 `fn commit_to_bridging_edge` |
| `MinimumTerminalSpanningTree::connectsWithoutBend(VertInf *, VertInf *)` | method | mtst.h:98 (decl)<br>mtst.cpp:832 | ported (comment) | mtst.rs:858 `fn connects_without_bend` |
| `MinimumTerminalSpanningTree::getOrthogonalEdgesFromVertex(VertInf *, VertInf *)` | method | mtst.h:99 (decl)<br>mtst.cpp:577 | ported (comment) | mtst.rs:626 `fn get_orthogonal_edges_from_vertex` |
| `MinimumTerminalSpanningTree::orthogonalPartner(VertInf *, double)` | method | mtst.h:101 (decl)<br>mtst.cpp:523 | ported (comment) | mtst.rs:577 `fn orthogonal_partner` |
| `MinimumTerminalSpanningTree::realVerticesCountingPartners(EdgeInf *)` | method | mtst.h:102 (decl)<br>mtst.cpp:964 | ported (comment) | mtst.rs:927 `fn real_vertices_counting_partners` |
| `delete_vertex` | struct | mtst.cpp:66 | ported (comment) | mtst.rs:94 `fn delete_vertex` |
| `delete_vertex::operator()(VertInf *)` | operator | mtst.cpp:68 | ported (manual) | mtst.rs fn delete_vertex() (struct and operator() ported as one fn) |

<a id="obstacle"></a>
### obstacle

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ObstacleList` | typedef | obstacle.h:49 | not-ported: subsumed | std::list&lt;Obstacle*&gt;; Router::m_obstacles is a Vec&lt;ObstacleIdx&gt; (router.rs) |
| `ConnRefList` | typedef | obstacle.h:50 | not-ported: subsumed | std::list&lt;ConnRef*&gt;; Rust uses Vec&lt;ConnIdx&gt; (alias in hyperedge.rs/hyperedgetree.rs) |
| `Obstacle` | class | obstacle.h:56 | ported (comment) | obstacle.rs:61 `struct Obstacle` |
| `Obstacle::Obstacle(Router *, Polygon, const unsigned int)` | ctor | obstacle.h:69 (decl)<br>obstacle.cpp:34 | ported (comment) | obstacle.rs:81 `fn new_base` |
| `Obstacle::~Obstacle()` | dtor (virtual) | obstacle.h:77 (decl)<br>obstacle.cpp:76 | ported (comment) | obstacle.rs:128 `fn delete` |
| `Obstacle::id() const` | method | obstacle.h:82 (decl)<br>obstacle.cpp:249 | ported (comment) | junction.rs:94 `fn id`<br>obstacle.rs:389 `fn id`<br>obstacle.rs:490 `fn id`<br>shape.rs:78 `fn id` |
| `Obstacle::polygon() const` | method | obstacle.h:86 (decl)<br>obstacle.cpp:255 | ported (comment) | junction.rs:104 `fn polygon`<br>obstacle.rs:395 `fn polygon`<br>obstacle.rs:499 `fn polygon` |
| `Obstacle::router() const` | method | obstacle.h:90 (decl)<br>obstacle.cpp:261 | not-ported: subsumed | m_router back-pointer getter; the port passes &Router explicitly (router.rs data model) |
| `Obstacle::position() const` | method (virtual, pure) | obstacle.h:93 (decl) | ported (comment) | obstacle.rs:508 `fn position` |
| `Obstacle::setNewPoly(const Polygon &)` | method | obstacle.h:95 (decl)<br>obstacle.cpp:100 | ported (comment) | obstacle.rs:231 `fn set_new_poly` |
| `Obstacle::firstVert()` | method | obstacle.h:96 (decl)<br>obstacle.cpp:237 | ported (comment) | obstacle.rs:377 `fn first_vert` |
| `Obstacle::lastVert()` | method | obstacle.h:97 (decl)<br>obstacle.cpp:243 | ported (comment) | obstacle.rs:383 `fn last_vert` |
| `Obstacle::routingBox() const` | method | obstacle.h:98 (decl)<br>obstacle.cpp:267 | ported (comment) | obstacle.rs:401 `fn routing_box`<br>obstacle.rs:517 `fn routing_box`<br>shape.rs:117 `fn routing_box` |
| `Obstacle::routingPolygon() const` | method | obstacle.h:99 (decl)<br>obstacle.cpp:277 | ported (comment) | obstacle.rs:410 `fn routing_polygon`<br>obstacle.rs:526 `fn routing_polygon`<br>shape.rs:127 `fn routing_polygon` |
| `Obstacle::attachedConnectors() const` | method | obstacle.h:100 (decl)<br>obstacle.cpp:340 | ported (comment) | junction.rs:312 `fn attached_connectors`<br>obstacle.rs:470 `fn attached_connectors`<br>obstacle.rs:535 `fn attached_connectors`<br>shape.rs:137 `fn attached_connectors` |
| `Obstacle::computeVisibilityNaive()` | method | obstacle.h:111 (decl)<br>visibility.cpp:52 | ported (comment) | visibility.rs:39 `fn compute_visibility_naive` |
| `Obstacle::computeVisibilitySweep()` | method | obstacle.h:112 (decl)<br>visibility.cpp:97 | ported (comment) | visibility.rs:87 `fn compute_visibility_sweep` |
| `Obstacle::outputCode(FILE *) const` | method (virtual, pure) | obstacle.h:114 (decl) | not-ported: debug-output | emits C++ reproduction code (noted in obstacle.rs) |
| `Obstacle::makeActive()` | method | obstacle.h:115 (decl)<br>obstacle.cpp:134 | ported (comment) | obstacle.rs:263 `fn make_active` |
| `Obstacle::makeInactive()` | method | obstacle.h:116 (decl)<br>obstacle.cpp:157 | ported (comment) | obstacle.rs:286 `fn make_inactive` |
| `Obstacle::isActive() const` | method | obstacle.h:117 (decl)<br>obstacle.cpp:231 | ported (comment) | obstacle.rs:371 `fn is_active` |
| `Obstacle::updatePinPolyLineVisibility()` | method | obstacle.h:118 (decl)<br>obstacle.cpp:187 | ported (comment) | obstacle.rs:321 `fn update_pin_poly_line_visibility` |
| `Obstacle::removeFromGraph()` | method | obstacle.h:119 (decl)<br>obstacle.cpp:298 | ported (comment) | obstacle.rs:429 `fn remove_from_graph` |
| `Obstacle::shapeCentre()` | method | obstacle.h:120 (decl)<br>obstacle.cpp:287 | ported (comment) | obstacle.rs:419 `fn shape_centre` |
| `Obstacle::getPointVertex(const Point &)` | method | obstacle.h:122 (decl)<br>obstacle.cpp:311 | ported (comment) | obstacle.rs:443 `fn get_point_vertex` |
| `Obstacle::addFollowingConnEnd(ConnEnd *)` | method | obstacle.h:124 (decl)<br>obstacle.cpp:328 | ported (comment) | obstacle.rs:459 `fn add_following_conn_end` |
| `Obstacle::removeFollowingConnEnd(ConnEnd *)` | method | obstacle.h:125 (decl)<br>obstacle.cpp:334 | ported (comment) | obstacle.rs:464 `fn remove_following_conn_end` |
| `Obstacle::addConnectionPin(ShapeConnectionPin *)` | method | obstacle.h:126 (decl)<br>obstacle.cpp:216 | ported (comment) | obstacle.rs:343 `fn add_connection_pin` |
| `Obstacle::removeConnectionPin(ShapeConnectionPin *)` | method | obstacle.h:127 (decl)<br>obstacle.cpp:224 | ported (comment) | obstacle.rs:358 `fn remove_connection_pin` |
| `Obstacle::assignPinVisibilityTo(const unsigned int, VertInf *)` | method | obstacle.h:128 (decl) | not-ported: declared-undefined | declared in obstacle.h, never defined |
| `Obstacle::possiblePinPoints(unsigned int) const` | method | obstacle.h:130 (decl)<br>obstacle.cpp:198 | ported (comment) | obstacle.rs:329 `fn possible_pin_points` |

<a id="orthogonal"></a>
### orthogonal

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `generateStaticOrthogonalVisGraph(Router *)` | function | orthogonal.h:33 (decl)<br>orthogonal.cpp:1730 | ported (comment) | orthogonal.rs:1336 `fn generate_static_orthogonal_vis_graph` |
| `improveOrthogonalRoutes(Router *)` | function | orthogonal.h:34 (decl)<br>orthogonal.cpp:3252 | ported (comment) | orthogonal.rs:3202 `fn improve_orthogonal_routes` |
| `freeSegmentID` | constant | orthogonal.cpp:54 | ported (comment) | orthogonal.rs:105 `const FREE_SEGMENT_ID` |
| `fixedSegmentID` | constant | orthogonal.cpp:55 | ported (comment) | orthogonal.rs:105 `const FREE_SEGMENT_ID` |
| `channelLeftID` | constant | orthogonal.cpp:56 | ported (comment) | orthogonal.rs:105 `const FREE_SEGMENT_ID` |
| `channelRightID` | constant | orthogonal.cpp:57 | ported (comment) | orthogonal.rs:105 `const FREE_SEGMENT_ID` |
| `freeWeight` | constant | orthogonal.cpp:59 | ported (comment) | orthogonal.rs:111 `const FREE_WEIGHT` |
| `strongWeight` | constant | orthogonal.cpp:60 | ported (comment) | orthogonal.rs:111 `const FREE_WEIGHT` |
| `strongerWeight` | constant | orthogonal.cpp:61 | ported (comment) | orthogonal.rs:111 `const FREE_WEIGHT` |
| `fixedWeight` | constant | orthogonal.cpp:62 | ported (comment) | orthogonal.rs:111 `const FREE_WEIGHT` |
| `UnsignedPair` | class | orthogonal.cpp:67 | ported (comment) | orthogonal.rs:120 `struct UnsignedPair` |
| `UnsignedPair::UnsignedPair(unsigned, unsigned)` | ctor | orthogonal.cpp:70 | ported (comment) | orthogonal.rs:128 `fn new` |
| `UnsignedPair::operator<(const UnsignedPair &) const` | operator | orthogonal.cpp:79 | ported (name) | orthogonal.rs:122 |
| `UnsignedPairSet` | typedef | orthogonal.cpp:91 | ported (comment) | orthogonal.rs:138 `type UnsignedPairSet` |
| `CmpIndexes` | class | orthogonal.cpp:96 | not-ported: subsumed | comparator class; its operator() is ported (orthogonal.rs) |
| `CmpIndexes::CmpIndexes(ConnRef *, size_t)` | ctor | orthogonal.cpp:99 | not-ported: subsumed | stores conn/dim; the ported comparator takes them as arguments |
| `CmpIndexes::operator()(size_t, size_t)` | operator | orthogonal.cpp:104 | ported (comment) | orthogonal.rs:1640 `fn cmp_indexes` |
| `NudgingShiftSegment` | class | orthogonal.cpp:115 | ported (comment) | orthogonal.rs:1646 `struct NudgingShiftSegment` |
| `NudgingShiftSegment::NudgingShiftSegment(ConnRef *, const size_t, const size_t, bool, bool, const size_t, double, double)` | ctor | orthogonal.cpp:119 | ported (comment) | orthogonal.rs:1666 `fn new_shiftable` |
| `NudgingShiftSegment::NudgingShiftSegment(ConnRef *, const size_t, const size_t, const size_t)` | ctor | orthogonal.cpp:138 | ported (comment) | orthogonal.rs:1699 `fn new_fixed` |
| `NudgingShiftSegment::~NudgingShiftSegment()` | dtor (virtual) | orthogonal.cpp:156 | not-ported: subsumed | empty virtual destructor |
| `NudgingShiftSegment::lowPoint()` | method | orthogonal.cpp:159 | ported (comment) | orthogonal.rs:1727 `fn low_point_of`<br>orthogonal.rs:2081 `fn low_point` |
| `NudgingShiftSegment::highPoint()` | method | orthogonal.cpp:163 | ported (comment) | orthogonal.rs:1733 `fn high_point_of`<br>orthogonal.rs:2085 `fn high_point` |
| `NudgingShiftSegment::lowPoint() const` | method | orthogonal.cpp:167 | ported (name) | orthogonal.rs:2082 |
| `NudgingShiftSegment::highPoint() const` | method | orthogonal.cpp:171 | ported (name) | orthogonal.rs:2086 |
| `NudgingShiftSegment::nudgeDistance() const` | method | orthogonal.cpp:175 | ported (comment) | orthogonal.rs:1764 `fn nudge_distance` |
| `NudgingShiftSegment::immovable() const` | method | orthogonal.cpp:179 | ported (comment) | orthogonal.rs:2093 `fn immovable` |
| `NudgingShiftSegment::createSolverVariable(const bool)` | method | orthogonal.cpp:183 | ported (comment) | orthogonal.rs:1772 `fn create_solver_variable` |
| `NudgingShiftSegment::updatePositionsFromSolver(const bool)` | method | orthogonal.cpp:235 | ported (comment) | orthogonal.rs:1814 `fn update_positions_from_solver` |
| `NudgingShiftSegment::fixedOrder(bool &) const` | method | orthogonal.cpp:266 | ported (comment) | orthogonal.rs:1836 `fn fixed_order` |
| `NudgingShiftSegment::order() const` | method | orthogonal.cpp:288 | ported (comment) | orthogonal.rs:1854 `fn order` |
| `NudgingShiftSegment::zigzag() const` | method | orthogonal.cpp:300 | ported (comment) | orthogonal.rs:1864 `fn zigzag` |
| `NudgingShiftSegment::overlapsWith(const ShiftSegment *, const size_t) const` | method | orthogonal.cpp:306 | ported (comment) | orthogonal.rs:1872 `fn nudging_overlaps_with`<br>orthogonal.rs:2089 `fn overlaps_with` |
| `NudgingShiftSegment::canAlignWith(const NudgingShiftSegment *, const size_t) const` | method | orthogonal.cpp:361 | ported (comment) | orthogonal.rs:1914 `fn can_align_with` |
| `NudgingShiftSegment::shouldAlignWith(const ShiftSegment *, const size_t) const` | method | orthogonal.cpp:383 | ported (comment) | orthogonal.rs:1934 `fn should_align_with` |
| `NudgingShiftSegment::mergeWith(const ShiftSegment *, const size_t)` | method | orthogonal.cpp:443 | ported (comment) | orthogonal.rs:1991 `fn merge_with` |
| `NudgingShiftSegment::hasCheckpointAtPosition(const double, const size_t) const` | method | orthogonal.cpp:479 | ported (comment) | orthogonal.rs:2035 `fn has_checkpoint_at_position` |
| `NudgingShiftSegment::lowC() const` | method | orthogonal.cpp:503 | ported (comment) | orthogonal.rs:2047 `fn low_c` |
| `NudgingShiftSegment::highC() const` | method | orthogonal.cpp:514 | ported (comment) | orthogonal.rs:2061 `fn high_c` |
| `ScanVisDirFlag: VisDirNone, VisDirUp, VisDirDown` | enum | orthogonal.cpp:529 | ported (comment) | orthogonal.rs:145 `const VIS_DIR_NONE` |
| `ScanVisDirFlags` | typedef | orthogonal.cpp:534 | ported (comment) | orthogonal.rs:149 `type ScanVisDirFlags` |
| `getPosVertInfDirections(VertInf *, size_t)` | function (static) | orthogonal.cpp:542 | ported (comment) | orthogonal.rs:157 `fn get_pos_vert_inf_directions` |
| `PosVertInf` | struct | orthogonal.cpp:588 | ported (comment) | orthogonal.rs:193 `struct PosVertInf` |
| `PosVertInf::PosVertInf(double, VertInf *, ScanVisDirFlags)` | ctor | orthogonal.cpp:590 | ported (comment) | orthogonal.rs:208 `fn new` |
| `PosVertInf::operator<(const PosVertInf &) const` | operator | orthogonal.cpp:597 | ported (comment) | orthogonal.rs:218 `fn less_than` |
| `CmpVertInf` | struct | orthogonal.cpp:629 | not-ported: subsumed | comparator struct; its operator() is ported (orthogonal.rs) |
| `CmpVertInf::operator()(const VertInf *, const VertInf *) const` | operator | orthogonal.cpp:631 | ported (comment) | orthogonal.rs:284 `fn cmp_vert_inf` |
| `VertSet` | typedef | orthogonal.cpp:651 | ported (comment) | orthogonal.rs:323 `type VertSet` |
| `BreakpointSet` | typedef | orthogonal.cpp:655 | ported (comment) | orthogonal.rs:328 `type BreakpointSet` |
| `LineSegment` | class | orthogonal.cpp:659 | ported (comment) | orthogonal.rs:333 `struct LineSegment` |
| `LineSegment::LineSegment(const double &, const double &, const double &, bool, VertInf *, VertInf *)` | ctor | orthogonal.cpp:662 | ported (comment) | orthogonal.rs:348 `fn new` |
| `LineSegment::LineSegment(const double &, const double &, VertInf *)` | ctor | orthogonal.cpp:680 | ported (comment) | orthogonal.rs:377 `fn new_point` |
| `LineSegment::operator<(const LineSegment &) const` | operator | orthogonal.cpp:693 | ported (comment) | orthogonal.rs:394 `fn less_than` |
| `LineSegment::overlaps(const LineSegment &) const` | method | orthogonal.cpp:711 | ported (comment) | orthogonal.rs:409 `fn overlaps` |
| `LineSegment::mergeVertInfs(const LineSegment &)` | method | orthogonal.cpp:732 | ported (comment) | orthogonal.rs:427 `fn merge_vert_infs` |
| `LineSegment::beginVertInf() const` | method | orthogonal.cpp:739 | ported (comment) | orthogonal.rs:436 `fn begin_vert_inf` |
| `LineSegment::finishVertInf() const` | method | orthogonal.cpp:754 | ported (comment) | orthogonal.rs:446 `fn finish_vert_inf` |
| `LineSegment::commitPositionX(Router *, double)` | method | orthogonal.cpp:770 | ported (comment) | orthogonal.rs:456 `fn commit_position_x` |
| `LineSegment::horiCommitBegin(Router *, VertInf *)` | method | orthogonal.cpp:790 | ported (comment) | orthogonal.rs:476 `fn hori_commit_begin` |
| `LineSegment::horiCommitFinish(Router *, VertInf *)` | method | orthogonal.cpp:809 | ported (comment) | orthogonal.rs:491 `fn hori_commit_finish` |
| `LineSegment::addSegmentsUpTo(double)` | method | orthogonal.cpp:829 | ported (comment) | orthogonal.rs:508 `fn add_segments_up_to` |
| `LineSegment::addEdgeHorizontal(Router *)` | method | orthogonal.cpp:856 | ported (comment) | orthogonal.rs:537 `fn add_edge_horizontal` |
| `LineSegment::setLongRangeVisibilityFlags(size_t)` | method | orthogonal.cpp:867 | ported (comment) | orthogonal.rs:549 `fn set_long_range_visibility_flags` |
| `LineSegment::addEdgeHorizontalTillIntersection(Router *, LineSegment &)` | method | orthogonal.cpp:956 | ported (comment) | orthogonal.rs:619 `fn add_edge_horizontal_till_intersection` |
| `LineSegment::insertBreakpointsBegin(Router *, LineSegment &)` | method | orthogonal.cpp:987 | ported (comment) | orthogonal.rs:654 `fn insert_breakpoints_begin` |
| `LineSegment::insertBreakpointsFinish(Router *, LineSegment &)` | method | orthogonal.cpp:1012 | ported (comment) | orthogonal.rs:677 `fn insert_breakpoints_finish` |
| `LineSegment::generateVisibilityEdgesFromBreakpointSet(Router *, size_t)` | method | orthogonal.cpp:1035 | ported (comment) | orthogonal.rs:699 `fn generate_visibility_edges_from_breakpoint_set` |
| `LineSegment::operator=(LineSegment const &)` | operator | orthogonal.cpp:1219 (decl) | not-ported: declared-undefined | private, never defined (MSVC workaround) |
| `LineSegment::LineSegment()` | ctor | orthogonal.cpp:1220 (decl) | not-ported: declared-undefined | private default ctor, never defined (MSVC workaround) |
| `SegmentList` | typedef | orthogonal.cpp:1223 | ported (comment) | orthogonal.rs:835 `type SegmentList` |
| `SegmentListWrapper` | class | orthogonal.cpp:1225 | ported (comment) | orthogonal.rs:852 `struct SegmentListWrapper` |
| `SegmentListWrapper::insert(LineSegment)` | method | orthogonal.cpp:1228 | ported (comment) | orthogonal.rs:865 `fn insert` |
| `SegmentListWrapper::list()` | method | orthogonal.cpp:1264 | ported (comment) | orthogonal.rs:900 `fn list` |
| `intersectSegments(Router *, SegmentList &, LineSegment &)` | function (static) | orthogonal.cpp:1276 | ported (comment) | orthogonal.rs:909 `fn intersect_segments` |
| `processEventVert(Router *, NodeSet &, SegmentListWrapper &, Event *, unsigned int)` | function (static) | orthogonal.cpp:1368 | ported (comment) | orthogonal.rs:986 `fn process_event_vert` |
| `processEventHori(Router *, NodeSet &, SegmentListWrapper &, Event *, unsigned int)` | function (static) | orthogonal.cpp:1551 | ported (comment) | orthogonal.rs:1174 `fn process_event_hori` |
| `fixConnectionPointVisibilityOnOutsideOfVisibilityGraph(Event **, size_t, ConnDirFlags)` | function | orthogonal.cpp:1691 | ported (comment) | orthogonal.rs:1299 `fn fix_connection_point_visibility_on_outside_of_visibility_graph` |
| `RectBounds` | typedef | orthogonal.cpp:1989 | ported (comment) | orthogonal.rs:2099 `type RectBounds` |
| `insideRectBounds(const Point &, const RectBounds &)` | function (static) | orthogonal.cpp:1991 | ported (comment) | orthogonal.rs:2102 `fn inside_rect_bounds` |
| `buildOrthogonalNudgingSegments(Router *, const size_t, ShiftSegmentList &)` | function (static) | orthogonal.cpp:2015 | ported (comment) | orthogonal.rs:2121 `fn build_orthogonal_nudging_segments` |
| `ConnRefVector` | typedef | orthogonal.cpp:2278 | not-ported: subsumed | local std::vector&lt;ConnRef*&gt; in improveOrthogonalRoutes; Rust Vec |
| `RouteVector` | typedef | orthogonal.cpp:2279 | not-ported: subsumed | local std::vector&lt;Polygon&gt; in improveOrthogonalRoutes; Rust Vec |
| `CmpLineOrder` | class | orthogonal.cpp:2282 | ported (comment) | orthogonal.rs:2339 `struct CmpLineOrder` |
| `CmpLineOrder::CmpLineOrder(PtOrderMap &, const size_t)` | ctor | orthogonal.cpp:2285 | not-ported: subsumed | stores orders/dimension; the ported CmpLineOrder is built from them (orthogonal.rs) |
| `CmpLineOrder::operator()(const ShiftSegment *, const ShiftSegment *, bool *) const` | operator | orthogonal.cpp:2290 | ported (comment) | orthogonal.rs:2348 `fn call` |
| `linesort(bool, ShiftSegmentList, CmpLineOrder &)` | function (static) | orthogonal.cpp:2378 | ported (comment) | orthogonal.rs:2418 `fn linesort` |
| `ShiftSegmentPtrList` | typedef | orthogonal.cpp:2459 | not-ported: subsumed | std::list&lt;ShiftSegment*&gt;; Rust Vec (orthogonal.rs) |
| `PotentialSegmentConstraint` | class | orthogonal.cpp:2461 | ported (comment) | orthogonal.rs:2490 `struct PotentialSegmentConstraint` |
| `PotentialSegmentConstraint::PotentialSegmentConstraint(size_t, size_t, const Variables &)` | ctor | orthogonal.cpp:2464 | ported (comment) | orthogonal.rs:2498 `fn new` |
| `PotentialSegmentConstraint::operator<(const PotentialSegmentConstraint) const` | operator | orthogonal.cpp:2472 | ported (comment) | orthogonal.rs:2503 `fn less_than` |
| `PotentialSegmentConstraint::sepDistance() const` | method | orthogonal.cpp:2476 | ported (comment) | orthogonal.rs:2508 `fn sep_distance` |
| `PotentialSegmentConstraint::stillValid() const` | method | orthogonal.cpp:2484 | ported (comment) | orthogonal.rs:2516 `fn still_valid` |
| `PotentialSegmentConstraint::rewriteIndex(size_t, size_t)` | method | orthogonal.cpp:2488 | ported (comment) | orthogonal.rs:2521 `fn rewrite_index` |
| `ImproveOrthogonalRoutes` | class | orthogonal.cpp:2509 | ported (comment) | orthogonal.rs:2533 `struct ImproveOrthogonalRoutes` |
| `ImproveOrthogonalRoutes::ImproveOrthogonalRoutes(Router *)` | ctor | orthogonal.cpp:2512 (decl)<br>orthogonal.cpp:2528 | ported (comment) | orthogonal.rs:2541 `fn new` |
| `ImproveOrthogonalRoutes::execute()` | method | orthogonal.cpp:2513 (decl)<br>orthogonal.cpp:2533 | ported (comment) | orthogonal.rs:2550 `fn execute` |
| `ImproveOrthogonalRoutes::simplifyOrthogonalRoutes()` | method | orthogonal.cpp:2516 (decl)<br>orthogonal.cpp:3132 | ported (comment) | orthogonal.rs:3081 `fn simplify_orthogonal_routes` |
| `ImproveOrthogonalRoutes::buildOrthogonalNudgingOrderInfo()` | method | orthogonal.cpp:2517 (decl)<br>orthogonal.cpp:3148 | ported (comment) | orthogonal.rs:3095 `fn build_orthogonal_nudging_order_info` |
| `ImproveOrthogonalRoutes::nudgeOrthogonalRoutes(size_t, bool)` | method | orthogonal.cpp:2518 (decl)<br>orthogonal.cpp:2597 | ported (comment) | orthogonal.rs:2606 `fn nudge_orthogonal_routes` |

<a id="router"></a>
### router

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `LineRep` | struct | router.h:50 | ported (comment) | router.rs:250 `struct LineRep` |
| `LineReps` | typedef | router.h:55 | ported (comment) | router.rs:250 `struct LineRep` |
| `IntList` | typedef | router.h:58 | ported (comment) | router.rs:258 `type IntList` |
| `ClusterRefList` | typedef | router.h:63 | not-ported: subsumed | std::list&lt;ClusterRef*&gt;; Router::cluster_refs is a Vec (router.rs) |
| `ObstacleList` | typedef | router.h:65 | not-ported: subsumed | std::list&lt;Obstacle*&gt;; Router::m_obstacles is a Vec&lt;ObstacleIdx&gt; (router.rs) |
| `RouterFlag: PolyLineRouting, OrthogonalRouting` | enum | router.h:70 | ported (comment) | router.rs:166 `const POLY_LINE_ROUTING` |
| `runningTo` | constant | router.h:81 | ported (comment) | router.rs:170 `const RUNNING_TO` |
| `runningFrom` | constant | router.h:82 | ported (comment) | router.rs:170 `const RUNNING_TO` |
| `runningToAndFrom` | constant | router.h:83 | ported (comment) | router.rs:170 `const RUNNING_TO` |
| `RoutingParameter: segmentPenalty, anglePenalty, crossingPenalty, clusterCrossingPenalty, fixedSharedPathPenalty, portDirectionPenalty, shapeBufferDistance, idealNudgingDistance, reverseDirectionPenalty, lastRoutingParameterMarker` | enum | router.h:88 | ported (comment) | router.rs:181 `enum RoutingParameter`<br>router.rs:194 `const LAST_ROUTING_PARAMETER_MARKER` |
| `PenaltyType` | typedef | router.h:170 | ported (comment) | router.rs:196 `type PenaltyType` |
| `RoutingOption: nudgeOrthogonalSegmentsConnectedToShapes, improveHyperedgeRoutesMovingJunctions, penaliseOrthogonalSharedPathsAtConnEnds, nudgeOrthogonalTouchingColinearSegments, performUnifyingNudgingPreprocessingStep, improveHyperedgeRoutesMovingAddingAndDeletingJunctions, nudgeSharedPathsWithCommonEndPoint, lastRoutingOptionMarker` | enum | router.h:174 | ported (comment) | router.rs:214 `enum RoutingOption`<br>router.rs:225 `const LAST_ROUTING_OPTION_MARKER` |
| `TransactionPhases: TransactionPhaseOrthogonalVisibilityGraphScanX, TransactionPhaseOrthogonalVisibilityGraphScanY, TransactionPhaseRouteSearch, TransactionPhaseCrossingDetection, TransactionPhaseRerouteSearch, TransactionPhaseOrthogonalNudgingX, TransactionPhaseOrthogonalNudgingY, TransactionPhaseCompleted` | enum | router.h:299 | ported (comment) | router.rs:240 `const TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X` |
| `ConnRerouteFlagDelegate` | class | router.h:332 | ported (comment) | router.rs:264 `struct ConnRerouteFlagDelegate` |
| `ConnRerouteFlagDelegate::ConnRerouteFlagDelegate()` | ctor | router.h:334 (decl)<br>router.cpp:3097 | not-ported: subsumed | empty constructor (Default) |
| `ConnRerouteFlagDelegate::~ConnRerouteFlagDelegate()` | dtor | router.h:335 (decl)<br>router.cpp:3101 | not-ported: subsumed | empty destructor |
| `ConnRerouteFlagDelegate::addConn(ConnRef *)` | method | router.h:336 (decl)<br>router.cpp:3105 | ported (comment) | router.rs:271 `fn add_conn` |
| `ConnRerouteFlagDelegate::removeConn(ConnRef *)` | method | router.h:337 (decl)<br>router.cpp:3111 | ported (comment) | router.rs:277 `fn remove_conn` |
| `ConnRerouteFlagDelegate::alertConns()` | method | router.h:338 (decl)<br>router.cpp:3124 | ported (comment) | router.rs:292 `fn alert_conns` |
| `zeroParamValue` | constant | router.h:343 | ported (comment) | router.rs:175 `const ZERO_PARAM_VALUE` |
| `chooseSensibleParamValue` | constant | router.h:344 | ported (comment) | router.rs:175 `const ZERO_PARAM_VALUE` |
| `TopologyAddonInterface` | class | router.h:350 | ported (comment) | router.rs:305 `trait TopologyAddonInterface` |
| `TopologyAddonInterface::TopologyAddonInterface()` | ctor | router.h:353 | not-ported: subsumed | empty ctor; TopologyAddonInterface is a Rust trait |
| `TopologyAddonInterface::~TopologyAddonInterface()` | dtor (virtual) | router.h:356 | not-ported: subsumed | empty virtual dtor; TopologyAddonInterface is a Rust trait |
| `TopologyAddonInterface::clone() const` | method (virtual) | router.h:359 | ported (manual) | router.rs trait TopologyAddonInterface::clone_box() (Box&lt;dyn&gt; clone) |
| `TopologyAddonInterface::improveOrthogonalTopology(Router *)` | method (virtual) | router.h:364 | ported (name) | router.rs:310 |
| `TopologyAddonInterface::outputCode(FILE *) const` | method (virtual) | router.h:368 | ported (name) | router.rs:315 |
| `TopologyAddonInterface::outputDeletionCode(FILE *) const` | method (virtual) | router.h:373 | ported (name) | router.rs:320 |
| `Router` | class | router.h:386 | ported (comment) | router.rs:341 `struct Router` |
| `Router::Router(const unsigned int)` | ctor | router.h:392 (decl)<br>router.cpp:47 | ported (comment) | router.rs:764 `fn new` |
| `Router::~Router()` | dtor (virtual) | router.h:399 (decl)<br>router.cpp:108 | ported (comment) | router.rs:846 `fn destruct` |
| `Router::setTransactionUse(const bool)` | method | router.h:453 (decl)<br>router.cpp:458 | ported (comment) | router.rs:1179 `fn set_transaction_use` |
| `Router::transactionUse() const` | method | router.h:462 (decl)<br>router.cpp:452 | ported (comment) | router.rs:1174 `fn transaction_use` |
| `Router::processTransaction()` | method | router.h:479 (decl)<br>router.cpp:641 | ported (comment) | router.rs:1367 `fn process_transaction` |
| `Router::deleteShape(ShapeRef *)` | method | router.h:495 (decl)<br>router.cpp:282 | ported (comment) | router.rs:1010 `fn delete_shape` |
| `Router::moveShape(ShapeRef *, const Polygon &, const bool)` | method | router.h:515 (decl)<br>router.cpp:361 | ported (comment) | router.rs:1091 `fn move_shape` |
| `Router::moveShape(ShapeRef *, const double, const double)` | method | router.h:535 (decl)<br>router.cpp:320 | ported (comment) | router.rs:1058 `fn move_shape_by` |
| `Router::deleteJunction(JunctionRef *)` | method | router.h:549 (decl)<br>router.cpp:686 | ported (comment) | router.rs:1428 `fn delete_junction` |
| `Router::deleteConnector(ConnRef *)` | method | router.h:563 (decl)<br>router.cpp:313 | ported (comment) | router.rs:1046 `fn delete_connector` |
| `Router::moveJunction(JunctionRef *, const Point &)` | method | router.h:578 (decl)<br>router.cpp:742 | ported (comment) | router.rs:1483 `fn move_junction` |
| `Router::moveJunction(JunctionRef *, const double, const double)` | method | router.h:597 (decl)<br>router.cpp:717 | ported (comment) | router.rs:1462 `fn move_junction_by` |
| `Router::setRoutingParameter(const RoutingParameter, const double)` | method | router.h:626 (decl)<br>router.cpp:1996 | ported (comment) | router.rs:2279 `fn set_routing_parameter` |
| `Router::routingParameter(const RoutingParameter) const` | method | router.h:635 (decl)<br>router.cpp:2039 | ported (comment) | router.rs:2300 `fn routing_parameter` |
| `Router::setRoutingOption(const RoutingOption, const bool)` | method | router.h:642 (decl)<br>router.cpp:2046 | ported (comment) | router.rs:2306 `fn set_routing_option` |
| `Router::routingOption(const RoutingOption) const` | method | router.h:649 (decl)<br>router.cpp:2054 | ported (comment) | router.rs:2312 `fn routing_option` |
| `Router::setRoutingPenalty(const RoutingParameter, const double)` | method | router.h:661 (decl)<br>router.cpp:2061 | ported (comment) | router.rs:2321 `fn set_routing_penalty` |
| `Router::hyperedgeRerouter()` | method | router.h:669 (decl)<br>router.cpp:2072 | ported (comment) | router.rs:2331 `fn hyperedge_rerouter` |
| `Router::outputInstanceToSVG(std::string)` | method | router.h:682 (decl)<br>router.cpp:2359 | not-ported: debug-output | SVG debug output, out of scope (mod.rs) |
| `Router::newObjectId() const` | method (virtual) | router.h:694 (decl)<br>router.cpp:802 | ported (comment) | router.rs:1554 `fn new_object_id` |
| `Router::objectIdIsUnused(const unsigned int) const` | method | router.h:703 (decl)<br>router.cpp:828 | ported (comment) | router.rs:1580 `fn object_id_is_unused` |
| `Router::shouldContinueTransactionWithProgress(unsigned int, unsigned int, unsigned int, double)` | method (virtual) | router.h:743 (decl)<br>router.cpp:1376 | ported (comment) | router.rs:1768 `fn should_continue_transaction_with_progress` |
| `Router::newAndDeletedObjectListsFromHyperedgeImprovement() const` | method | router.h:763 (decl)<br>router.cpp:3091 | ported (comment) | router.rs:2608 `fn new_and_deleted_object_lists_from_hyperedge_improvement` |
| `Router::setDebugHandler(DebugHandler *)` | method | router.h:765 (decl)<br>router.cpp:149 | not-ported: debug-output | DebugHandler is not ported (noted in router.rs) |
| `Router::debugHandler() const` | method | router.h:766 (decl)<br>router.cpp:154 | not-ported: debug-output | DebugHandler is not ported (noted in router.rs) |
| `Router::processActions()` | method | router.h:770 (decl)<br>router.cpp:465 | ported (comment) | router.rs:1185 `fn process_actions` |
| `Router::deleteCluster(ClusterRef *)` | method | router.h:772 (decl)<br>router.cpp:792 | ported (comment) | router.rs:1540 `fn delete_cluster` |
| `Router::attachedShapes(IntList &, const unsigned int, const unsigned int)` | method | router.h:773 (decl)<br>router.cpp:892 | ported (comment) | router.rs:1625 `fn attached_shapes` |
| `Router::attachedConns(IntList &, const unsigned int, const unsigned int)` | method | router.h:775 (decl)<br>router.cpp:868 | ported (comment) | router.rs:1608 `fn attached_conns` |
| `Router::markPolylineConnectorsNeedingReroutingForDeletedObstacle(Obstacle *)` | method | router.h:777 (decl)<br>router.cpp:1768 | ported (comment) | router.rs:2108 `fn mark_polyline_connectors_needing_rerouting_for_deleted_obstacle` |
| `Router::generateContains(VertInf *)` | method | router.h:779 (decl)<br>router.cpp:1674 | ported (comment) | router.rs:2024 `fn generate_contains` |
| `Router::printInfo()` | method | router.h:780 (decl)<br>router.cpp:2084 | ported (comment) | router.rs:2343 `fn print_info` |
| `Router::regenerateStaticBuiltGraph()` | method | router.h:781 (decl)<br>router.cpp:431 | ported (comment) | router.rs:1159 `fn regenerate_static_built_graph` |
| `Router::destroyOrthogonalVisGraph()` | method | router.h:782 (decl)<br>router.cpp:410 | ported (comment) | router.rs:1141 `fn destroy_orthogonal_vis_graph` |
| `Router::setStaticGraphInvalidated(const bool)` | method | router.h:783 (decl)<br>router.cpp:404 | ported (comment) | router.rs:1136 `fn set_static_graph_invalidated` |
| `Router::validConnType(const ConnType) const` | method | router.h:784 (decl)<br>router.cpp:1970 | ported (comment) | router.rs:2259 `fn valid_conn_type` |
| `Router::isInCrossingPenaltyReroutingStage() const` | method | router.h:785 (decl)<br>router.cpp:2078 | ported (comment) | router.rs:2336 `fn is_in_crossing_penalty_rerouting_stage` |
| `Router::markAllObstaclesAsMoved()` | method | router.h:786 (decl)<br>router.cpp:343 | ported (comment) | router.rs:1077 `fn mark_all_obstacles_as_moved` |
| `Router::shapeContainingPoint(const Point &)` | method | router.h:787 (decl)<br>router.cpp:159 | ported (comment) | router.rs:912 `fn shape_containing_point` |
| `Router::performContinuationCheck(unsigned int, size_t, size_t)` | method | router.h:788 (decl)<br>router.cpp:1357 | ported (comment) | router.rs:1749 `fn perform_continuation_check` |
| `Router::registerSettingsChange()` | method | router.h:790 (decl)<br>router.cpp:2067 | ported (comment) | router.rs:2326 `fn register_settings_change` |
| `Router::setTopologyAddon(TopologyAddonInterface *)` | method | router.h:799 (decl)<br>router.cpp:2338 | ported (comment) | router.rs:2522 `fn set_topology_addon` |
| `Router::improveOrthogonalTopology()` | method | router.h:800 (decl)<br>router.cpp:2353 | ported (comment) | router.rs:2531 `fn improve_orthogonal_topology` |
| `Router::existsOrthogonalSegmentOverlap(const bool)` | method | router.h:803 (decl)<br>router.cpp:2175 | ported (comment) | router.rs:2436 `fn exists_orthogonal_segment_overlap` |
| `Router::existsOrthogonalFixedSegmentOverlap(const bool)` | method | router.h:804 (decl)<br>router.cpp:2208 | ported (comment) | router.rs:2446 `fn exists_orthogonal_fixed_segment_overlap` |
| `Router::existsOrthogonalTouchingPaths()` | method | router.h:805 (decl)<br>router.cpp:2242 | ported (comment) | router.rs:2455 `fn exists_orthogonal_touching_paths` |
| `Router::existsCrossings(const bool)` | method | router.h:806 (decl)<br>router.cpp:2278 | ported (comment) | router.rs:2466 `fn exists_crossings` |
| `Router::existsInvalidOrthogonalPaths()` | method | router.h:807 (decl)<br>router.cpp:2311 | ported (comment) | router.rs:2497 `fn exists_invalid_orthogonal_paths` |
| `Router::outputDiagramSVG(std::string, LineReps *)` | method | router.h:813 (decl)<br>router.cpp:2869 | not-ported: debug-output | SVG debug output, out of scope (mod.rs) |
| `Router::outputDiagramText(std::string)` | method | router.h:816 (decl)<br>router.cpp:3019 | ported (comment) | router.rs:2540 `fn output_diagram_text_g`<br>router.rs:2548 `fn output_diagram_text` |
| `Router::outputDiagram(std::string)` | method | router.h:817 (decl)<br>router.cpp:3011 | not-ported: subsumed | wrapper calling outputDiagramText() (+ SVG under SVG_OUTPUT); the port exposes Router::output_diagram_text() returning a String |
| `Router::assignId(const unsigned int)` | method | router.h:832 (decl)<br>router.cpp:808 | ported (comment) | router.rs:1559 `fn assign_id` |
| `Router::addShape(ShapeRef *)` | method | router.h:833 (decl)<br>router.cpp:256 | ported (comment) | router.rs:981 `fn add_shape` |
| `Router::addJunction(JunctionRef *)` | method | router.h:834 (decl)<br>router.cpp:660 | ported (comment) | router.rs:1400 `fn add_junction` |
| `Router::addCluster(ClusterRef *)` | method | router.h:835 (decl)<br>router.cpp:781 | ported (comment) | router.rs:1526 `fn add_cluster` |
| `Router::modifyConnector(ConnRef *)` | method | router.h:836 (decl)<br>router.cpp:203 | ported (comment) | router.rs:950 `fn modify_connector` |
| `Router::modifyConnector(ConnRef *, const unsigned int, const ConnEnd &, bool)` | method | router.h:837 (decl)<br>router.cpp:177 | ported (comment) | router.rs:929 `fn modify_connector_end` |
| `Router::modifyConnectionPin(ShapeConnectionPin *)` | method | router.h:839 (decl)<br>router.cpp:221 | ported (comment) | router.rs:963 `fn modify_connection_pin` |
| `Router::removeObjectFromQueuedActions(const void *)` | method | router.h:841 (decl)<br>router.cpp:239 | ported (comment) | router.rs:976 `fn remove_object_from_queued_actions` |
| `Router::newBlockingShape(const Polygon &, int)` | method | router.h:842 (decl)<br>router.cpp:1551 | ported (comment) | router.rs:1919 `fn new_blocking_shape` |
| `Router::checkAllBlockedEdges(int)` | method | router.h:843 (decl)<br>router.cpp:1616 | ported (comment) | router.rs:1969 `fn check_all_blocked_edges` |
| `Router::checkAllMissingEdges()` | method | router.h:844 (decl)<br>router.cpp:1638 | ported (comment) | router.rs:1987 `fn check_all_missing_edges` |
| `Router::adjustContainsWithAdd(const Polygon &, const int)` | method | router.h:845 (decl)<br>router.cpp:1729 | ported (comment) | router.rs:2080 `fn adjust_contains_with_add` |
| `Router::adjustContainsWithDel(const int)` | method | router.h:846 (decl)<br>router.cpp:1745 | ported (comment) | router.rs:2097 `fn adjust_contains_with_del` |
| `Router::adjustClustersWithAdd(const PolygonInterface &, const int)` | method | router.h:847 (decl)<br>router.cpp:1705 | ported (comment) | router.rs:2059 `fn adjust_clusters_with_add` |
| `Router::adjustClustersWithDel(const int)` | method | router.h:849 (decl)<br>router.cpp:1719 | ported (comment) | router.rs:2073 `fn adjust_clusters_with_del` |
| `Router::rerouteAndCallbackConnectors()` | method | router.h:850 (decl)<br>router.cpp:925 | ported (comment) | router.rs:1649 `fn reroute_and_callback_connectors` |
| `Router::improveCrossings()` | method | router.h:851 (decl)<br>router.cpp:1409 | ported (comment) | router.rs:1783 `fn improve_crossings` |
| `ConnCostRef` | typedef | router.cpp:1037 | ported (comment) | router.rs:499 `type ConnCostRef` |
| `CmpConnCostRef` | class | router.cpp:1040 | ported (comment) | router.rs:504 `type ConnCostRefSet` |
| `CmpConnCostRef::CmpConnCostRef()` | ctor | router.cpp:1043 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpConnCostRef::operator()(const ConnCostRef &, const ConnCostRef &) const` | operator | router.cpp:1046 | not-ported: subsumed | order-by-connector-id realised as the BTreeMap&lt;u32, ..&gt; key of ConnCostRefSet (router.rs) |
| `ConnCostRefSet` | typedef | router.cpp:1052 | ported (comment) | router.rs:504 `type ConnCostRefSet` |
| `ConnCostRefSetList` | typedef | router.cpp:1053 | ported (name) | router.rs:506 |
| `cheapEstimatedCost(ConnRef *)` | function (static) | router.cpp:1056 | ported (comment) | router.rs:512 `fn cheap_estimated_cost` |
| `CrossingConnectorsMap` | typedef | router.cpp:1079 | ported (comment) | router.rs:508 `type CrossingConnectorsMap` |
| `CrossingConnectorsMapList` | typedef | router.cpp:1082 | not-ported: subsumed | std::list&lt;CrossingConnectorsMap&gt;; Rust Vec (router.rs field comment) |
| `CrossingConnectorsInfo` | class | router.cpp:1087 | ported (comment) | router.rs:540 `struct CrossingConnectorsInfo` |
| `CrossingConnectorsInfo::addCrossing(ConnRef *, ConnRef *)` | method | router.cpp:1092 | ported (comment) | router.rs:552 `fn add_crossing` |
| `CrossingConnectorsInfo::crossingSetsListToRemoveCrossingsFromGroups()` | method | router.cpp:1111 | ported (comment) | router.rs:565 `fn crossing_sets_list_to_remove_crossings_from_groups` |
| `CrossingConnectorsInfo::connsKnownToCross(ConnRef *, ConnRef *)` | method | router.cpp:1205 | ported (comment) | router.rs:648 `fn conns_known_to_cross` |
| `CrossingConnectorsInfo::removeConnectorWithMostCrossings(CrossingConnectorsMap &)` | method | router.cpp:1227 | ported (comment) | router.rs:665 `fn remove_connector_with_most_crossings` |
| `CrossingConnectorsInfo::groupForConn(ConnRef *)` | method | router.cpp:1287 | ported (comment) | router.rs:719 `fn group_for_conn` |
| `CrossingConnectorsInfo::groupForCrossingConns(ConnRef *, ConnRef *)` | method | router.cpp:1310 | ported (comment) | router.rs:726 `fn group_for_crossing_conns` |
| `CmpOrderedConnCostRef` | class | router.cpp:1395 | not-ported: subsumed | comparator class; its operator() is inlined (see below) |
| `CmpOrderedConnCostRef::CmpOrderedConnCostRef()` | ctor | router.cpp:1398 | not-ported: subsumed | empty ctor of stateless comparator |
| `CmpOrderedConnCostRef::operator()(const ConnCostRef &, const ConnCostRef &) const` | operator | router.cpp:1401 | ported (manual) | router.rs Router::improve_crossings(): closure `u.0 &lt; v.0` passed to libstdcxx_list_sort |
| `ConnCostRefList` | typedef | router.cpp:1406 | not-ported: subsumed | std::list&lt;ConnCostRef&gt;; Rust Vec sorted with libstdcxx_list_sort (router.rs) |
| `AngleAFromThreeSides(const double, const double, const double)` | function (static) — conditional (#ifdef SELECTIVE_DEBUG) | router.cpp:1755 | not-ported: dead-code | only compiled with SELECTIVE_DEBUG |
| `LIMIT` | constant | router.cpp:2162 | not-ported: debug-output | coordinate clamp used only by the SVG output functions |
| `reduceRange(double &)` | function (static) | router.cpp:2164 | not-ported: debug-output | coordinate clamp used only by the SVG output functions |

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
| `buildOrthogonalChannelInfo(Router *, const size_t, ShiftSegmentList &)` | function | scanline.h:130 (decl)<br>scanline.cpp:462 | ported (comment) | scanline.rs:793 `fn build_orthogonal_channel_info` |
| `processShiftEvent(NodeSet &, Event *, size_t, unsigned int)` | function (static) | scanline.cpp:388 | ported (comment) | scanline.rs:745 `fn process_shift_event` |

<a id="shape"></a>
### shape

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `ShapeRefList` | typedef | shape.h:53 | not-ported: subsumed | std::list&lt;ShapeRef*&gt;; Rust uses Vec of handles |
| `ShapeTransformationType: TransformationType_CW90, TransformationType_CW180, TransformationType_CW270, TransformationType_FlipX, TransformationType_FlipY` | enum | shape.h:57 | ported (comment) | shape.rs:35 `enum ShapeTransformationType` |
| `ShapeRef` | class | shape.h:81 | ported (comment) | shape.rs:51 `struct ShapeRef` |
| `ShapeRef::ShapeRef(Router *, Polygon &, const unsigned int)` | ctor | shape.h:119 (decl)<br>shape.cpp:39 | ported (comment) | shape.rs:64 `fn new` |
| `ShapeRef::~ShapeRef()` | dtor (virtual) | shape.h:129 (decl)<br>shape.cpp:46 | ported (comment) | obstacle.rs:129 `fn delete` |
| `ShapeRef::polygon() const` | method | shape.h:134 (decl)<br>shape.cpp:228 | ported (comment) | shape.rs:89 `fn polygon` |
| `ShapeRef::transformConnectionPinPositions(ShapeTransformationType)` | method | shape.h:143 (decl)<br>shape.cpp:93 | ported (comment) | shape.rs:186 `fn transform_connection_pin_positions` |
| `ShapeRef::position() const` | method | shape.h:145 (decl)<br>shape.cpp:260 | ported (comment) | obstacle.rs:219 `fn position`<br>shape.rs:100 `fn position` |
| `ShapeRef::outputCode(FILE *) const` | method | shape.h:152 (decl)<br>shape.cpp:234 | not-ported: debug-output | emits C++ reproduction code (noted in shape.rs) |
| `ShapeRef::moveAttachedConns(const Polygon &)` | method | shape.h:153 (decl)<br>shape.cpp:57 | ported (comment) | shape.rs:157 `fn move_attached_conns` |
| `ShapeRef::assignPinVisibilityTo(const unsigned int, VertInf *)` | method | shape.h:154 (decl) | not-ported: declared-undefined | declared in shape.h, never defined (noted in shape.rs) |
| `ShapeRef::setCentrePos(const Point &)` | method | shape.h:156 (decl)<br>shape.cpp:273 | ported (comment) | shape.rs:340 `fn set_centre_pos` |
| `absoluteOffsetInverse(double, const Box &, size_t)` | function (static) | shape.cpp:77 | ported (comment) | shape.rs:357 `fn absolute_offset_inverse` |

<a id="vertices"></a>
### vertices

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `EdgeInfList` | typedef | vertices.h:44 | ported (name) | vertices.rs:260 |
| `VertexPair` | typedef | vertices.h:45 | ported (comment) | mtst.rs:71 `type VertexPair`<br>vertices.rs:47 `type VertexPair` |
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
| `VertID::operator==(const VertID &) const` | operator | vertices.h:72 (decl)<br>vertices.cpp:77 | ported (comment) | vertices.rs:151 `fn eq` |
| `VertID::operator!=(const VertID &) const` | operator | vertices.h:73 (decl)<br>vertices.cpp:87 | ported (comment) | vertices.rs:151 `fn eq` |
| `VertID::operator<(const VertID &) const` | operator | vertices.h:74 (decl)<br>vertices.cpp:97 | ported (comment) | vertices.rs:175 `fn cmp` |
| `VertID::operator+(const int &) const` | operator | vertices.h:75 (decl)<br>vertices.cpp:108 | ported (comment) | vertices.rs:97 `fn add` |
| `VertID::operator-(const int &) const` | operator | vertices.h:76 (decl)<br>vertices.cpp:114 | ported (comment) | vertices.rs:103 `fn sub` |
| `VertID::operator++(int)` | operator | vertices.h:77 (decl)<br>vertices.cpp:120 | ported (comment) | vertices.rs:111 `fn post_inc` |
| `VertID::print(FILE *) const` | method | vertices.h:78 (decl)<br>vertices.cpp:127 | ported (comment) | vertices.rs:118 `fn print_string` |
| `VertID::db_print() const` | method | vertices.h:79 (decl)<br>vertices.cpp:132 | ported (comment) | vertices.rs:118 `fn print_string` |
| `operator<<(ostream &, const VertID &)` | operator (friend) | vertices.h:80 (decl)<br>vertices.cpp:149 | ported (comment) | vertices.rs:182 `fn fmt` |
| `VertID::isOrthShapeEdge() const` | method (inline) | vertices.h:83 | ported (comment) | vertices.rs:123 `fn is_orth_shape_edge` |
| `VertID::isConnPt() const` | method (inline) | vertices.h:87 | ported (comment) | vertices.rs:128 `fn is_conn_pt` |
| `VertID::isConnectionPin() const` | method (inline) | vertices.h:91 | ported (comment) | vertices.rs:133 `fn is_connection_pin` |
| `VertID::isConnCheckpoint() const` | method (inline) | vertices.h:95 | ported (comment) | vertices.rs:138 `fn is_conn_checkpoint` |
| `VertID::isDummyPinHelper() const` | method (inline) | vertices.h:99 | ported (comment) | vertices.rs:143 `fn is_dummy_pin_helper` |
| `dummyOrthogID` | constant | vertices.h:110 | ported (comment) | vertices.rs:191 `const DUMMY_ORTHOG_ID` |
| `dummyOrthogShapeID` | constant | vertices.h:111 | ported (comment) | vertices.rs:191 `const DUMMY_ORTHOG_ID` |
| `VertInf` | class | vertices.h:115 | ported (comment) | vertices.rs:530 `struct VertInf` |
| `VertInf::VertInf(Router *, const VertID &, const Point &, const bool)` | ctor | vertices.h:118 (decl)<br>vertices.cpp:156 | ported (comment) | vertices.rs:569 `fn new` |
| `VertInf::~VertInf()` | dtor | vertices.h:120 (decl)<br>vertices.cpp:184 | ported (comment) | vertices.rs:621 `fn delete` |
| `VertInf::Reset(const VertID &, const Point &)` | method | vertices.h:121 (decl)<br>vertices.cpp:204 | ported (comment) | vertices.rs:646 `fn reset` |
| `VertInf::Reset(const Point &)` | method | vertices.h:122 (decl)<br>vertices.cpp:213 | ported (comment) | vertices.rs:654 `fn reset_point` |
| `VertInf::removeFromGraph(const bool)` | method | vertices.h:123 (decl)<br>vertices.cpp:228 | ported (comment) | vertices.rs:670 `fn remove_from_graph` |
| `VertInf::orphaned()` | method | vertices.h:124 (decl)<br>vertices.cpp:222 | ported (comment) | vertices.rs:663 `fn orphaned` |
| `VertInf::pathLeadsBackTo(const VertInf *) const` | method | vertices.h:126 (decl)<br>vertices.cpp:357 | ported (comment) | vertices.rs:763 `fn path_leads_back_to` |
| `VertInf::setVisibleDirections(const ConnDirFlags)` | method | vertices.h:127 (decl)<br>vertices.cpp:320 | ported (comment) | vertices.rs:741 `fn set_visible_directions` |
| `VertInf::directionFrom(const VertInf *) const` | method | vertices.h:128 (decl)<br>vertices.cpp:290 | ported (comment) | vertices.rs:716 `fn direction_from` |
| `VertInf::hasNeighbour(VertInf *, bool) const` | method | vertices.h:130 (decl)<br>vertices.cpp:190 | ported (comment) | vertices.rs:628 `fn has_neighbour` |
| `VertInf::orphan()` | method | vertices.h:131 (decl)<br>vertices.cpp:262 | ported (comment) | vertices.rs:695 `fn orphan` |
| `VertInf::makeTreeRootPointer(VertInf *)` | method | vertices.h:133 (decl)<br>vertices.cpp:381 | ported (comment) | vertices.rs:791 `fn make_tree_root_pointer` |
| `VertInf::treeRoot() const` | method | vertices.h:134 (decl)<br>vertices.cpp:388 | ported (comment) | vertices.rs:808 `fn tree_root` |
| `VertInf::treeRootPointer() const` | method | vertices.h:135 (decl)<br>vertices.cpp:393 | ported (comment) | vertices.rs:819 `fn tree_root_pointer` |
| `VertInf::setTreeRootPointer(VertInf **)` | method | vertices.h:136 (decl)<br>vertices.cpp:403 | ported (comment) | vertices.rs:835 `fn set_tree_root_pointer` |
| `VertInf::clearTreeRootPointer()` | method | vertices.h:137 (decl)<br>vertices.cpp:398 | ported (comment) | vertices.rs:830 `fn clear_tree_root_pointer` |
| `VertInf::setSPTFRoot(VertInf *)` | method | vertices.h:139 (decl)<br>vertices.cpp:408 | ported (comment) | vertices.rs:843 `fn set_sptf_root` |
| `VertInf::sptfRoot() const` | method | vertices.h:140 (decl)<br>vertices.cpp:415 | ported (comment) | vertices.rs:852 `fn sptf_root` |
| `XL_EDGE` | constant | vertices.h:176 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `XL_CONN` | constant | vertices.h:177 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `XH_EDGE` | constant | vertices.h:178 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `XH_CONN` | constant | vertices.h:179 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `YL_EDGE` | constant | vertices.h:180 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `YL_CONN` | constant | vertices.h:181 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `YH_EDGE` | constant | vertices.h:182 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `YH_CONN` | constant | vertices.h:183 | ported (comment) | vertices.rs:196 `const XL_EDGE` |
| `directVis(VertInf *, VertInf *)` | function | vertices.h:186 (decl)<br>vertices.cpp:422 | ported (comment) | vertices.rs:904 `fn direct_vis` |
| `VertInfList` | class | vertices.h:194 | ported (comment) | vertices.rs:956 `struct VertInfList` |
| `VertInfList::VertInfList()` | ctor | vertices.h:197 (decl)<br>vertices.cpp:464 | ported (comment) | vertices.rs:968 `fn new` |
| `VertInfList::addVertex(VertInf *)` | method | vertices.h:198 (decl)<br>vertices.cpp:494 | ported (comment) | vertices.rs:1010 `fn add_vertex` |
| `VertInfList::removeVertex(VertInf *)` | method | vertices.h:199 (decl)<br>vertices.cpp:557 | ported (comment) | vertices.rs:1062 `fn remove_vertex` |
| `VertInfList::getVertexByID(const VertID &)` | method | vertices.h:200 (decl)<br>vertices.cpp:660 | ported (comment) | vertices.rs:1143 `fn get_vertex_by_id` |
| `VertInfList::getVertexByPos(const Point &)` | method | vertices.h:201 (decl)<br>vertices.cpp:688 | ported (comment) | vertices.rs:1167 `fn get_vertex_by_pos` |
| `VertInfList::shapesBegin()` | method | vertices.h:202 (decl)<br>vertices.cpp:702 | ported (comment) | vertices.rs:1181 `fn shapes_begin` |
| `VertInfList::connsBegin()` | method | vertices.h:203 (decl)<br>vertices.cpp:708 | ported (comment) | vertices.rs:1187 `fn conns_begin` |
| `VertInfList::end()` | method | vertices.h:204 (decl)<br>vertices.cpp:719 | ported (comment) | vertices.rs:1197 `fn end` |
| `VertInfList::connsSize() const` | method | vertices.h:205 (decl)<br>vertices.cpp:725 | ported (comment) | vertices.rs:1203 `fn conns_size` |
| `VertInfList::shapesSize() const` | method | vertices.h:206 (decl)<br>vertices.cpp:731 | ported (comment) | vertices.rs:1209 `fn shapes_size` |
| `ShapeSet` | typedef | vertices.h:217 | ported (comment) | vertices.rs:206 `type ShapeSet` |
| `ContainsMap` | typedef | vertices.h:218 | ported (comment) | vertices.rs:208 `type ContainsMap` |
| `checkVertInfListConditions()` | macro | vertices.cpp:475 | ported (comment) | vertices.rs:973 `fn check_vert_inf_list_conditions` |

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
| `vertexVisibility(VertInf *, VertInf *, bool, const bool)` | function | visibility.h:34 (decl)<br>visibility.cpp:115 | ported (comment) | visibility.rs:106 `fn vertex_visibility` |
| `vertexSweep(VertInf *)` | function (static) | visibility.cpp:50 (decl)<br>visibility.cpp:436 | ported (comment) | visibility.rs:445 `fn vertex_sweep` |
| `PointPair` | class | visibility.cpp:171 | ported (comment) | visibility.rs:162 `struct PointPair` |
| `PointPair::PointPair(const Point &, VertInf *)` | ctor | visibility.cpp:174 | ported (comment) | visibility.rs:176 `fn new` |
| `PointPair::operator<(const PointPair &) const` | operator | visibility.cpp:181 | ported (comment) | visibility.rs:191 `fn less` |
| `VertSet` | typedef | visibility.cpp:210 | ported (comment) | visibility.rs:214 `struct VertSet` |
| `EdgePair` | class | visibility.cpp:213 | ported (comment) | visibility.rs:233 `struct EdgePair` |
| `EdgePair::EdgePair()` | ctor | visibility.cpp:216 | not-ported: subsumed | default ctor that only asserts it is never called (MSVC appeasement) |
| `EdgePair::EdgePair(const PointPair &, VertInf *)` | ctor | visibility.cpp:228 | ported (comment) | visibility.rs:251 `fn new` |
| `EdgePair::operator<(const EdgePair &) const` | operator | visibility.cpp:238 | ported (comment) | visibility.rs:269 `fn less` |
| `EdgePair::operator==(const EdgePair &) const` | operator | visibility.cpp:247 | ported (comment) | visibility.rs:278 `fn equals` |
| `EdgePair::operator!=(const EdgePair &) const` | operator | visibility.cpp:258 | ported (comment) | visibility.rs:288 `fn not_equals` |
| `EdgePair::setNegativeAngle()` | method | visibility.cpp:269 | ported (comment) | visibility.rs:298 `fn set_negative_angle` |
| `EdgePair::setCurrAngle(const PointPair &)` | method | visibility.cpp:273 | ported (comment) | visibility.rs:303 `fn set_curr_angle` |
| `SweepEdgeList` | typedef | visibility.cpp:316 | ported (comment) | visibility.rs:337 `type SweepEdgeList` |
| `AHEAD` | define | visibility.cpp:319 | ported (comment) | visibility.rs:351 `const AHEAD` |
| `BEHIND` | define | visibility.cpp:320 | ported (comment) | visibility.rs:351 `const AHEAD` |
| `isBoundingShape` | class | visibility.cpp:322 | ported (comment) | visibility.rs:355 `struct IsBoundingShape` |
| `isBoundingShape::isBoundingShape(ShapeSet &)` | ctor | visibility.cpp:326 | not-ported: subsumed | stores the ShapeSet; the ported predicate takes it as an argument (visibility.rs) |
| `isBoundingShape::operator()(const PointPair &)` | operator | visibility.cpp:330 | ported (comment) | visibility.rs:361 `fn call` |
| `isBoundingShape::operator=(isBoundingShape const &)` | operator | visibility.cpp:343 (decl) | not-ported: declared-undefined | private, never defined (MSVC workaround) |
| `isBoundingShape::isBoundingShape()` | ctor | visibility.cpp:344 (decl) | not-ported: declared-undefined | private default ctor, never defined (MSVC workaround) |
| `sweepVisible(SweepEdgeList &, const PointPair &, std::set<unsigned int> &, int *)` | function (static) | visibility.cpp:350 | ported (comment) | visibility.rs:370 `fn sweep_visible` |

<a id="vpsc"></a>
### vpsc

| Upstream item | Kind | Upstream | Status | Rust location / reason |
| --- | --- | --- | --- | --- |
| `Variables` | typedef | vpsc.h:82 | not-ported: subsumed | std::vector&lt;Variable*&gt;; Rust Vec&lt;Variable&gt; (vpsc.rs) |
| `Constraints` | typedef | vpsc.h:83<br>vpsc.h:161 | not-ported: subsumed | std::vector&lt;Constraint*&gt;; Rust Vec of constraint indices (vpsc.rs) |
| `CompareConstraints` | class | vpsc.h:84 | not-ported: subsumed | comparator class; its operator() is ported (vpsc.rs) |
| `CompareConstraints::operator()(Constraint * const &, Constraint * const &) const` | operator | vpsc.h:86 (decl)<br>vpsc.cpp:1366 | ported (comment) | vpsc.rs:270 `fn compare_constraints` |
| `PositionStats` | struct | vpsc.h:88 | ported (comment) | vpsc.rs:161 `struct PositionStats` |
| `PositionStats::PositionStats()` | ctor | vpsc.h:89 | not-ported: subsumed | zero-initialisation; PositionStats derives Default (vpsc.rs) |
| `PositionStats::addVariable(Variable *)` | method | vpsc.h:90 (decl)<br>vpsc.cpp:688 | ported (comment) | vpsc.rs:171 `fn add_variable` |
| `Heap` | typedef | vpsc.h:97 | not-ported: subsumed | std::priority_queue; heap reimplemented with libstdc++ semantics (vpsc.rs module docs "Heaps") |
| `Block` | class | vpsc.h:100 | ported (comment) | vpsc.rs:182 `struct Block` |
| `Block::Vit` | typedef | vpsc.h:102 | not-ported: subsumed | iterator typedef |
| `Block::Cit` | typedef | vpsc.h:103 | not-ported: subsumed | iterator typedef |
| `Block::Cit_const` | typedef | vpsc.h:104 | not-ported: subsumed | iterator typedef |
| `operator<<(ostream &, const Block &)` | operator (friend) | vpsc.h:106 (decl)<br>vpsc.cpp:1281 | not-ported: debug-output | ostream debug printing of Block/Variable/Constraint |
| `Block::Block(Blocks *, Variable * const)` | ctor | vpsc.h:113 (decl)<br>vpsc.cpp:721 | ported (comment) | vpsc.rs:953 `fn block_new` |
| `Block::~Block()` | dtor | vpsc.h:114 (decl)<br>vpsc.cpp:752 | not-ported: subsumed | deletes member containers; Rust drop |
| `Block::findMinLM()` | method | vpsc.h:115 (decl)<br>vpsc.cpp:1135 | ported (comment) | vpsc.rs:1305 `fn block_find_min_lm` |
| `Block::findMinLMBetween(Variable * const, Variable * const)` | method | vpsc.h:116 (decl)<br>vpsc.cpp:1146 | ported (comment) | vpsc.rs:1316 `fn block_find_min_lm_between` |
| `Block::findMinInConstraint()` | method | vpsc.h:117 (decl)<br>vpsc.cpp:864 | ported (comment) | vpsc.rs:1086 `fn block_find_min_in_constraint` |
| `Block::findMinOutConstraint()` | method | vpsc.h:118 (decl)<br>vpsc.cpp:913 | ported (comment) | vpsc.rs:1116 `fn block_find_min_out_constraint` |
| `Block::deleteMinInConstraint()` | method | vpsc.h:119 (decl)<br>vpsc.cpp:923 | ported (comment) | vpsc.rs:1132 `fn block_delete_min_in_constraint` |
| `Block::deleteMinOutConstraint()` | method | vpsc.h:120 (decl)<br>vpsc.cpp:931 | ported (comment) | vpsc.rs:1137 `fn block_delete_min_out_constraint` |
| `Block::updateWeightedPosition()` | method | vpsc.h:121 (decl)<br>vpsc.cpp:738 | ported (comment) | vpsc.rs:985 `fn block_update_weighted_position` |
| `Block::merge(Block *, Constraint *, double)` | method | vpsc.h:122 (decl)<br>vpsc.cpp:807 | ported (comment) | vpsc.rs:1046 `fn block_merge_dist` |
| `Block::merge(Block *, Constraint *)` | method | vpsc.h:123 (decl)<br>vpsc.cpp:781 | ported (comment) | vpsc.rs:1028 `fn block_merge` |
| `Block::mergeIn(Block *)` | method | vpsc.h:124 (decl)<br>vpsc.cpp:838 | ported (comment) | vpsc.rs:1062 `fn block_merge_in` |
| `Block::mergeOut(Block *)` | method | vpsc.h:125 (decl)<br>vpsc.cpp:855 | ported (comment) | vpsc.rs:1075 `fn block_merge_out` |
| `Block::split(Block *&, Block *&, Constraint *)` | method | vpsc.h:126 (decl)<br>vpsc.cpp:1259 | ported (comment) | vpsc.rs:1458 `fn block_split` |
| `Block::splitBetween(Variable * const, Variable * const, Block *&, Block *&)` | method | vpsc.h:127 (decl)<br>vpsc.cpp:1237 | ported (comment) | vpsc.rs:1440 `fn block_split_between` |
| `Block::setUpInConstraints()` | method | vpsc.h:128 (decl)<br>vpsc.cpp:758 | ported (comment) | vpsc.rs:998 `fn block_set_up_in_constraints` |
| `Block::setUpOutConstraints()` | method | vpsc.h:129 (decl)<br>vpsc.cpp:761 | ported (comment) | vpsc.rs:1003 `fn block_set_up_out_constraints` |
| `Block::cost()` | method | vpsc.h:130 (decl)<br>vpsc.cpp:1273 | ported (comment) | vpsc.rs:1472 `fn block_cost` |
| `Block::getActivePathBetween(Constraints &, Variable const *, Variable const *, Variable const *) const` | method | vpsc.h:135 (decl)<br>vpsc.cpp:1183 | ported (comment) | vpsc.rs:1368 `fn block_get_active_path_between` |
| `Block::isActiveDirectedPathBetween(Variable const *, Variable const *) const` | method | vpsc.h:137 (decl)<br>vpsc.cpp:1207 | ported (comment) | vpsc.rs:1401 `fn block_is_active_directed_path_between` |
| `Block::getActiveDirectedPathBetween(Constraints &, Variable const *, Variable const *) const` | method | vpsc.h:139 (decl)<br>vpsc.cpp:1218 | ported (comment) | vpsc.rs:1418 `fn block_get_active_directed_path_between` |
| `Block::Direction: NONE, LEFT, RIGHT` | enum | vpsc.h:141 | not-ported: dead-code | only used by the commented-out Block::compute_dfdv_between() |
| `Block::Pair` | typedef | vpsc.h:142 | not-ported: dead-code | only used by the commented-out Block::compute_dfdv_between() |
| `Block::reset_active_lm(Variable * const, Variable * const)` | method | vpsc.h:143 (decl)<br>vpsc.cpp:1093 | ported (comment) | vpsc.rs:1277 `fn block_reset_active_lm` |
| `Block::list_active(Variable * const, Variable * const)` | method | vpsc.h:144 (decl)<br>vpsc.cpp:1109 | ported (comment) | vpsc.rs:1301 `fn block_find_min_lm` |
| `Block::compute_dfdv(Variable * const, Variable * const)` | method | vpsc.h:145 (decl)<br>vpsc.cpp:967 | ported (comment) | vpsc.rs:1200 `fn block_compute_dfdv` |
| `Block::compute_dfdv(Variable * const, Variable * const, Constraint *&)` | method | vpsc.h:146 (decl)<br>vpsc.cpp:946 | ported (comment) | vpsc.rs:1154 `fn block_compute_dfdv_min` |
| `Block::split_path(Variable *, Variable * const, Variable * const, Constraint *&, bool)` | method | vpsc.h:147 (decl)<br>vpsc.cpp:995 | ported (comment) | vpsc.rs:1226 `fn block_split_path` |
| `Block::canFollowLeft(Constraint const *, Variable const *) const` | method (inline) | vpsc.h:149 (decl)<br>vpsc.cpp:934 | ported (comment) | vpsc.rs:1142 `fn can_follow_left` |
| `Block::canFollowRight(Constraint const *, Variable const *) const` | method (inline) | vpsc.h:150 (decl)<br>vpsc.cpp:937 | ported (comment) | vpsc.rs:1148 `fn can_follow_right` |
| `Block::populateSplitBlock(Block *, Variable *, Variable const *)` | method | vpsc.h:151 (decl)<br>vpsc.cpp:1169 | ported (comment) | vpsc.rs:1338 `fn block_populate_split_block` |
| `Block::addVariable(Variable *)` | method | vpsc.h:152 (decl)<br>vpsc.cpp:704 | ported (comment) | vpsc.rs:972 `fn block_add_variable` |
| `Block::setUpConstraintHeap(Heap *&, bool)` | method | vpsc.h:153 (decl)<br>vpsc.cpp:764 | ported (comment) | vpsc.rs:1008 `fn block_set_up_constraint_heap` |
| `Variable` | class | vpsc.h:162 | ported (comment) | vpsc.rs:66 `struct Variable` |
| `operator<<(std::ostream &, const Variable &)` | operator (friend) | vpsc.h:164 (decl)<br>vpsc.cpp:1389 | not-ported: debug-output | ostream debug printing of Block/Variable/Constraint |
| `Variable::Variable(const int, const double, const double, const double)` | ctor (inline) | vpsc.h:181 | ported (comment) | vpsc.rs:90 `fn new` |
| `Variable::dfdv() const` | method | vpsc.h:193 | ported (comment) | vpsc.rs:249 `fn dfdv_of`<br>vpsc.rs:749 `fn dfdv` |
| `Variable::position() const` | method (inline) | vpsc.h:197 | ported (comment) | vpsc.rs:233 `fn position_of`<br>vpsc.rs:744 `fn position` |
| `Variable::unscaledPosition() const` | method (inline) | vpsc.h:200 | ported (comment) | vpsc.rs:240 `fn unscaled_position_of` |
| `Constraint` | class | vpsc.h:208 | ported (comment) | vpsc.rs:119 `struct Constraint` |
| `Constraint::Constraint(Variable *, Variable *, double, bool)` | ctor | vpsc.h:211 (decl)<br>vpsc.cpp:1293 | ported (comment) | vpsc.rs:139 `fn new` |
| `Constraint::~Constraint()` | dtor | vpsc.h:212 (decl)<br>vpsc.cpp:1309 | not-ported: subsumed | empty destructor (body commented out) |
| `Constraint::slack() const` | method (inline) | vpsc.h:213 | ported (comment) | vpsc.rs:254 `fn slack_of`<br>vpsc.rs:739 `fn slack` |
| `Constraint::toString() const` | method | vpsc.h:228 (decl)<br>vpsc.cpp:1323 | not-ported: debug-output | debug string formatting |
| `operator<<(std::ostream &, const Constraint &)` | operator (friend) | vpsc.h:230 (decl)<br>vpsc.cpp:1340 | not-ported: debug-output | ostream debug printing of Block/Variable/Constraint |
| `Blocks` | class | vpsc.h:247 | ported (comment) | vpsc.rs:196 `struct Blocks` |
| `Blocks::Blocks(Variables const &)` | ctor | vpsc.h:250 (decl)<br>vpsc.cpp:491 | ported (comment) | vpsc.rs:809 `fn blocks_new` |
| `Blocks::~Blocks()` | dtor | vpsc.h:251 (decl)<br>vpsc.cpp:498 | not-ported: subsumed | deletes blocks and resets blockTimeCtr of the dying object; Rust drop |
| `Blocks::mergeLeft(Block *)` | method | vpsc.h:252 (decl)<br>vpsc.cpp:546 | ported (comment) | vpsc.rs:850 `fn blocks_merge_left` |
| `Blocks::mergeRight(Block *)` | method | vpsc.h:253 (decl)<br>vpsc.cpp:580 | ported (comment) | vpsc.rs:883 `fn blocks_merge_right` |
| `Blocks::split(Block *, Block *&, Block *&, Constraint *)` | method | vpsc.h:254 (decl)<br>vpsc.cpp:651 | ported (comment) | vpsc.rs:920 `fn blocks_split` |
| `Blocks::totalOrder()` | method | vpsc.h:255 (decl)<br>vpsc.cpp:513 | ported (comment) | vpsc.rs:820 `fn blocks_total_order` |
| `Blocks::cleanup()` | method | vpsc.h:256 (decl)<br>vpsc.cpp:614 | ported (comment) | vpsc.rs:913 `fn blocks_cleanup` |
| `Blocks::cost()` | method | vpsc.h:257 (decl)<br>vpsc.cpp:678 | ported (comment) | vpsc.rs:940 `fn blocks_cost` |
| `Blocks::size() const` | method (inline) | vpsc.h:259 (decl)<br>vpsc.h:273 | not-ported: subsumed | inline `m_blocks.size()`; Rust uses bs.m_blocks.len() directly |
| `Blocks::at(size_t) const` | method (inline) | vpsc.h:260 (decl)<br>vpsc.h:278 | not-ported: subsumed | inline `m_blocks[index]`; Rust indexes bs.m_blocks directly |
| `Blocks::insert(Block *)` | method (inline) | vpsc.h:261 (decl)<br>vpsc.h:283 | not-ported: subsumed | inline `m_blocks.push_back()`; Rust pushes to bs.m_blocks directly |
| `Blocks::dfsVisit(Variable *, list<Variable *> *)` | method | vpsc.h:265 (decl)<br>vpsc.cpp:527 | ported (comment) | vpsc.rs:836 `fn blocks_dfs_visit` |
| `Blocks::removeBlock(Block *)` | method | vpsc.h:266 (decl)<br>vpsc.cpp:608 | ported (comment) | vpsc.rs:908 `fn blocks_remove_block` |
| `UnsatisfiableException` | struct | vpsc.h:288 | ported (comment) | vpsc.rs:208 `struct UnsatisfiableException` |
| `UnsatisfiedConstraint` | struct | vpsc.h:291 | ported (name) | vpsc.rs:217 |
| `UnsatisfiedConstraint::UnsatisfiedConstraint(Constraint &)` | ctor | vpsc.h:292 | not-ported: subsumed | stores the constraint; Rust struct literal (vpsc.rs UnsatisfiedConstraint) |
| `IncSolver` | class | vpsc.h:298 | ported (comment) | vpsc.rs:381 `struct IncSolver` |
| `IncSolver::satisfy()` | method | vpsc.h:301 (decl)<br>vpsc.cpp:274 | ported (comment) | vpsc.rs:592 `fn satisfy` |
| `IncSolver::solve()` | method | vpsc.h:302 (decl)<br>vpsc.cpp:243 | ported (comment) | vpsc.rs:575 `fn solve` |
| `IncSolver::moveBlocks()` | method | vpsc.h:303 (decl)<br>vpsc.cpp:367 | ported (comment) | vpsc.rs:662 `fn move_blocks` |
| `IncSolver::splitBlocks()` | method | vpsc.h:304 (decl)<br>vpsc.cpp:383 | ported (comment) | vpsc.rs:671 `fn split_blocks` |
| `IncSolver::IncSolver(Variables const &, Constraints const &)` | ctor | vpsc.h:305 (decl)<br>vpsc.cpp:61 | ported (comment) | vpsc.rs:403 `fn new` |
| `IncSolver::~IncSolver()` | dtor | vpsc.h:307 (decl)<br>vpsc.cpp:92 | not-ported: subsumed | deletes the Blocks; Rust ownership drops them |
| `IncSolver::addConstraint(Constraint *)` | method | vpsc.h:308 (decl)<br>vpsc.cpp:96 | ported (comment) | vpsc.rs:441 `fn add_constraint` |
| `IncSolver::getVariables()` | method | vpsc.h:309 | ported (comment) | vpsc.rs:459 `fn get_variables` |
| `IncSolver::printBlocks()` | method | vpsc.h:318 (decl)<br>vpsc.cpp:107 | not-ported: debug-output | LIBVPSC_LOGGING-only (body empty otherwise) |
| `IncSolver::copyResult()` | method | vpsc.h:319 (decl)<br>vpsc.cpp:124 | ported (comment) | vpsc.rs:469 `fn copy_result` |
| `IncSolver::constraintGraphIsCyclic(const unsigned, Variable * const)` | method | vpsc.h:321 (decl)<br>vpsc.cpp:137 | ported (comment) | vpsc.rs:478 `fn constraint_graph_is_cyclic` |
| `IncSolver::blockGraphIsCyclic()` | method | vpsc.h:322 (decl)<br>vpsc.cpp:184 | ported (comment) | vpsc.rs:522 `fn block_graph_is_cyclic` |
| `IncSolver::mostViolated(Constraints &)` | method | vpsc.h:325 (decl)<br>vpsc.cpp:431 | ported (comment) | vpsc.rs:700 `fn most_violated` |
| `delete_object` | struct | vpsc.h:328 | not-ported: subsumed | `delete ptr` functor for std::for_each; Rust drop |
| `delete_object::operator()(T *)` | operator (template) | vpsc.h:331 | not-ported: subsumed | `delete ptr` functor for std::for_each; Rust drop |
| `constraintsRemovingRedundantEqualities(Variables const &, Constraints const &)` | function | vpsc.h:334 (decl)<br>vpsc.cpp:1468 | ported (comment) | vpsc.rs:1559 `fn constraints_removing_redundant_equalities` |
| `ZERO_UPPERBOUND` | constant | vpsc.cpp:57 | ported (comment) | vpsc.rs:50 `const ZERO_UPPERBOUND` |
| `LAGRANGIAN_TOLERANCE` | constant | vpsc.cpp:58 | ported (comment) | vpsc.rs:52 `const LAGRANGIAN_TOLERANCE` |
| `node` | struct | vpsc.cpp:132 | not-ported: subsumed | helper struct of constraintGraphIsCyclic; Rust uses index maps (vpsc.rs) |
| `__NOTNAN(p)` | macro | vpsc.cpp:488 | not-ported: subsumed | NaN self-compare inside COLA_ASSERT; `debug_assert!(!x.is_nan())` in vpsc.rs |
| `VarOffsetMapList` | typedef | vpsc.cpp:1397 | not-ported: subsumed | std::list&lt;std::map&lt;Variable*,double&gt;&gt;; EqualityConstraintSet::variable_groups is Vec&lt;BTreeMap&lt;VarIdx, f64&gt;&gt; (vpsc.rs) |
| `EqualityConstraintSet` | class | vpsc.cpp:1399 | ported (comment) | vpsc.rs:1489 `struct EqualityConstraintSet` |
| `EqualityConstraintSet::EqualityConstraintSet(Variables)` | ctor | vpsc.cpp:1402 | ported (comment) | vpsc.rs:1499 `fn new` |
| `EqualityConstraintSet::isRedundant(Variable *, Variable *, double)` | method | vpsc.cpp:1411 | ported (comment) | vpsc.rs:1510 `fn is_redundant` |
| `EqualityConstraintSet::mergeSets(Variable *, Variable *, double)` | method | vpsc.cpp:1426 | ported (comment) | vpsc.rs:1526 `fn merge_sets` |
| `EqualityConstraintSet::setForVar(Variable *)` | method | vpsc.cpp:1452 | ported (comment) | vpsc.rs:1553 `fn set_for_var` |

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
