# Original frontend recovery — incomplete

This is asset/flow research, **not a functioning original menu or mission acceptance**. The main launcher still selects the replacement battle. The menu-to-authored-victory checklist remains unchecked.

## Local source

Extracted Frontend2 configuration, level/preload object XML, resource archive, English string table and frontend font assets from the existing local disc. Originals and decoded resource inventories remain ignored under `local/`.

Frontend graph has 82 GUI pages, 497 text widgets and 213 button widgets. Page `mpScript` resolves through the initialization entity to a named game-script resource. Start is 2144911942; Main is 2144911973; Save is 310002031; Game is 2144912002. Inventory contains 96 script chunks and 37 model chunks.

## Fresh-profile flow evidence

Original Lua callbacks, inspected statically (not yet run as a frontend):

- `Start.gotoMain` pushes `LoadCampaignData`, not directly Main.
- `LoadCampaignData.Update` waits for `GameSaveLoaded`, then pushes Save.
- `Save.gotoMap` selects a slot, applies settings and handles a blank profile name before pushing MainPre.
- `MainPre` transitions to Main.
- `Main.gotoSave` checks completed levels. Fresh-profile branch selects level/campaign 1, sets `GAMESTATUS.TRAINING`, then pushes CutsceneIn.
- CutsceneIn contains `0.1_Prologue_1`; it registers the mission attempt and calls SetCurrentLevel before transitioning toward Game/loading.

This is not permission to skip profile handling, transition animation, movies or persistence. Remaining branches must be recovered and tested. GStringID uses a global-string-count offset; do not mistake its frontend-local offsets for complete runtime string IDs.

## Page-stack execution follow-up

The next bounded slice is complete: the original PageStack Lua resource now executes push/pop/replace/reset/getter operations through the shared VM. See [FRONTEND_PAGE_STACK.md](FRONTEND_PAGE_STACK.md) for tests, rollback evidence and explicit integration limits. This does not yet execute page lifecycle callbacks or render menus.

## Authored graph loader

`src/frontend.rs` now loads all **82 pages and 1,955 widget definitions** from the original level/preload XML. It resolves page script identities and event-handler arrays while preserving null slots, shared widgets, original metadata fields, callback names and blip values. Missing child widgets, ancestry cycles, bad reference counts, duplicate selected fields and incorrect script/event types fail explicitly. Dynamic metadata is retained, not evaluated; native UI defaults are not invented.

`cargo run --locked --bin bw-frontend-audit` produces a read-only structural report. Current local output is `local/analysis/frontend-graph-audit.json`. It explicitly reports callbacks not executed, menus not rendered and mission not launched. This is loader progress, not a functioning menu.

Three synthetic integration tests verify identity/slot preservation and malformed-graph rejection; one opt-in original-data test verifies the 82 pages and Start's authored `eGUIEvent_OnAB → gotoMain` binding. All pass. The 123 viewer-library tests and warning-denying viewer/all-target Clippy also pass.

```
cargo test --locked --test frontend_graph
cargo test --locked --test frontend_graph -- --ignored --nocapture
```

## Texture capability

Shared Rust top-mip decoder now supports GX I4, I8, IA4, IA8, indexed P4/C4 and P8/C8 with RGB5A3 palettes, alongside existing CMPR and RGBA8. Bounded sizes, exact tiled payload sizes and referenced palette-index checks are enforced. The `bw-texture` CLI now uses the same decoder instead of a separate CMPR-only path, and retains exclusive-create output behavior.

Local integration checks examine all 432 frontend textures across eight formats: **425 decode, seven explicitly reject short tiled payloads**:

- P4: OP2_screen, OP1_desk, OP4_nunch, OP5_screen2.
- RGBA8: PHTO_06_SolBeach, FM_panel_left, FM_panel_mid.

Example: OP2_screen declares 320×220 but contains 35,200 P4 bytes, less than the 35,840 bytes required by full 8×8 tiles. The community converter appends zeros to its input buffer; this implementation deliberately does not silently synthesize missing pixels. Native behavior/layout still needs investigation.

Original BWii_logo (400×124) was decoded to a local TGA/PNG and visually inspected. This is an asset preview, not a native menu screenshot or pixel-accuracy comparison with the game.

Format reference: RenolY2/battalion-level-editor commit c2b074cfac58af6e751c8cb7e9713fa6b7b795e5, `plugins/bw_texture_conv/bwtex.py` and `lib/bw/texlib/texture_utils.py`. The latter's LagoLunatic MIT notice is retained in `references/texlib-MIT.txt`; the editor notice is retained separately. No original images are redistributed.

## Verification

```
cargo test --locked --lib texture::
cargo test --locked --test frontend_assets -- --ignored --nocapture
cargo clippy --locked --all-targets -- -D warnings
```

Unit fixtures cover tile ordering, odd image edges, nibble ordering, intensity/alpha expansion, both RGB5A3 branches, palette range failures and short data rejection. Local integration explicitly expects the seven failures above; it does not claim every frontend texture is supported.

Next: recover bounded GUI bindings and frontend script execution, actual save/profile semantics, full prologue presentation, and original-mission launch integration. The mission's navigation/vehicle and simultaneous scheduling blockers remain unchanged.
