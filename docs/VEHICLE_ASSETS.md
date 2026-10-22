# Ground-vehicle profile and physics-node recovery

The original first mission remains the acceptance target. This resolves asset/profile questions; it does **not** enable vehicle dynamics or advance the intro past its movement blocker.

## Native selection evidence

RBWE01 main.dol SHA-256: `9e557a6c9d316de3a3db8e9790b8d8c54ffc9cd1656dee89a90a2d7f964df87f`.

- Enum registration at `0x80071134..0x80071190`: `physEngODE=0`, `physEngAgeia=1`, `physEngMax=2`. Strings are at `0x804865e8`, `0x804865f4`, `0x80486604`; registration passes explicit integer values, not inferred enumeration order.
- Ground-vehicle-base reflection at `0x80062d0c..0x80062dbc`: `mPhysicsBase` offset `0x440`, `mPhysicsBaseNewEngine` offset `0x444`. The adjacent type names are `cGroundVehiclePhysicsBase`. The similarly named water-vehicle properties have different offsets and are not used here.
- Getter `0x80061d1c`: argument 1 selects `+0x444`; otherwise selects `+0x440`.
- Vehicle reflection at `0x80071414`: `mPhysEngine` offset `0x644`.
- Ground-vehicle initialization `0x8006243c..0x800624d8` obtains the override at current level-entry `+0x1d4`; sentinel 2 preserves the instance engine, otherwise replaces it. The engine is passed into base selection and physics construction (`0x801e7f38`).
- **Correction:** the earlier constructor-default claim from `0x8001f898/0x8001f970` was based on an offset match without establishing object identity. Reflection at `0x80022ed0` names that class's +0x1d4 field `mBloomBlurFilter`, not the physics override. Those stores do not prove the live physics default. The Rust loader requires an explicit override policy. The audit now obtains that policy from the original level catalog, rather than assuming authored selection.

Local traces: `local/analysis/physics-{engine-enum-native,override-default,override-context,selection-800624a0,base-getter-80061d24}.txt` and `vehicle-{physics-reflection-a,physics-reflection-b,engine-reflection}.txt`.

`src/vehicle_profile.rs` validates selected object kinds, references, unique scalar fields, finite nonnegative parameters, positive movement limits, model identity and reflected-parameter references. Profiles are shared by source identity, not copied per placement. It preserves turn values without claiming a recovered turn integrator or angular-unit conversion.

**Actual SP_0.1 catalog selection is Ageia.** `Data/levels.xml` explicitly supplies `physics="1"`, overriding all 30 authored ODE unit settings. The current audit resolves **four** new-engine profiles: 50034495, 50034654, 50034658 and 550041986. Unit 550040155 selects 50034654 (attack 11, patrol 8), not old-engine 2138048309 (attack/patrol 10). The three old-engine profiles remain valid only under the explicit authored-policy fixture.

`src/level_catalog.rs` preserves all 77 catalog entries in order, including duplicate unused names; unique lookup rejects ambiguity. Native physics attribute lookup `0x80189844..74` uses fallback string `"2"` at `0x806022e4`. Constructor `0x80186ac4` receives the parsed value in r9 and stores it at +0x1d4 (`0x80186d4c`). Current-level selection `0x80011404..14` retrieves a catalog entry and stores it at global-state +0x50. The same entry supplies both profile selection and manager startup. Tests validate the original catalog and selected profiles, not an operational Ageia solver.

## Authored shape metadata—not render-triangle hulls

The three models contain **11 PHYSICS_COLLIDE nodes**:

- VSHTNKH: 4
- VUBSTAH: 3
- VUHTNKH: 4

Each node has only **one marker triangle**. Treating those triangles as closed collision hulls would be incorrect. Their separate `XOBB` chunk stores six little-endian float bounds. `src/model.rs` now retains that metadata, rejecting malformed lengths, duplicates and nonfinite numbers. General model metadata retains inverted bounds verbatim; the physics-node consumer rejects them.

`src/model_geometry.rs` returns node identity/parent, exact local bounds, all eight bind-space box corners and marker triangles separately. It preserves articulated hierarchy rather than flattening it into guessed chassis dimensions. It does **not** claim that every recovered box has been matched to its native ODE shape-creation call. Inertia-box parameters are not used as collider dimensions.

The original renderer now uses the same bounded bind-transform implementation, including finite/quaternion/parent/cycle checks. Vertex scale remains applied exactly once in the existing binary decoder. BBOX values are retained separately and not multiplied by vertex quantization scale.

The decoder also retains `Model::header_sphere`, four big-endian authored header floats, separately from node XOBB and body groups. Native loader copies them into model metadata +0..+0xc; they must not be substituted for physical chassis shapes or for the model matrix translation used in light scoring. See [MISSION_EFFECTS.md](MISSION_EFFECTS.md) for this later recovery and native traces.

Source-format cross-check: the already recorded MIT editor reference, commit `c2b074cfac58af6e751c8cb7e9713fa6b7b795e5`, reads BBOX as six floats. No external source assets are added to the repository.

## Wheel descriptors and ordered hierarchy

`src/vehicle_wheels.rs` preserves 22 slots (12 wheels, 10 interpolation roles), including holes, original role IDs, descriptor offsets and the A_S fallback only when ordinary A is absent. Native tables are `0x804aa0d0` and `0x804aa0dc`; assembly starts at `0x80068b04` / `0x80068d1c`. Exact duplicates and noncanonical indexed/case-folded/substring aliases are rejected instead of silently selecting or dropping a native role. Native name classification performs exact matching then longest substring matching; the supported wheel subset remains canonical exact names, with A_S distinguished from ordinary A. Observed wheel/interpolation counts are VSHTNKH 6/0, VUBSTAH 4/0, VUHTNKH 6/4. Auxiliary idlers remain unconfigured.

The BBOX loader at `0x80329510..0x80329554` reads six floats into metadata min XYZ `+0x20/+0x24/+0x28`, max XYZ `+0x30/+0x34/+0x38`. Wheel assembly at `0x80068c08` derives radius from half the absolute Y span. Rust preserves this radius without multiplying by vertex quantization scale; overflow is rejected. This is not a tire/contact solver.

The native node-kind records at `0x804ec2f0` have **u8 output flag at +0, an unclassified byte +1, u16 kind +2, u32 name pointer +4** (the earlier raw first-u16 description did not identify the flag byte). PHYSICS_COLLIDE is **31**, not 32. `model::Model::hierarchy_links` now retains the original TCNC edge order because parent indices alone lose insertion order. Missing declared links and duplicate link sections are rejected. Native TCNC loading at `0x803275b4..0x80327604` appends children in edge order; instance cloning at `0x8030dd98..0x8030ddf0` preserves that order. Child is +4 and next sibling +0. This now supports explicit box-chain selection.

## Body-factory boundary

Factory `0x803693a8` has a **second override check** at manager `+0x34d54`. Startup `0x8003c584..0x8003c58c -> 0x8036963c` supplies it from the same global +0x50 object's +0x1d4 field used for profile selection. It is separate storage/checking, not evidence of an independent user setting. Sentinel 2 uses the requested engine, or 1 when the request is itself 2. Missing/inactive backend entries at `+0x34d58/+0x34d5c` return null. Constructor `0x80368f78` initially clears backend zero and installs backend one at `0x803691bc`, enabled at `0x803691c0`. The recovered SP_0.1 catalog value forces backend one. PhysX SDK strings are present; do not select an ODE implementation merely because authored asset enums are zero. Backend parameter/solver integration remains unimplemented. It scans ten slots, stride `0xe58`, calls virtual preparation, then `0x80366324`; decoding 30 vehicle assets is not evidence of 30 active bodies.

Constructor `0x803660c4` installs vtable `0x804ee0d0`; its `+0x10` entry is `0x803685e4`. That geometry consumer locates kind-31 nodes and reads max-minus-min dimensions and node-instance positions. The bind-pose box selection is now ported; shape allocation, engine initialization, forces and dynamics are not yet integrated. Local traces include `vehicle-body-{factory-native,class-constructor}.txt` and `vehicle-physics-node31-80368630.txt`.

## Native box groups (metadata, not rigid bodies)

`src/vehicle_boxes.rs` ports the box-selection portion of `0x803685e4`. Chassis takes the first direct PHYSICS_COLLIDE child of root zero, then repeatedly its first direct PHYSICS_COLLIDE child; it does **not** gather all sibling markers. SEAT_A is found recursively in native order. TURRET is its direct child; BARREL is a direct child of TURRET, or of SEAT_A when no turret exists. Each shape chain is bounded to the eight entries indicated by the descriptor arrays.

Dimensions are raw BBOX max-minus-min. Positions use local translations and the native cumulative sums, including seat/turret/barrel offsets. They are **not** general bind-space corners, BBOX midpoints or vertex-quantization-scale products. Remaining markers are explicitly reported, not silently converted into hulls.

Observed chassis/turret/barrel/unselected counts: VSHTNKH **1/1/1/1** (PHYSICS_COLLIDE06 is unselected), VUBSTAH **1/1/1/0**, VUHTNKH **2/1/1/0**. Tests exercise reordered edges versus node order, chained offsets, barrel-without-turret, invalid/incomplete hierarchy, noncanonical role aliases, degenerate/overflowing bounds and over-capacity chains. These are recovered bind descriptors, not validated moving colliders.

Further body assembly at `0x80367178` computes volume-weighted positions and inertia terms from the selected box arrays. Mass scaling is obtained via `0x8036c57c -> 0x8036a280` from backend-entry +0x14; it must not be guessed from the authored mass alone. Parameter/backend/solver integration is still pending. Local traces: `model-hierarchy-link-loader.txt`, `model-instance-creation-8030dd64.txt`, `model-node-kind-name-{parser,match}.txt`, `vehicle-body-{box-groups-native,initialize-native}.txt`, `vehicle-backend-body-assembly.txt`.

## Verification and boundaries

`tests/vehicle_assets.rs`: engine policy, shared identities, malformed profiles, transformed physics nodes, invalid ancestry/geometry, all original ground-vehicle bindings and all three original model families. A binary-node test covers BBOX lengths, duplicates and NaN. Full viewer tests and original-data integration tests remain required after this shared-renderer change.

`bw-mission-audit --authored-startup --fresh-attempt` reports the selected profiles. Building the audit with `--features viewer` also reports physics-node metadata. Current resume output: `local/analysis/resume-effects-final-audit.json` (156 library and 64 selected integration tests passed at this checkpoint); older output `local/analysis/mission-vehicle-assets-audit.json` predates catalog selection; `dynamics_configured` remains false. Without viewer, geometry is explicitly null, not silently asserted valid.

Next integration work remains: native shape/parameter consumers and vehicle dynamics, complete terrain/obstacle installation, lifecycle/roster handling, and unmodified intro playback. No fabricated movement, omitted active-unit collisions, or completed-mission flags are introduced.
