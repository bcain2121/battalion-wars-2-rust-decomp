# Current-mission profile queries and remaining native blockers

**First mission is still not playable.** This slice advances its real Gameflow script, not a replacement mission. No win/completion flag is forced.

## Implemented

- `World.mission_history` is an explicit, optional snapshot of the selected profile's current mission. Attempted and completed remain independent booleans. Missing history fails; it is not silently treated as a new profile.
- `GetCurrentMissionAttempted()` and `GetCurrentMissionCompleted()` read the snapshot and return Lua booleans. They do not alter runtime victory, objectives, save files or mission progress.
- `MissionHistory::fresh_attempt()` explicitly means entering the first mission with an unsaved new profile: attempted=true, completed=false. This is caller-selected replacement profile state, not recovered Wii save-file loading.
- `GetTutorialMode()` returns literal true because the inspected RBWE01 native implementation does exactly that. It is not an unsupported API stub or a guessed preference default. Other signatures remain rejected.
- `bw-mission-audit --authored-startup --fresh-attempt` explicitly opts into that fresh-profile snapshot; older audit modes remain unchanged. Output labels it `explicit_unsaved_fresh_attempt`.

## Native evidence

Same local DOL SHA256 as prior research: `9e557a6c9d316de3a3db8e9790b8d8c54ffc9cd1656dee89a90a2d7f964df87f`.

- GetCurrentMissionAttempted at `0x800defa0`: selected profile stride 0x1c0; current mission byte +4; tests its bit in mask +0xc for indices below 24, pushes boolean.
- GetCurrentMissionCompleted at `0x800df04c`: analogous test against mask +0x10. The adapter consumes already-resolved current-mission booleans; it does not guess the frontend-to-native mission-index mapping.
- GetTutorialMode at `0x800dd5d4`: checks zero arguments, loads r4=1 and calls the same boolean-push helper `0x8037b818`.

Traces are ignored local files: `GetCurrentMissionAttempted-native.txt`, `GetCurrentMissionCompleted-native.txt`, `tutorial-mode-native.txt` under `local/analysis/`.

## Actual original-script frontier

The earlier history-only milestone reached PC87 (unsupported Despawn). With the retained-despawn flag now preserved, that limited fixture reaches **PC153: GetPlayerUnit not configured**, and rolls back all prior changes including retention. The fully configured isolated audit now yields from Gameflow and four other formerly blocked first-resume probes; see [MISSION_DESPAWN_RETENTION.md](MISSION_DESPAWN_RETENTION.md). The intro separately remains blocked by waypoint/navigation, and shared startup still fails on ArtInitialisation Kill. Isolated yields are not simultaneous gameplay.

Historical history-query milestone validation: **179 tests passed** (123 viewer-library, 41 integration, 15 Python); formatting, viewer/all-target warning-denying Clippy, locked viewer builds and replacement battle restart regression passed. Logs: `local/analysis/mission-history-*.log`; audit: `local/analysis/mission-fresh-profile-audit.json`.

## Research retained for next work

### Despawn second argument

`0x800e9ed8` defaults the optional boolean to false. Unit event handler `0x8007d374` performs normal despawn then stores it at unit+0x3e1. Network forwarding at `0x801e10c4` preserves it. Consumers at `0x8003aca0`, `0x801a7930`, `0x80200298` admit an entity whose state field (+0x2f8) is zero when this flag is true. Correction: +0x2f8 is an integer state, not a presence pointer; the spawn implementation explicitly stores 1 at 0x800781ec, consistent with the earlier MISSION_STARTUP findings. Therefore **true cannot safely be ignored or collapsed into ordinary despawn**. The setter and shared presence term are now preserved explicitly; complete native list filters and presentation remain unresolved. See [MISSION_DESPAWN_RETENTION.md](MISSION_DESPAWN_RETENTION.md). Traces: `despawn-roster-native.txt` (initial research filename; actually network forwarding), `despawn-flag-consumer*.txt`.

### Player defeat

AllPlayerUnitsDead at `0x800d9f10` reads a maintained player-state byte +0x18ac (with a separate network path). It is not simply an inline count of same-army units. A setter at `0x800f5de4` marks it after `0x8007c320` fails to find a replacement unit. That search dispatches predicate `0x800758d8` through `0x80075808`: checks include presence, group/army field +0x340, health/type exceptions, additional flags/state, and virtual eligibility. Other setters/clearers exist. The fresh constructor explicitly stores zero at `0x800f18b4` (r30 is loaded with zero at `0x800f183c`). `configure_fresh_player_lifecycle()` now permits the known-not-defeated branch for a living, present authored player, without inventing a roster. Lethal damage, scripted zero health or player despawn invalidate that knowledge; healing/respawning alone cannot clear it. A supported different-target explicit transfer clears it, following `0x800e3398`. Automatic replacement and the terminal defeated transition still fail explicitly. The unchanged original WinLose thread now yields with an empty roster in a fresh live-player test, then fails transactionally after lethal damage rather than inventing a defeat. This is **partial recovery**, not a recovered roster or complete defeat implementation.

Traces: `all-player-units-dead-native.txt`, `player-dead-store-*.txt`, `player-next-unit-native.txt`, `player-next-unit-predicate.txt`. Paired-single instructions remain subject to Capstone misdecoding; no vector semantics were inferred from its incorrect later-ISA mnemonics.

### Kill

Follow-up asset inspection: all three ArtInitialisation targets (250009261, 250010226, 250010165) share sDestroyBase 250009278, ObjFlags=8194 and ExplodeType=250009268. That explosion profile has zero damage, no Tequila effect or boom sound, radius-80 orange effect lighting, duration field 10, flags=16, and self-referencing next-explosion pointers. Thus killing these objects invokes an effect-bearing destruction path, not merely hiding an unwanted light. Follow-up native recovery establishes that light duration is a counter of effect updates, not seconds: profile +0x74 is copied to instance +0x158 at 0x800ba5c4; 0x800bc49c..0x800bc4fc applies radius × remaining/initial, decrements once per update, and removes the light on a later zero-counter update. Effect radius/colour/duration reflection offsets are +0x6c/+0x70/+0x74. The engine's exact update-frequency mapping remains unverified. The native next-explosion delay is profile +0xb4; delayed spawning uses the separate +0xd0 timer and subtracts the engine delta at 0x800bcf00..0x800bcf14. Full chained-effect/rendering semantics are still incomplete. Flag 0x10 is EXPLODEFLAG_LIGHTOBJECTS (editor enum corroborated by native consumers), not a looping flag; its per-object illumination cannot be discarded in favor of only a generic point light. Local evidence: explosion-{light-reflection,constructor-native,update-native,delay-reflection}.txt. StaffFX's separate explosion profile 550035525 includes a sound and Tequila resource and cannot be handled as the same lighting-only case.

Additional native damage-gate evidence: profile finalization `0x800b9c20..0x800b9c80` sums the four damage amounts only for nonzero damage types and sets derived flag `0x800000` only for a nonzero sum. At `0x800bbbb0..0x800bbc00`, an unset bit branches directly to the per-object-lighting path `0x800bc1ec`, bypassing the damage and troop-force path (including the +0x40 force factor at `0x800bc0e8`). Thus the three zero-damage light profiles do not require an invented impulse just because their authored troop-force factor is nonzero. Reflection confirms damage types +0x14, damage amounts +0x24, wave time +0x34, max/min blast radius +0x38/+0x3c, troop force +0x40 and object-light colour +0x68. Radius getters `0x800bd564`/`0x800bd580` multiply the authored radius by instance scale fields; they do **not** prove a linearly expanding wave. Per-object illumination and cleanup still need implementation. Evidence: `explosion-core-field-reflection.txt`, `explosion-damage-flag-800b9c7c.txt`, `explosion-radius-getters.txt`.

Further lighting dispatch research: the callback at `0x800bc1ec..0x800bc27c` resolves virtual +0x58 then calls +0x16c on the returned object. Verified cUnit, cDestroyableObject and cSceneryCluster base vtables return self at +0x58 (`0x8000d464`) and a literal `blr` at +0x16c (`0x800ce694`). A scan finds 33 matching self-return candidate vtables, all with that same no-op lighting entry. This narrows the research: callback dispatch does not itself prove a visible per-object effect on these classes. Concrete derived/other target coverage still requires validation; do not generalize the scan to all targets or discard the separate effect-light channel. Local traces: `explosion-object-light-callback.txt`, `{unit-model,destroyable-light,scenery-light}-method-*.txt`, `light-object-vtable-candidates.json`.

Wrapper `0x800e9ca8` has distinct unit/destroyable/other branches; destroyable path `0x8009fb70` marks state and performs model/effect/attachment cleanup. A generic health=0 replacement alone would not recover all of these effects. Traces `kill-native.txt`, `kill-destroyable-native.txt` retained; no broad Kill support added.

## Remaining acceptance blockers

Original scene navigation and vehicle profiles, lifecycle flags/cleanup, controller/HUD adapters, player roster/transfer-on-death, full simultaneous script scheduling, hardware audio and rendered cinematics, and original end-to-end play/win/loss/restart remain unverified or incomplete. Native research has not made those capabilities available automatically. Main launcher still runs the replacement battle.
