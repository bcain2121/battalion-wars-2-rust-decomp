# Bomber deployment and ordnance checkpoint

**Historical 311-test checkpoint.** The anti-air boundary below is superseded by [AMBUSH_COMPLETION.md](AMBUSH_COMPLETION.md): the original ambush now returns control. The later faded staff transfer also works; [STAFF_TRANSFER.md](STAFF_TRANSFER.md) records the current battle-waypoint boundary. Retained details/evidence below describe the earlier state.

## Accepted scope

The original SP_0.1 scripts now execute aircraft `Teleport` and the bomber's forced `AttackTarget`. Real recorded player input completes the barrel objective, reaches the camp, completes the authored camp-arrival objective (`550032489`), and enters the original ambush. No progress globals, script instructions, camera cuts or player positions were overridden.

The aircraft moves under an attack order and its heading is propagated to original model parts. The original cinematic/radio prefix renders. **The complete bombing scene and squad recruitment are not accepted.**

Current transactional boundary:

```text
Gambit_Part_1_Cutscene_Bombed pc 173: AttackTarget: AttackTarget ground weapons not configured
```

That caller is anti-air infantry, not another bomber. The two authored defenders are `550035415` and `550035416`, base `260015223` (`UNIT_MAN_ANTI_AIR`), weapon `50034190`, projectile `50034030`. Implement genuine missile/aircraft-contact behavior next; do not substitute rifle fire or skip their orders.

## Reusable engine behavior

- `mission_air::teleport` resolves aircraft physics/flight data, validates finite planar coordinates and heading, and applies terrain-relative cruise height. The binding accepts X/Z/heading and an optional **zero** placement margin. Ground teleport and nonzero clearance-search margins remain rejected.
- Teleport cancels movement/attack orders. World heading updates drive rendered aircraft rotation.
- `mission_bomber` reads the primary seat's weapon, flight speeds/turn rate/height, drop distance, firing cone/range, launch speed, burst interval, clip/reload, damage channels and 27-entry impact table. Unknown projectile/weapon modes and malformed data remain errors.
- Forced bomber orders currently support scenery/building targets, not direct unit/aircraft targeting. They steer, maintain terrain-relative altitude with source vertical speed bounds, release moving projectiles, and use scene collision plus authored direct/blast damage and retirement.
- Reissuing an order cannot refill ammunition or bypass reload. Weapon cooldown persists across canceled orders. Invulnerability remains effective.
- Public ordnance advancement stages its changes; the mission uses its existing whole-tick transaction. The anti-air boundary test checks frame, all positions, headings and release count rollback.

These are **explicit Rust replacement policies**, not native aircraft/projectile parity:

- Guided descending bombs use a fixed acceleration of 30 world-units/s² and source-bounded horizontal guidance. Flight is kinematic; no banking, native aerodynamics, full clearance search or water flight response is claimed.
- Terrain without an object owner uses the source **STONE** impact response. This is a declared uniform-land response, not recovered terrain material classification. Object contacts use their authored `mSurfaceType`; unknown object surfaces fail.
- Bombs have bounded capacity/lifetime checks. A still-live over-age projectile errors rather than silently discarding damaging behavior.
- Projectiles use procedural visible proxies. Dedicated native projectile/muzzle/impact presentation and hardware sound are not integrated; the mission requires cosmetic-gap opt-in and records that omission. Original models/scripts remain untouched.

## Native reference scope

Private disassembly: `local/analysis/teleport-native.txt`, `air-teleport-native.txt`, `air-teleport-implementation.txt`, `air-placement-predicate.txt`, `air-height-probe-native.txt`, `air-reference-height.txt`, and `teleport-placement-query*.txt`.

The wrapper at `0x800d4a18` admits 4–5 arguments and converts heading. Aircraft dispatch uses vtable slots `0x434` → `0x80059d3c` → `0x80107808`, and `0x438` → `0x80079638` for placement querying. The implementation accesses a flight height field at `+0x80`; source flight data supplies `mCruiseHAG`. The native reference-height/placement queries are **not fully ported**. Paired-single words must not be interpreted as Capstone's incorrect VSX instructions.

## Verification

- **311 viewer all-target tests passed**, including private assets, zero failed/ignored. **185 overlapping non-viewer library tests passed**; do not add the totals.
- Formatting, all-target Clippy with warnings denied, and viewer build passed.
- Isolated original-data ordnance fixture: **7 releases, 6 impacts**, damage from 100000 to **99000** before invulnerability; later impacts leave health unchanged. Burst/reload timing, repeated orders, malformed damage data and invalid-step rollback checked. The fixture deliberately isolates actors and adjusts target health; it is **not original mission progression evidence**.
- Original-route integration separately verifies camp arrival, aircraft deployment/flight and the unsupported anti-air rollback boundary. It depends on `local/analysis/first-combat-inputs.json`, generated by the first-combat integration test.
- Reviewed GPU captures: `local/analysis/bomber-cinematic-prefix-02.png` and `bomber-combat-regression-02.png`; both logs contain no ERROR/WARN lines. The prefix check reports 27 synchronized aircraft model parts. This is not full rendered bombing, missile combat, native camera-parity, animation or hardware-audio acceptance.
- Logs: `bomber-full-tests-03.log`, `bomber-core-tests-02.log`, `bomber-final-clippy-03.log`, `bomber-final-build-02.log`, and the matching GPU logs under `local/analysis/`.
- The full regression run caught a new timestep restriction leaking into strict/reference animation tests. Bomber advancement is now correctly limited to the opt-in rewrite, and the original animation regression passes.

Existing launchers and user-owned running previews were not stopped or replaced. Reopening **Preview First Mission Runtime.command** uses the rebuilt binary. Heavy weapons, full ambush/staff recovery, recruitment objectives, capture, player replacement, victory/defeat and whole-mission restart remain unfinished.
