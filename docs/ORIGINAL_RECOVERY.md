# Original units, scenery, animation and script-bootstrap recovery

2026-10-02 continuation. This advances original-content compatibility, **not** another terrain-material milestone. A restored original mission is **not ready to play**. At this historical checkpoint, play launchers still ran the temporary skirmish. The main launcher now runs the original-asset replacement battle in `PLAYABLE_BATTLE.md`; the tools below remain internal recovery viewers, and the complete original mission remains unfinished.

**Later execution update:** [MISSION_EXECUTION.md](MISSION_EXECUTION.md) records two completed original initializer bodies and a controlled original scoring-coroutine test. Counts and bootstrap-only statements below describe this earlier recovery slice.

## Verified this slice

- `src/model.rs`: native Rust decoder for nested LDOM revision `(101, 200)`, EDON node headers, transforms, material names, first-LOD vertex/UV arrays, GX indexed primitives, and TCNC parent links. Rejects truncation, unsupported GX commands, invalid indices, cyclic hierarchy and unsupported versions. Synthetic fixture contains no game data.
- All **116** first-mission models decode. The emitted first-LOD index buffers contain **264,040 triangle entries including strip degenerates**, not 264,040 independently verified visible triangles. Secondary LODs, material flags, original normals and multi-texture effects remain incomplete.
- Original **SVET** geometry and diffuse texture render on native Metal. No capsule or generated substitute is used in the model gallery.
- `src/original_scene.rs` resolves **1,016** authored primary-model placements through the original object graph. The preview renders **1,005** of them, skips **5** outside available terrain and omits **6** instances due to unavailable palette textures. A separate **100** world objects lack a supported primary model, including **90** scenery clusters whose expansion rules are not implemented.
- Scene preview requires 85 model resources; three whole models are omitted rather than silently assigned invented textures: `SLOGOsmall`, `VSGSHPH`, `VUGSHPH`. Missing/unsupported diffuse dependencies include `ArmyColours`, `rotorblur2`, `VGLASS`, and `vglassopaque`. Only first nonempty material texture slots are rendered; bump/specular slots are not claimed supported.
- Native scene capture visually shows authored buildings, terrain-facing rock walls, vehicles and troops together. All authored instances are considered; **original activation/spawning/cutscene rules are NOT applied**. This is not a snapshot of a proven original mission state.

## Skeleton and animation evidence

RNOD tables decode as a little-endian count followed by 64-byte inverse-bind matrices. Two XBS2 words form the sampled bone-use bit mask. Ascending set bits define the matrix palette; GX matrix selectors are divided by 3. Inverse-bind reconstruction is supported by a strong numerical check (this does **not** independently validate the palette assignment): **6 skinned models, 16,555 emitted vertices**, reconstructed in bind pose with maximum per-coordinate error **0.00000071525574** against decoded geometry.

`src/animation.rs` reads all **184** MINA/BAN resources, including track names, translation quantization metadata, packed position samples, packed rotation samples and sample rates. A 64-byte track record starts after the 88-byte header. Observed data consumption is 4 bytes per position sample and 8 bytes per rotation sample. **84** clips retain uninterpreted trailing data, potentially related to root motion/events; its meaning is not established.

Rotation magnitudes use an experimental 14/14/15/15-bit interpretation. All **245,350** rotation samples satisfy a near-unit magnitude check (measured maximum squared-norm error about 0.0000869). Sign interpretation, conjugation and interpolation are candidates supported by held-channel comparisons, **not original-runtime parity**. CPU deformation can preview original rotation tracks. Captures show imperfect joint/limb results; this is not accepted as finished character animation. Compressed translations, root motion, blending, event dispatch, LOD switching and animated attachments remain unfinished.

Troop preview attachments use candidate named bones (`BONE_R_HAND`, `BONE_HELMET`, `BONE_BCK_MISC`). Exact attachment conventions are not yet verified.

## Source-coordinate alignment

The original-content viewer maps the earlier diagnostic grid to source space with:

```
x_source = x_grid * 16/3 - 2048
z_source = z_grid * 16/3 - 2048
y_source = y_diagnostic * 2       # stored height /16 instead of /32
```

This was cross-checked against the community terrain reader and visually aligns sampled scenery with terrain. It remains pending independent collision/original-runtime verification. The existing skirmish's coordinate scale was deliberately left unchanged. Ground-lock flags are read where available; troop/ground-vehicle grounding is a provisional adapter and does not resolve platforms/navigation.

## Script behavior: important but limited progress

`src/lua50.rs` now parses the original Lua 5.0 chunks directly in Rust and executes a **restricted bootstrap instruction subset**. It is not a general Lua VM.

All **54 root chunks** initialize successfully: **4,660 executed instructions**, **741 validated object-ID registration calls**, **42 entity tables**, and **53 registered function identities**. This establishes original named mission references without rewriting the original tables by hand.

Safety contract:
- Only the explicitly installed `RegisterReflectionId` adapter may be called. It accepts a decimal string naming an existing graph object and returns a symbolic `ObjectId`.
- This adapter is not a claim about the original native pointer ABI or invalid-ID behavior.
- Unsupported calls/opcodes/captured closures fail; no permissive no-op stubs.
- Instruction, allocation/count and input-size limits; no filesystem, network, process or OS bindings.
- Bootstrap state commits transactionally. Failed execution leaves the previous globals/tables intact.
- Lua 5.0 RK constants start at operand **250**, not 256; this is covered by a synthetic test.
- **No registered mission function bodies have been run**, and no scheduler, win/lose logic, original combat or cinematic behavior is implemented by this bootstrap tool.

## Reproduce internal checks

```sh
cargo test --locked --features viewer --lib
cargo test --locked --features viewer --test original_assets -- --ignored --nocapture
cargo clippy --locked --features viewer --all-targets -- -D warnings
.venv/bin/python -m unittest discover -s tests -q
cargo run --locked --bin bw-script-bootstrap
cargo run --locked --bin bw-model-audit -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res

# Static original scene; not a mission runtime:
cargo run --locked --features viewer --bin bw-original-viewer
# Original soldier rest-pose gallery:
BW_MODEL=SVET cargo run --locked --features viewer --bin bw-original-viewer
# Experimental original rotation-track deformation (not accepted animation fidelity):
BW_MODEL=SVET BW_ANIMATION=FVSCRATCH cargo run --locked --features viewer --bin bw-original-viewer
```

`BW_SMOKE=1` makes this viewer exit after 180 frames. `BW_CAPTURE=local/analysis/name.png` captures frame 100 and may overwrite that explicit test output. Optional `BW_POSE_TIME` fixes a finite sample time. These settings are internal, not a new user-facing play launcher.

Validation: **50 Rust unit tests + 4 opt-in original-asset integration tests + 15 Python tests**, warning-denying Clippy, original/custom skirmish headless regressions, and native Metal model/scene/rotation-preview smoke runs. Test-created windows exited normally; existing user game windows were not killed. Linux, Windows and physical controllers remain untested.

Local-only reviewed images:
- `local/analysis/original-SVET.png`
- `local/analysis/original-scene-final.png`
- `local/analysis/original-scratch.png`
- `local/analysis/original-scratch-playback.png`

## Next acceptance gate — unchanged

A playable **original** mission must combine correct units/scenery/animation with original player control, targeting, orders, AI, vehicles, triggers/objectives, scheduling and win/lose behavior. It must pass gameplay checks, not just render. This gate is still open. Immediate work: verify animation decompression against executable routines, finish a capability-defined Lua 5.0 execution/scheduler path, and connect one original mission's actual state transitions to the simulation.

## External references and provenance

Community code was consulted for binary-format facts, not adopted as the engine or repository architecture:
- https://github.com/RenolY2/battalion-level-editor/blob/master/lib/bw/model_rendering.py
- https://github.com/RenolY2/battalion-level-editor/blob/master/lib/bw/gx.py
- https://github.com/RenolY2/battalion-level-editor/blob/master/lib/bw/read_binary.py
- https://github.com/RenolY2/battalion-level-editor/blob/master/lib/bw_terrain.py

That repository uses the MIT license, copyright 2019 Yoshi2; notice retained in `docs/references/battalion-editor-MIT.txt`. Reference snapshots remain under ignored `local/tools`; their hashes are recorded separately. This work is **not described as clean-room**. Skin-palette validation, packed BAN investigation and the restricted Rust bootstrap were checked against local original bytes. Original game material remains ignored/local and was not uploaded or committed.
