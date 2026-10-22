# Authored controller enable masks

## Implemented slice

`src/mission_controls.rs` adds a transactional, single-player enable-mask backend for the mission's supported symbolic controller items. `EnableControllerItem(item, 0|1 [, PLAYER_ONE])` stores the requested state. Unknown items, other players, booleans in the numeric enable slot and non-0/1 values fail explicitly. Twenty-one supported action names are declared (including the subsequently recovered GCONTROL_PAUSE_SELECT); CONTROL_JUST_PRESSED is a query mode, not an action, and is rejected here.

The default capability-probe instance leaves unassigned states unknown. The explicit `fresh_player()` constructor now initializes them enabled using native player-constructor evidence; see [MISSION_INPUT.md](MISSION_INPUT.md). `enabled` reports an optional value; the consumer-facing `accepts` gate errors on unknown defaults. This avoids silently enabling unconfigured actions. Forced scripted transfer is not conflated with a player's permission to request transfer.

World state holds an optional controller backend. Authored-startup audit mode now installs an explicitly fresh mask. The new digital input adapter consumes those masks when the original scripts call ReadControllerState, including a Bevy-software-input/original-script integration test. The complete interactive original-mission loop, human controller input and presentation remain unaccepted.

## Native evidence

RBWE01 `EnableControllerItem`, `0x800d91f4`:

- Reads two or three arguments: numeric item, numeric enable value, optional player (default zero).
- Resolves the selected player object through `0x80013a60`.
- Nonzero enable writes zero to the disable byte at player + item + 0x16fd; zero enable writes one.

The Rust adapter uses symbolic item identities rather than guessing engine numeric IDs and intentionally restricts numeric enables to the observed authored 0/1 domain. It does not emulate all Lua numeric coercions or an absent-player silent return. Source trace: `local/analysis/EnableControllerItem-native.txt`.

## Original execution

Unmodified `Gambit_Part_1_Controls_Disabled` now executes all ten authored disables and yields at EndFrame in an isolated initialized-program test. It disables transfer/HUD transfer/map transfer, next/previous unit and type selectors, map toggle and global toggle. In that original empty-backend fixture, Fire remains unassigned, not implicitly disabled or enabled. The later explicit fresh-player backend instead preserves its proven initial enabled state.

A ten-instruction budget interrupts after partial execution: VM/thread and controller state roll back. Retrying with a sufficient budget commits all ten settings. The test also verifies missing-backend rejection and leaves victory unset.

## Historical mask-slice validation

- 125 viewer-library tests, 42 integration tests (original-data tests enabled), 15 Python tests: **182 passed**.
- Formatting and warning-denying viewer/all-target Clippy passed.
- Locked battle/original-viewer/play builds and offscreen replacement battle restart regression passed.
- Logs: `local/analysis/mission-controller-*.log`; current isolated probe audit: `local/analysis/mission-controller-audit.json`.

First-mission completion checklist remains open. No rendered menu, original mission playthrough, hardware controller acceptance or original victory is established by this slice. Continue with native lifecycle, navigation/vehicle and runtime presentation blockers; retain the existing launcher until actual mission launch is supported.
