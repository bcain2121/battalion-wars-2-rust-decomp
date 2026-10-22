# Destruction: mutable replacement collision scenes

## What this unlocks

Previously `unit_collision::Scene` could only accumulate anonymous meshes. There was no way to remove one object's geometry after destruction. It now supports ownership-scoped node registration/removal, immediately affecting its real movement, overlap and line-of-sight queries.

This is a **replacement collision service**, not a recovered native spatial algorithm and not an installed Lua Kill backend. The original shared startup remains blocked. The original-scene diagnostic loader now retains source node indices and registers complete object batches. Its allocation handles are an explicit viewer policy (placement-order tokens for one scene lifetime), not recovered native pointers or authored-ID casts. Geometry remains render-derived and activation rules remain unapplied; this is not a native mission collision scene.

## Contract

- `register_object_meshes` prepares a complete batch before publishing it, without cloning the growing scene per placement. Duplicate objects, owner aliases, repeated nodes or invalid geometry cannot partially register an object. `collision_owner` resolves the authored ID to its binding in that snapshot; it is not general object liveness.
- Empty object batches are explicitly known empty, unlike unknown owners. Full removal clears their collision binding; partial retention keeps it.
- `ColliderKey` combines a live allocation handle and a node index. Authored object IDs alone are not runtime identity.
- `add_owned_mesh` registers one complete node's geometry. Duplicate keys and invalid geometry fail without reserving a key or changing the scene.
- Empty/degenerate nodes are registered explicitly, distinguishing known empty geometry from unknown ownership.
- `remove_owned_meshes(owner, retained_nodes)` validates the entire request first. Unknown owners, unknown retained nodes and duplicates fail without mutation.
- Callers must supply the **actual retained node identities**. `mNumLeftBehindCollisionNodes=1` does not justify retaining an arbitrary first node, nor clearing all geometry.
- Other owners and anonymous/static meshes remain untouched. The return count measures nonempty meshes removed, not node registrations.
- Scene clones share immutable triangle buffers and ground data, but copy the ownership/mesh tables. Staged mutation through `Arc::make_mut` cannot alter a previous scene snapshot.

## Verified behavior

`tests/collision_lifecycle.rs` exercises the actual query engine, not a recording backend:

1. A wall blocks a cylinder movement, overlaps the cylinder and blocks a sight ray.
2. Removing its owner/node makes all three queries clear.
3. A retained node, another owner's wall and an anonymous wall still block.
4. Invalid removal/registration requests leave the original scene unchanged.
5. Editing and discarding a cloned scene leaves the original obstacle intact.
6. Empty nodes remain known; rejected nonfinite geometry does not consume their keys.

`tests/collision_registration.rs` additionally checks atomic batch registration, binding lifetime/snapshot isolation and flat-versus-node-grouped movement/LOS equivalence across 75 movement/ray cases.

The offscreen original-scene script preview loaded **1007 owned objects / 3326 nodes**, with movement/navigation smoke results `overlap=false`, one arrival and zero navigation failures. The replacement battle loaded 827 objects / 3146 nodes and passed its restart regression. Logs: `local/analysis/collision-registration-{viewer,battle}.log`. The existing missing VGLASSopaque texture/model omission remains reported, not silently resolved.

These are regression checks, not original mission acceptance.

## Native recovery and remaining integration

The concrete Kill spatial call reaches `0x80237a50`, then `0x8023d13c`. The latter performs a spatial query, calls `0x80239f8c`, sets object byte +0x75 and conditionally marks queried neighbors. The lower path traverses model nodes and dispatches on node kinds. These additional effects and the exact role of the native spatial structures are not fully recovered; do not equate the new mesh-removal service with that entire path.

Follow-up [NATIVE_NODE_KINDS.md](NATIVE_NODE_KINDS.md) establishes that Art's source carrier has no nodes handled by this volume dispatcher. Surrounding spatial notifications still apply; do not invent a general keep-first-N debris rule as an Art prerequisite.

The notification control flow and concrete destroyable predicates are now isolated in `mission_spatial_removal`, with rollback tests; see [NATIVE_SPATIAL_REMOVAL.md](NATIVE_SPATIAL_REMOVAL.md). This still requires live broadphase/state providers and does not install Kill.

Next: connect the actual Art spatial side effects and effect lifetime; recover broader retention/registration policy where required by verified consumers; construct a complete native-selected collision scene rather than the diagnostic render-derived one; populate mission-session ownership; integrate scene mutation with the destruction transaction. Camera obstruction uses a separate scene and must also be addressed. Native explosion creation/presentation and concrete resource destructors remain required. No fake success is returned for those missing capabilities.

New private trace files: `local/analysis/destruction-collision-node-dispatch-native.txt` and `local/analysis/destruction-spatial-volume-native.txt`. Original data remains ignored.
