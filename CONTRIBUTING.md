# Contributing to `battalion-wars-2-rust-decomp`

Welcome! Thank you for your interest in helping preserve and reconstruct **Battalion Wars 2** (`RBWE01`) as a native, cross-platform Rust + Bevy engine.

---

## 1. Strict Clean-Room & Copyright Policy (Read First!)

To keep this repository 100% legal and safe for everyone:

1. **NEVER commit or upload copyrighted game assets:**
   - Do not commit `.wbfs`, `.iso`, `.rvz`, `.dol`, `.elf`, `.res`, `.out`, `.btf`, `.thp`, `.str`, `.adp`, or any extracted contents from `local/`, `extracted/`, `DATA/`, or `Battalion Wars 2 (USA)/`.
   - Keep all extracted disc files and Ghidra/disassembly databases inside the git-ignored `local/` directory.
2. **No proprietary binary blobs in tests:**
   - Unit tests committed to this repository must construct synthetic in-memory fixtures (or verify behavior via SHA-256 / structural metadata without embedding copyrighted asset payloads).
   - Full-mission integration tests that read `local/extracted/files/` must cleanly skip or remain gated behind `#[ignore]` when local assets are absent.
3. **Fail-Closed Engine Architecture:**
   - Do not add hardcoded mission shortcuts, skipped cutscenes, or silent no-ops that fake gameplay progress.
   - Maintain transactional Lua tick semantics (`World` staged state rollback on error) so any unimplemented native binding is caught immediately and accurately.

---

## 2. Where We Need Community Help

Whether you are a player, a Rust/Bevy developer, or a PowerPC/Ghidra reverse engineer, there are huge ways to contribute:

### A. Playtesters (Campaign & Multiplayer)
- Play through all **20 Single-Player Campaign missions** (`SP_0.1`, `SP_1.1`–`SP_5.4`) and **16 Multiplayer maps** (`MP1`–`MP15`, `MP17`).
- Report any differences vs. original Wii hardware / Dolphin:
  - Unit movement/animation quirks or stuck AI pathfinding
  - Audio timing, voiceover overlap, or SFX attenuation issues
  - Cutscene camera framing or terrain clipping
  - Mission objective triggers or CO dialogue edge cases
- When filing an issue, include the **Mission ID** (e.g., `SP_1.3`), what happened, and how to reproduce it.

### B. Ghidra / PowerPC Reverse Engineers (`main.dol` / `RBWE01`)
- Help us replace remaining high-level Rust approximations with 1:1 decompiled logic from the original PowerPC binary:
  - **Physics & ODE Vehicle Dynamics:** Exact inertia tensors, wheel slip/friction curves, collision response impulses, and artillery ballistic arcs (`src/vehicle_wheels.rs`, `src/mission_combat.rs`).
  - **Native AI & Threat Evaluation:** Exact target scoring weights, formation slot offsets, and reactive evasion behaviors (`src/mission_world.rs`).
  - **Remaining Native C-to-Lua Bindings:** Auditing edge-case flag bits and uncommon script opcodes across the 268 native Lua bindings (`src/mission_vm.rs`).

### C. Graphics, Shader & Audio Engineers (Bevy / WGPU / WGSL)
- **Wii GX / TEV Shader Parity:** Translating multi-stage GameCube/Wii TEV combiners, specular environment maps, shadow volumes/blobs, and terrain splatting (`assets/shaders/`, `src/terrain_render.rs`, `src/original_scene.rs`).
- **Tequila Particle System (`FEQT`):** Expanding emitter curves, sub-emitters, trail ribbons, and blending modes (`src/tequila.rs`).
- **DSP ADPCM & 3D Audio:** Refining streaming music transitions, reverb/env effects, and polyphony mixing.

### D. Cross-Platform Testers & Maintainers (Windows, Linux, Steam Deck)
- Help verify and improve first-run ROM extraction and native builds on **Windows (x86_64/ARM64)**, **Linux (X11/Wayland)**, and **Steam Deck**.
- Add `.bat` / `.ps1` / `.sh` one-click launcher equivalents alongside `Play Battalion Wars Rust.command`.

---

## 3. Development Workflow & Running Tests

### Prerequisites
- Rust toolchain (automatically selected via [`rust-toolchain.toml`](rust-toolchain.toml))
- Python 3 (for asset extraction and synthetic fixture tests)

### Pre-PR Verification Checklist
Before submitting a Pull Request, run the asset-free test and lint suite:

```sh
cargo fmt --check
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo test --locked --features viewer --lib
python3 -m unittest discover -s tests -v
```

If you have your own extracted `RBWE01` ROM in `local/extracted/files/`, also run the integration suites relevant to your changes:

```sh
cargo test --locked --features viewer --test frontend_runtime
cargo test --locked --features viewer --test first_mission_runtime -- --include-ignored
cargo test --locked --features viewer --test first_mission_squad -- --include-ignored
cargo test --locked --features viewer --test first_mission_staff -- --include-ignored
cargo test --locked --features viewer --test campaign_levels_runtime
```
