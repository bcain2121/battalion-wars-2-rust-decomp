# First native Bevy terrain viewer

2026-10-02. Scope: SP_0.1 terrain only; macOS smoke test.

## Proven data structure

- PAMC contains 64x64 entries, each two big-endian u16 values.
- Observed inactive entry: flag 2, index 65535. Active: flag 1, record index.
- 734 active entries; indices span 0..733.
- KNHC is 2,113,920 bytes = 734 records x 2,880 bytes.
- Each record has 16 subtiles in 4x4 row-major order, 180 bytes each.
- First 32 subtile bytes contain 16 big-endian u16 heights on a 4x4 grid.
- Neighboring subtiles share their edge vertices, giving 12 grid intervals per record.
- Independent Python checks: 70,464/70,464 within-record edge comparisons match; 22,464/22,464 cross-record edge comparisons match.
- Rust's global grid duplicate check compares 81,439 overlapping samples with zero mismatches. Its count differs because it compares repeated coordinate samples rather than all neighbor pairs.

## Native implementation

`src/terrain.rs` reconstructs positions and triangle indices with bounded sections and record lookup. `src/bin/bw-viewer.rs` displays that geometry using Bevy 0.19.1, pinned as an optional dependency with 3D features. Default CLI/parser builds do not compile Bevy.

Actual generated mesh: 187,904 vertices; 211,392 triangles. Duplicate patch-edge vertices are retained for now.

## Important limitations

This is a geometry diagnostic, not visual/gameplay parity:
- Horizontal coordinates use one unit per recovered grid interval.
- Vertical coordinates use raw u16 height / 32 for readable inspection. This divisor is PROVISIONAL, not a recovered world-unit scale.
- Original world origin and handedness remain unverified.
- Triangle diagonals are chosen consistently, not yet proven to match the original topology.
- RGB values are tentatively taken from bytes 32..96 of each subtile. Fourth-byte meaning is unresolved; displayed alpha is 1.
- Unlit material, provisional vertex colors, flat placeholder normals (unused by unlit shading), no terrain texture blending, water, scenery, units, animation, scripting, or gameplay.
- No collision geometry or navigation inferred from this rendering.

## Launch

From repository root:

```sh
cargo run --locked --features viewer --bin bw-viewer -- local/extracted/files/Data/CompoundFiles/SP_0.1.out
```

Controls: arrow keys orbit/tilt; W zooms in; S zooms out. Close the window to stop.

Optional screenshot: set `BW_CAPTURE` to a local PNG output path; frame 90 requests a Bevy window capture. Use a fresh path; Bevy's screenshot saver may overwrite an existing destination.

## Validation

- Successfully built against Bevy 0.19.1 (stable; 0.20 prerelease deliberately not selected).
- Launched native application; renderer selected Apple M5 / Metal.
- Captured and visually inspected `local/analysis/terrain-bevy.png` (1280x720).
- Terrain surface renders; this is not a screenshot of the original Wii game and not proof of coordinate/material parity.
- Viewer smoke-test process was terminated and waited for by the owning test. No viewer left running.
- Native Windows/Linux launch not tested.
- Three terrain tests cover flat record geometry, bad map index, truncated data; total Rust tests now 16.

## Next work

Recover world scale/origin using collision data and object placements; determine actual terrain materials/UV mapping; decode original scenery/unit model meshes; overlay known objects only after coordinate alignment is proven.

## Dependency references

- crates.io Bevy metadata: stable 0.19.1, Rust MSRV 1.95.0.
- Context7 /websites/rs_bevy custom mesh documentation consulted.
- Installed Bevy 0.19.1 screenshot example consulted for window-only capture (no desktop capture).
