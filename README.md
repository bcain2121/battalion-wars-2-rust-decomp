# Battalion Wars 2 — Native Rust Decompilation & Engine (`battalion-wars-2-rust-decomp`)

An independent, clean-room native engine reimplementation and decompilation of **Battalion Wars 2** (`RBWE01`) written in **Rust** and **Bevy**, targeting macOS, Linux, and Windows.

This is a **native PC/Mac engine port** — not an emulator and not a mod. It reads the original game's compound archives (`.res` / `.xml`), 3D skinned models (`NODE` / `SKIN`), 38-motion skeletal animations (`ANIM`), terrain heightfields (`TERR` / `WATR`), Tequila particle effects (`FEQT`), `.THP` FMV movies, DSP ADPCM audio banks, and compiled Lua 5.0 mission/frontend bytecode directly from a user-supplied disc image (`.wbfs` or `.iso`).

---

## Legal & Copyright Notice (Bring Your Own Disc / ROM)

**Zero copyrighted game assets, disc images (`.wbfs` / `.iso` / `.rvz`), executable binaries (`main.dol` / `main.elf`), textures, models, audio, or cryptographic keys are included in or distributed with this repository.**

- All paths containing proprietary or extracted material (`*.wbfs`, `*.iso`, `*.dol`, `/Battalion Wars 2 (USA)/`, `/DATA/`, `/local/`, `/extracted/`) are strictly excluded via [`.gitignore`](.gitignore).
- To play the game, you must supply your own legally obtained copy of **Battalion Wars 2 (USA)** (`RBWE01`).
- The automated library test suite (`cargo test --locked --features viewer --lib`, 221 unit tests) runs 100% on synthetic fixtures without requiring any copyrighted files.

---

## Quick Start (How to Play)

### 1. One-Click Launch on macOS: [`Play Battalion Wars Rust.command`](Play%20Battalion%20Wars%20Rust.command)
1. **Drop your `.wbfs` or `.iso` ROM into the repo folder (or select it when prompted):**
   - Place your **Battalion Wars 2 (USA)** (`RBWE01`) `.wbfs` or `.iso` file into the repository root (or inside `Battalion Wars 2 (USA)/`).
   - If no ROM is in the folder on first launch, a native file-picker window opens automatically so you can choose your `.wbfs` or `.iso` file from anywhere on your drive.
2. **Automatic First-Run Extraction:**
   - Double-click **[`Play Battalion Wars Rust.command`](Play%20Battalion%20Wars%20Rust.command)**.
   - On your first run, the launcher automatically extracts the Wii disc filesystem into `local/extracted/files/`, decompresses all `.gz` compound archives, compiles the native Rust engine, and launches the game.
   - *(Alternative)* If you already extracted Partition 0 (`DATA`) using Dolphin Emulator, you can place the extracted files in `local/extracted/files/` or `DATA/files/`.

To verify your ROM's SHA-256 against [`docs/evidence/source-image.sha256`](docs/evidence/source-image.sha256):
```sh
shasum -a 256 "Battalion Wars 2 (USA)/Battalion Wars 2 (USA).wbfs"
# Expected: 0db75fa75df64e8630aae762a0f54682e9d759454d3f3d4bcfdbdf5a13210864
```

### 2. Launch from Terminal (macOS / Linux / Windows)
```sh
# 1. Extract your ROM once (if not using the .command launcher):
python3 tools/extract_wbfs.py "path/to/Battalion Wars 2 (USA).wbfs" local/extracted

# 2. Run the native game engine:
cargo run --release --locked --features viewer --bin bw-first-mission
```

---

## What We Have Accomplished So Far

### 1. Native Lua 5.0 Virtual Machine & 268 Decompiled C-to-Lua Bindings
- **Custom Lua 5.0 Bytecode VM ([`src/lua50.rs`](src/lua50.rs), [`src/mission_vm.rs`](src/mission_vm.rs), [`src/mission_program.rs`](src/mission_program.rs)):** Executes the unmodified big-endian PowerPC Lua 5.0 bytecode directly from the retail `.out` bundles, including coroutines, upvalues/shared closures, C-runtime deterministic `rand()` ([`src/mission_random.rs`](src/mission_random.rs)), and whole-tick transactional state rollback.
- **268 Native C-Function Bindings Decompiled:** Reverse-engineered the native function registration tables (`0x804b1a60..0x804b22c0`) from `main.dol`, implementing unit spawning, orders, waypoint routing, area/flag queries, camera scripting, HUD tutorials, CO radio transmissions, capture points, transport boarding, and scoring.

### 2. All 20 Single-Player Campaign Missions Playable End-to-End (`SP_0.1` – `SP_5.4`)
- Every single-player mission across all 5 campaigns (**Prologue**, **Anglo Isles**, **Solar Empire**, **Western Frontier**, **Tundra Territories**, and **Xylvania**) loads its original compound archives, executes its intro/mid-mission/outro cutscenes and mission scripts, and reaches authored `WinMission` (`MissionOutcome::Won`) and `S/A/B/C` Speed/Power/Technique grading ([`src/mission_world.rs`](src/mission_world.rs), [`tests/campaign_levels_runtime.rs`](tests/campaign_levels_runtime.rs)).

### 3. Authentic Wii Boot Sequence & 96-Script 3D Globe Frontend
- **Full `Frontend2_Level` Runtime ([`src/frontend_runtime.rs`](src/frontend_runtime.rs), [`src/frontend_gui.rs`](src/frontend_gui.rs)):** Initializes and constructs all 96 frontend Lua scripts and resolves all 54 `cGUIPage` hierarchies (1,683 `cGUI*Widget` nodes).
- **Complete Player Flow ([`src/bin/support/first_mission_view.rs`](src/bin/support/first_mission_view.rs)):**
  - Wii wrist-strap & ESRB boot screens $\rightarrow$ `BWii_Intro.thp` FMV movie (MJPEG video + THP ADPCM audio) $\rightarrow$ Title Screen with layered 3D rotating satellite globe (`Globe_sat` + `Globe_sat_clouds` + `Globe_atmos`) and Wii Remote hand pointer $\rightarrow$ 3-slot Save Select (`SAVE A/B/C` persisted to `local/saves/bw2_saves.json`) & Wii On-Screen Keyboard $\rightarrow$ 2x2 Main Menu $\rightarrow$ 6-Continent 3D Globe Campaign Select, Unit Dossier, Concept Art Gallery, 15 `.THP` movies, and Credits.

### 4. All 16 Multiplayer Maps (`MP1`–`MP15`, `MP17`), Skirmish AI, Split-Screen & UDP Netplay
- **Assault (`MP1`–`MP6`), Skirmish (`MP7`–`MP11`, `MP17`), and Co-Op (`MP12`–`MP15`):** All 16 multiplayer levels load and run with an autonomous Skirmish AI opponent, dual-viewport top/bottom **2-Player Split-Screen** (`PLAYER_TWO`), and lockstep UDP `NetPeer` networking ([`src/net_sync.rs`](src/net_sync.rs)).

### 5. 38-Motion Skeletal Animation, Vehicle Physics, Aircraft/Naval Piloting & `FEQT` Particles
- **38-Motion `cTroopAnimationSet` Repertoire ([`src/unit_animation.rs`](src/unit_animation.rs), [`src/skinning.rs`](src/skinning.rs)):** Full matrix-palette normal+position skeletal skinning, smooth locomotion phase continuity (`Walk`/`Run`/`Strafe`/`Sprint`), combat rolls, jumps, upper-body spine aim-pitch/yaw twist blending, left/right hand weapon bone attachment (`meHoldWeaponInLeftHand`), and visible seated vehicle drivers, gunners, and passengers.
- **1-to-1 Unit Physics ([`src/vehicle_wheels.rs`](src/vehicle_wheels.rs), [`src/mission_air.rs`](src/mission_air.rs)):**
  - **Ground Vehicles:** 12-wheel terrain height raycasting, spring-damper suspension compression, chassis pitch/roll, tank tread UV scrolling, main-gun firing recoil impulse (`mFireImpulse`), and turret/barrel articulation.
  - **Aircraft & Helicopters:** Continuous fixed-wing forward cruise speed, banked turns, `Space` 360° barrel-roll evasion, and helicopter hover/strafe/reverse flight.
  - **Naval Vessels & Submarines:** Wave bobbing, turn banking, and submarine dive/surface toggle (`Space`) with depth transitions and HUD oxygen gauge.
- **Tequila Particle System (`FEQT`) & Morphing Buildings ([`src/tequila.rs`](src/tequila.rs)):** Native `cTequilaEffectResource` parser with live 3D billboard bursts, `ColourRange` keyframe tinting, and `cMorphingBuilding` intact/destroyed/faction-swap model transitions.
- **Clean 3D Positional Audio:** Decodes original DSP ADPCM sound effects and voiceovers with 44.1kHz linear upsampling, anti-click fade envelopes, polyphony limiting, and distance attenuation.

---

## What Is Left To Do (Current Roadmap)

While the entire campaign and multiplayer map catalog are playable from boot to victory, **we are now in active community playtesting and decompilation parity refinement**:

1. **Human Playtesting & Edge-Case Polish Across All 36 Levels:**
   - Finding and fixing subtle visual, audio, animation, or scripting edge cases during real human playthroughs of all 20 campaign missions and 16 multiplayer maps (like the Prologue cutscene floor shader, infantry locomotion phase, and explosion audio polyphony fixes we just landed).
2. **1:1 PowerPC Decompilation Parity for Physics & AI Subsystems:**
   - Several runtime systems currently use clean-room Rust approximations calibrated against the original XML/resource data rather than instruction-exact translations of the PowerPC `main.dol` functions:
     - Exact ODE rigid-body inertia, wheel friction curves, and vehicle-to-vehicle collision impulse formulas.
     - Exact native AI target-selection scoring weights and squad formation slot math.
     - Remaining rare/edge-case native C-to-Lua function flags.
3. **Wii GX / TEV Shader & Lighting Fidelity:**
   - Full multi-stage GameCube/Wii TEV combiner emulation in WGSL (environment-mapped metallic reflections, projected shadow blobs/volumes, multi-layer terrain detail splatting, and weather/bloom post-processing).
4. **Cross-Platform Packaging & One-Click Launchers (Windows, Linux, Steam Deck):**
   - Adding native `.bat` / `.sh` one-click setup scripts for Windows and Linux, and testing controller/audio hardware across Windows, Linux, and Steam Deck.
5. **Online Multiplayer Matchmaking UX & Modding Tools:**
   - In-game host/join lobby UI for `NetPeer` UDP multiplayer (currently configured via `BW_NET_BIND` / `BW_NET_PEER` env vars) and visual level/asset modding tools.

---

## How You Can Help as a Community

We are actively looking for contributors across several areas! See **[`CONTRIBUTING.md`](CONTRIBUTING.md)** for full details:

- **Playtesters:** Grab your `Battalion Wars 2` ROM, play through the campaign or skirmish maps, and open GitHub Issues with bug reports, screenshots, or behavioral comparisons against Dolphin/Wii hardware.
- **Ghidra / PowerPC Decompilers:** Help decompile and document remaining functions in `main.dol` (`RBWE01`) so we can tighten physics, AI, and edge-case script parity.
- **Rust / Bevy / WGPU Developers:** Help improve WGSL shaders (TEV material stages, water, shadows, `FEQT` particle emitters), audio mixing, UI polish, and networking.
- **Windows / Linux / Steam Deck Testers:** Test compilation, ROM extraction, and gamepad controls on your OS and help streamline cross-platform launchers.

---

## Controls & In-Game Remapping

Press **`F4`** at any time (or select **Options $\rightarrow$ Customize Keybindings & Gamepad** from the Main Menu) to open the interactive **Keybindings & Controller Remapping** screen. All 24 keyboard and gamepad bindings are saved automatically to [`config/controls.json`](config/controls.json).

| Action | Keyboard / Mouse Default | Gamepad Default |
| :--- | :--- | :--- |
| **Move / Throttle / Steer** | `W` `A` `S` `D` | Left Stick |
| **Aim / Look / Camera** | Mouse or Arrow Keys | Right Stick |
| **Fire Primary Weapon** | Left Mouse Button | Right Trigger (`RT`) |
| **Lock-On Target (`Z`)** | Right Mouse Button or `Tab` | Left Trigger (`LT`) |
| **Jump / Combat Roll / Barrel Roll / Sub Dive** | `Space` | South (`A` / Cross) |
| **Transfer Control to Locked Unit** | `T` | North (`Y` / Triangle) |
| **Send Squad / Attack / Capture / Board** | `G` or Middle Mouse | East (`B` / Circle) |
| **Recruit / Recall Squad (`Follow`)** | `C` | West (`X` / Square) |
| **Cycle Squad Weapon Group** | `Q` / `E` or `1`–`5` | D-Pad Left / Right |
| **Load / Unload Transport Passengers** | `V` | D-Pad Down |
| **Confirm Tutorial / Dialogue** | `Enter` | South (`A` / Cross) |
| **Pause / Mission Menu** | `Esc` or `P` | Start |
| **Keybindings & Controller Remapping** | `F4` | Options Menu |
| **Player 2 (Split-Screen Multiplayer)** | `I` `J` `K` `L` move, `U`/`O` aim, `H` fire, `Y` switch unit | Second Gamepad |

---

## Verification & Testing

### 1. Asset-Free CI Checks (No ROM Required)
```sh
cargo fmt --check
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo test --locked --features viewer --lib
python3 -m unittest discover -s tests -v
```

### 2. Full Original-Asset Integration Suite (Requires `local/extracted/files/`)
```sh
cargo test --locked --features viewer --test frontend_runtime
cargo test --locked --features viewer --test first_mission_runtime -- --include-ignored
cargo test --locked --features viewer --test first_mission_squad -- --include-ignored
cargo test --locked --features viewer --test first_mission_staff -- --include-ignored
cargo test --locked --features viewer --test campaign_levels_runtime
```

---

## Documentation Index
- **[`CONTRIBUTING.md`](CONTRIBUTING.md)** — How to contribute, clean-room copyright rules, and areas needing help.
- **[`docs/ROADMAP.md`](docs/ROADMAP.md)** — Detailed compatibility roadmap and subsystem checklist.
- **[`HANDOFF.md`](HANDOFF.md)** — Architecture map, reproduction commands, and engineering invariants.
- **[`where we are.md`](where%20we%20are.md)** — Detailed engineering log and milestone history.
- **[`LICENSE`](LICENSE)** — MIT License & clean-room trademark disclaimer.
