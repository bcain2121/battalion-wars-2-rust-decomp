# Battalion Wars Rust

Independent native reimplementation of Battalion Wars 2 in Rust with Bevy, targeting macOS, Linux, and Windows. This is not an emulator and not a mod. Full-game compatibility is the long-term goal. The current playable build uses original terrain, scenery, textured troops and animation resources with direct control, combat, enemy AI and squad support. Its small **Fort Assault** encounter uses replacement rules; the complete original mission remains unfinished.

## Continuing this project

**New agent: read [HANDOFF.md](HANDOFF.md) first**, then [where we are.md](where%20we%20are.md) and the prioritized [roadmap](docs/ROADMAP.md). This checkout is largely untracked on `main` with no commits yet: preserve it; Git cannot restore untracked work. Older milestone docs retain historical blockers and test counts, not current status.

## Original first-mission integration build

Double-click **Preview First Mission Runtime.command**, or run `cargo run --locked --features viewer --bin bw-first-mission`.

The original opening, movement/look tutorials and first barrel-destruction objective now work with all 97 global/unit coroutines. Gold stars provide guidance; source-profile rifles, damage, reload and nearby rifle return fire are connected. Walking onward now completes camp arrival and the original bomber ambush, including anti-air missile contacts, bomb impacts, the staff reveal and restored player control. The faded staff-player handoff now works, but a battle waypoint order blocks the staff cutscene before control returns. See [STAFF_TRANSFER.md](docs/STAFF_TRANSFER.md) for the 319-test checkpoint and [AMBUSH_COMPLETION.md](docs/AMBUSH_COMPLETION.md) for the earlier ambush acceptance and remaining presentation limits.

WASD/left stick moves; arrows/right stick looks; LMB/RT fires; RMB/Tab/LT assists aim; E/X calls nearby troops; Enter/A acknowledges the original pause tutorial; R restarts.

**Not yet a complete mission:** subsequent squad/attack objectives, heavy weapons, capture, victory/defeat, full animation and hardware audio remain pending. See [FIRST_COMBAT.md](docs/FIRST_COMBAT.md) for validation and limits.

## Original battlefield preview (Mac)

Double-click **Play Battalion Wars Rust.command**. It now opens the original SP_0.1 scene in native Rust, not Fort Assault. Three viewpoints show the staff bearer, an authored landmark and the battlefield overview.

**WASD** move the view; **arrows** look; **Q/E** zoom; **Tab** next viewpoint; **R** reset; **Esc** close. Controller bindings: left/right sticks, bumpers, A and Y; physical hardware verification remains pending.

This is a **static restoration preview, not the playable original mission**. Its panel describes the older restoration/strict-reference scope, not the newer playable-first runtime above. See [ORIGINAL_PRESENTATION.md](docs/ORIGINAL_PRESENTATION.md).

## Separate replacement-rule prototype

Double-click **Play Fort Assault Prototype.command**, then press **Enter**.

Eliminate four enemy riflemen with your two squadmates, then secure the cyan uplink for five seconds.

WASD move; arrows/Q/E camera; wheel zoom; Space or left mouse fire; Tab switch lock; Shift sprint; **C squad hold/follow**; P pause; R restart; Esc quit.

See [PLAYABLE_BATTLE.md](docs/PLAYABLE_BATTLE.md) for controls, honest limitations and **114 passing tests** plus native win/loss/restart verification. Animation fidelity and original campaign behavior are not yet restored.

The separate **Play Custom Sandbox.command** retains the older JSON-authored prototype. Edit `config/controls.json` to remap most controls.

Gamepad defaults: left stick move, right stick camera, RT fire, LB switch target, South/A/Cross start. Hardware testing remains pending.

See `docs/MODDING_AND_CONTROLLERS.md` for the prototype mod interfaces; `docs/FIRST_PLAYABLE.md` records the historical capsule-skirmish milestone.

## Original mission restoration (in progress)

[MISSION_STARTUP.md](docs/MISSION_STARTUP.md) records the authored initializer/closure work, recovered presence/activity distinction, original localization and voice decoding, and the remaining engine-contract blockers. The main launcher offers an original-scene inspection preview; a restored playable campaign mission is still blocked.

## Original unit collision preview

Double-click **Preview Original Unit Collision.command** to watch an original troop route around another active original troop and return home. **R** replays movement; **P** pauses; **K** kills the moving troop. Collision remains enforced throughout the detour. This is a fixture, not a playable original mission. See [UNIT_NAVIGATION.md](docs/UNIT_NAVIGATION.md) for the 107-test milestone, bounded planner limits, and native evidence; [UNIT_COLLISION.md](docs/UNIT_COLLISION.md) records the preceding milestone. Original navigation parity, moving vehicle physics, and full mission integration remain unfinished.

## Working rules

- Work solo; current authorization explicitly excludes subagents unless William changes it.
- Preserve the original image. Inspection tools open it read-only.
- Keep original binaries, extracted assets, keys, and local tool environments out of Git.
- No publication, upload, or push without William's authorization.
- William reports developer permission; scope of redistribution has not been independently established. Do not assume permission to distribute third-party platform material.
- Record observations separately from hypotheses and cite external references.
- Research tools can use Python; the replacement runtime will use Rust/Bevy.

## Bevy terrain diagnostic

```sh
cargo run --locked --features viewer --bin bw-viewer -- local/extracted/files/Data/CompoundFiles/SP_0.1.out
```

Original terrain heights render natively; arrow keys orbit, W/S zoom. World scale/origin, colors and triangle diagonals remain provisional. See `docs/TERRAIN_VIEWER.md`. This is not gameplay yet.

## Rust loader

```sh
cargo test --locked
cargo run --locked --quiet -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.xml local/extracted/files/Data/CompoundFiles/SP_0.1_Level_preload.xml
```

Loads real level/preload objects and validates references; not yet a playable runtime. See `docs/RUST_LOADER.md`.

## Original asset inspection

```sh
cargo run --locked --quiet --bin bw-archive -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res
cargo run --locked --quiet --bin bw-texture -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res C4TTROCKB local/analysis/rock-preview.tga
```

Original model geometry, authored scenery, bind-pose skinning checks, experimental rotation tracks and a restricted Lua 5.0 bootstrap are now implemented. **These do not restore original mission gameplay**. The main launcher displays the original-scene restoration preview; the replacement battle and custom sandbox have separate launchers. Current evidence and internal viewer commands: `docs/ORIGINAL_RECOVERY.md`. Original initializer execution and resumable scoring-script tests: `docs/MISSION_EXECUTION.md`. Original win/loss triggers, unit state and animation bindings: `docs/MISSION_GAMEPLAY.md`. Current single-script movement/animation integration in the native original scene: `docs/SCRIPT_RENDER_INTEGRATION.md` (still not a complete playable original mission). Earlier inspection history: `docs/ASSET_FORMATS.md`.

## Current inspection

```sh
python3 tools/inspect_disc.py 'Battalion Wars 2 (USA)/Battalion Wars 2 (USA).wbfs'
uv venv .venv
uv pip install --python .venv/bin/python -r requirements-research.txt
.venv/bin/python -m unittest discover -s tests -v
```

See `docs/RESEARCH_LOG.md`, `docs/FILESYSTEM_FINDINGS.md`, `docs/ROADMAP.md`, and `docs/references/disc-and-reengineering.md`.

Filesystem inventory (local key required):
```sh
.venv/bin/python tools/inventory_filesystem.py 'Battalion Wars 2 (USA)/Battalion Wars 2 (USA).wbfs' --key-file local/tools/common-key.bin --output docs/evidence/filesystem-inventory.json
```

The inspector validates H0 hashes for accessed clusters only. It does not certify full-disc integrity. Extraction uses exclusive file creation and will not overwrite prior extracted files.
