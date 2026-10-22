# Native spatial-removal notification control flow

`src/mission_spatial_removal.rs` ports the ordering of `0x8023d13c` and query callback `0x8023d654`. The preceding `0x802322ec` selection condition is now recovered, but its selected `0x8023172c` operation is not implemented. Native broadphase, model volume updates, live state population and a production destruction backend are still missing. No Lua Kill binding is installed.

## Recovered sequence

1. Known null model at object +0x7c returns without query or byte writes.
2. Query uses mask `0xa0400008`, mode `3`, object +0x60 coordinates and the model resource's sphere radius. Backend must provide actual native geometry and traversal order.
3. Callback extracts the candidate owner from query record +0x20. Self is skipped.
4. Virtual +0x38 must accept RTTI descriptor `0x805132e8` (**cWorldObject**).
5. If virtual +0x78 is true, skip candidates whose main flags (+0x14) intersect `0x00900000`. Helper `0x80064110` returns true exactly when those bits are clear. This is not a health test; `0x00080000` is not part of this mask.
6. Collect at most 30 eligible candidates, preserving order and duplicates. Traversal stops at the **31st eligible candidate**, not immediately when the 30th is stored. Rejected candidates do not stop traversal. The later +0x1a4 test does not reclaim capacity.
7. Execute `0x80239f8c(owner, 1)` model-volume path.
8. Write literal 1 to owner's raw byte +0x75.
9. In collection order, invoke each candidate's virtual +0x1a4; if true, write its byte +0x75 to 1. No visibility/health meaning is invented for this byte.

The Rust backend uses a pull query interface so filtering and candidate retrieval remain interleaved and no candidate after the stop point is read. Query completion and every required effect are explicit methods. All mutable backend state is staged; any unsupported operation/error leaves the original backend unchanged. Providers must deep-clone mutable state and perform no external I/O during the transaction.

## Verified concrete destroyable predicates

For the verified main vtable `0x804aeb90` only:

- +0x78 → `0x800cf454`: model exists and model-instance flags (+0x6c) contain `0x01000000`.
- +0x104 → `0x8000d45c`: returns the model pointer.
- +0x1a4 → `0x800a1290`: false without a model or model flag `0x00400000`. Otherwise true if main flags `0x00900000` are clear. On the remaining branch, true if the explosion profile's flags contain 4 and base float +0x78 is not greater than zero; otherwise tests base word +0xa8 for nonzero.
- +0x7c → `0x800a091c`: base float +0x78 > 0.
- +0x290 → `0x800a1890`: base word +0xa8.

These are ported as `destroyable_virtual_78` and `destroyable_virtual_1a4`, with explicit raw inputs and an error when a required profile is missing. Known-null model/explosion pointers are distinct from unconfigured data. Native reflection now identifies base +0x78 as `mDeadModelDecayTime`, +0xa8 as the `DeadModel` resource pointer, and +0xb0 as `CollideType`. `SpatialProfile` strictly reads these authored fields and binds the known-null-dead-model subset to the predicate, resolving explosion flags from a checked `sExplodeBase`. Non-null dead models still require a live resource bank and explicitly fail this binding. Unverified derived vtables cannot use these helpers automatically.

**Do not confuse +0x290/base +0xa8 with adjacent +0x28c/base +0x9c (`mNumLeftBehindCollisionNodes`).** This does not establish the latter's consumer or any retain-first-N policy.

## Pre-query condition and actual Art source bindings

`0x802322ec` first checks cDestroyableObject RTTI `0x8050d970` (registration string `0x80487e48`). It calls `0x8023172c(manager, object, 1)` only if base `CollideType & 8` is nonzero OR object +0x12c has bit `0x800`. These are different from the main-object dead flags and from `ObjFlags` at base +0xb4.

Reflection at `0x800a3900` identifies object +0x12c as **mInstanceFlags**. The three original Art objects all author this as 0; base 250009278 has CollideType 0, DeadModel 0, decay time 0 and explosion flags 16. The gate is therefore false **at those imported values**, and their concrete dead-neighbor predicate is false regardless of model flags. `authored_instance_flags` and `SpatialProfile` load and test these real fields rather than deriving them from a constructor default.

An imported snapshot is NOT proof of all subsequent runtime changes. `pre_query_update_required(None)` still rejects missing live flags when that branch reads them. The source audit reports both the authored-state result and the absent live backend. The direct-store scan found 73 unclassified +0x12c stores; it is not a proof about object types, aliases, or post-load invariance.

Reflection traces: `destruction-base-a8-b0-reflection.txt` (base +0xa8/+0xb0), `destruction-base-fields-native.txt` (base +0x78), `destruction-full-field-registration.txt` (instance +0x12c), and `destruction-pre-query-operation.txt`. Native name strings are at `0x804af228` (DeadModel), `0x804af21c` (CollideType), `0x804a00b4` (mDeadModelDecayTime), and `0x804aeed8` (mInstanceFlags).

## Evidence and checks

RTTI registration at `0x800cf854..0x800cf86c` constructs descriptor `0x805132e8` using string `0x80488c70`, `cWorldObject`.

Private traces: `destruction-spatial-notify-native.txt`, `destruction-spatial-query-callback-native.txt`, `destruction-query-helper-native.txt`, `destruction-query-*-destroyable.txt`, `destruction-neighbor-slots.json` and `destruction-neighbor-slot-*.txt` under `local/analysis/`.

Four Rust tests cover short-circuit reads, both excluded bits versus deferred-delete, duplicate/order retention, capacity timing, final-predicate capacity behavior, concrete model/profile predicates, known-null early return and failure injection at every operation in empty, duplicate and over-capacity cases.

Three additional spatial-profile tests check strict parsing, missing/duplicate fields, unresolved resources, the live-state gate and original Art bindings. Validation: 171 viewer library + 81 selected integration + 15 Python tests = 267 passes; formatting and all-target viewer Clippy pass. A Clippy simplification warning was corrected; its initial log is retained. One malformed Cargo test option was rejected before dispatch and corrected. Full selected integration checks subsequently passed.

## Remaining integration

The source Art model's zero eligible volume nodes do not erase this surrounding query/notification work. Still required: the selected pre-query operation where required, native query membership/order, live model flags and instance-state tracking, non-null dead-model resource binding, actual volume operations where applicable, record/auxiliary/attachment cleanup, and explosion/light lifecycle. The shared startup audit still fails transactionally at ArtInitialisation PC3 / Kill, frame zero.
