# Resource, texture and terrain investigation

2026-10-02 — SP_0.1 sample, original local image only.

**Historical first-inspection snapshot.** Later terrain/RGBA8/rendering work supersedes several limitations below. Current original model, authored-scene, skinning and animation status is in [ORIGINAL_RECOVERY.md](ORIGINAL_RECOVERY.md); playable prototype status is in [MODDING_AND_CONTROLLERS.md](MODDING_AND_CONTROLLERS.md).

## Rust archive reader

`src/archive.rs` implements bounded little-endian length-prefixed chunk iteration and selected named-resource headers. It consumes the entire resource archive, reports 603 chunks, and matches the Python chunk-type inventory exactly. Unknown top-level tags remain inventoried; their payloads are not silently treated as understood.

RXET payload: little-endian u32 resource-name length + name + nested FTBG chunk. FTBG begins with a little-endian u32 texture count followed by DXTG chunks. Count verified: 553 textures. This differs from the XML Hints count; hints are not authoritative.

DXTG observed header offsets relative to its payload:

| Offset | Observed field |
|---|---|
| 0..32 | fixed-width texture name, NUL padded |
| 32..36 | width, BIG endian u32 |
| 36..40 | height, BIG endian u32 |
| 52..56 | four-byte stored format tag |
| 112 onward | mip chunk stream for sampled 1TXD textures |

Mixed endian data is important. An initial little-endian dimension interpretation produced impossible values and was corrected before accepting evidence. Rust tests now encode big-endian dimensions. Sample dimensions range from 8 to 1024 pixels in width.

Observed format tag counts:
- 1TXD: 331
- NUL NUL 8P: 155
- NUL 8AI: 40
- 8R8A: 19
- NUL NUL 8I: 5
- NUL 4AI: 3

Only 1TXD top-mip decoding is implemented. Other tags are recorded, not decoded.

## First actual texture previews

`src/texture.rs` implements an initial CMPR decoder: 8x8 macrotiles, four 4x4 subblocks in raster order, big-endian RGB565 endpoints, high-bit-first two-bit row selectors. RGB interpolation uses a BC1-style approximation; exact Wii interpolation/transparent RGB parity is not established.

`bw-texture` requires the observed 1TXD tag and first ` PIM` chunk at the observed offset, checks compressed byte count, and creates a new RGBA TGA without overwriting existing output.

Successfully decoded and visually inspected:
- `local/analysis/HUD_FLAG01.png` — 64x64 recognizable flag with transparency.
- `local/analysis/C4TTROCKB.png` — 512x512 coherent rocky terrain texture.

TGA is emitted by Rust; macOS sips converted it to PNG for inspection. PNG conversion is NOT a portable runtime dependency. Extracted previews remain local/ignored, not tracked game assets. Visual coherence is not pixel-exact validation against the original renderer.

```sh
cargo run --locked --quiet --bin bw-archive -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res
cargo run --locked --quiet --bin bw-texture -- local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res C4TTROCKB local/analysis/new-rock-preview.tga
```

## Terrain inspection

Extracted only SP_0.1.out.gz; decompressed size 2,522,748 bytes. Same chunk framing consumes entire file. Tags: RRET, ROHS, KNHC, FNPG, PAMC, TCWU, MLOC, LTAM.

RRET payload has little-endian u32 values 64,64,1,14. PAMC payload is 16,384 bytes; this is consistent with (but does not prove) a 64x64 four-byte map. KNHC is 2,113,920 bytes. LTAM contains readable terrain material names matching texture resources. Height interpretation, tile placement, coordinates, topology and material bindings remain unresolved. Do NOT render guessed heightfields as validated terrain.

## Model inspection

LDOM records have a length-prefixed resource name and another LDOM structure. Observed nested markers include MEMX, EDON and XOBB. Models are not decoded yet; do not confuse chunk discovery with usable geometry.

## Validation and next work

13 Rust tests and 15 Python tests pass. Clippy passes with warnings denied. Rust resource chunk inventory matches Python. Real texture previews inspected.

Next: determine terrain tile/index/height structure and export a verified geometry slice; expand texture formats as needed; introduce Bevy once real geometry can be loaded. Bevy viewer and playable runtime are not implemented yet.

Research search lead (not an authoritative implementation reference): https://encode.su/threads/1905-Compressed-Pixel-Formats — search result describes 8x8 CMPR blocks. Current decoder is independently implemented from the observed layout and standard block-compression structure; cross-check against authoritative hardware/emulator decoding remains pending.
