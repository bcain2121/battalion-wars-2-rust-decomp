# Original ambush: functional completion checkpoint

**318-test checkpoint.** The default-transfer boundary below is superseded by [STAFF_TRANSFER.md](STAFF_TRANSFER.md): hidden ownership transfer works, while the staff cutscene still blocks on a subsequent battle waypoint order. Ambush completion remains accepted; staff recovery does not.

## What now works

Recorded movement from the original barrel objective reaches camp and runs the unmodified bomber ambush through control return. The original scripts issue the attack orders, kill/despawn their authored casualties, change cameras, reveal the staff, expose the staff-recovery objective, and unfreeze the player. No scene was skipped, no objective/progress variable was forced, and no player relocation was used to shortcut the route.

The integration run finishes the ambush at approximately **101.10 game seconds** with **14 missile launches / 14 aircraft contacts** and **4 bomb releases / 4 impacts**. The next marker is `550041658`, at `[96.706, 0.25, -74.3]`. The player's location remains `[109.59683, 0, -153.18692]` through the cinematic's authored facing reset. Restart clears the new missile/bomb state and returns the initial session.

This is functional script/ordnance progression, **not complete native cinematic, animation, sound, flight or weapon-balance parity**.

## New reusable engine support

- Forced defensive anti-air infantry orders load the original NPC weapon/projectile data rather than substituting rifle fire. Aircraft hit volumes come from yaw-oriented rendered bind-pose bounds. The degenerate `SFXdummyRoot` effect carrier is not treated as an aircraft hull; targeting a missing hull still fails.
- Missiles launch, accelerate after their thrust delay, steer with a bounded turn-rate ramp, collide with scenery/terrain/aircraft, apply direct and explosion damage, and airburst on expiry. Repeated orders do not reset ammunition/reload. Disabled or despawned shooters cannot launch new rounds; existing projectiles continue.
- Forced orders bypass autonomous-AI sleep, **not** explicit weapon disabling. The original defenders have authored AI-disabled flags, which is distinct from `EnableWeapon(false)`.
- `EnableWeapon(unit, boolean)` updates weapon enablement without changing presence or AI activity. Player rifle fire, NPC rifle fire, bomber release and anti-air release honor it. Native wrapper reference: `0x800ec480`, with enable/disable calls to `0x8014499c` / `0x80144958`; private disassembly in `local/analysis/EnableWeapon-native.txt`.
- Scripted ground `Teleport` supports a troop's **in-place heading reset only**, in addition to the existing aircraft deployment. It validates unchanged X/Z and never moves a troop. General ground relocation and nonzero placement-margin searches remain errors. The original ambush itself requests this in-place player reset.
- Missile advancement is staged; malformed profiles and invalid time steps cannot partially commit. The existing whole-script-tick transaction remains authoritative.

### Explicit Rust policies

This is not a port of the complete native projectile solver. Launch velocity, thrust delay, acceleration, turn/ramp, lifetime, clip/cadence/reload, channels and impact tables come from source data. `mDrag` is interpreted as exponential velocity damping per second. Muzzle direction is aligned directly to the current target; the muzzle is a two-unit ground-relative proxy. Aircraft collision uses yaw-oriented bind bounds, not animated/native physics nodes or continuous moving-body collision.

Source damage channel amounts use the existing resistance/linear-blast backend, including the authored `DAMAGE_PRECALCULATED` channel. Native weapon-effectiveness tiers, flight heating, probabilistic evasion/ricochet, animated firing alignment and water behavior are not reproduced. Contacts detonate deterministically. Ownerless land uses the already-declared STONE response; object surfaces use their authored impact-table entry. Unknown weapon modes/classes/signatures still fail.

Projectiles use procedural visible proxies. Dedicated smoke/muzzle/impact presentation, full animation and hardware sound remain cosmetic gaps requiring explicit opt-in and recorded warnings. The original assets and scripts are unchanged.

## Next boundary: staff-player handoff

Walking onward another 70+ units through actual input triggers the original staff-recovery cutscene. It stops transactionally at:

```text
Gambit_Part_1_Cutscene_StaffBearer pc 327: ForceUnitTransfer: Unsupported gameplay call/signature: ForceUnitTransfer
```

This call is `ForceUnitTransfer(PLAYER_ONE, Sstaffbearer01)`, with two arguments. The existing backend accepts the explicit third-argument `true` hard-cut variant. Native wrapper `0x800e3100` accepts 2–4 arguments: omitted options leave transfer flags at `1`, whereas a true third option adds `0x14`. The default camera/auxiliary-notification path must be resolved, not silently treated as the already-supported hard cut. See retained `local/analysis/force-transfer-native.txt` and transfer implementation references.

`tests/first_mission_staff.rs` asserts this exact error and unchanged frame, player ownership and positions. Its passing result is **known-boundary/rollback acceptance, not staff recovery or recruitment completion**. Later staff gameplay, squad objectives, heavy weapons, capture, player replacement, victory/defeat and whole-mission completion remain open.

## Verification and replay

- **318 viewer all-target tests passed**, including private assets, zero failed/ignored.
- **188 overlapping non-viewer library tests passed**; do not sum the counts.
- Formatting, all-target Clippy with warnings denied, and viewer build passed.
- Original-route tests separately cover ambush completion/restart and the staff-handoff boundary.
- Isolated original-data arenas cover damage, invulnerability, repeated orders, clip/reload timing, explicit disarming/re-enabling, cover, target despawn, expiry/airburst, invalid-step rollback and malformed-mode rejection. One protected aircraft takes **218** damage before invulnerability; subsequent impacts do not lower health. These arenas intentionally reposition actors/change health and are **not mission-progression evidence**.
- Full GPU replay prerolls 4,459 inputs, renders the last 1,800 inputs and continues briefly after the recording. **1,284 rendered frames occur inside the original ambush**, then control returns. This is no longer just a two-second cinematic prefix.
- Reviewed captures include the original staff-bearer/staff reveal at frames 1450 and 1550, and the staff objective with restored player control. First-shot aircraft framing/visibility is not accepted against a Wii reference; sampled wide shots remain dominated by sky/terrain. Animation, native blast/trail presentation and hardware audio remain unaccepted.
- GPU logs have no ERROR/WARN/BLOCKED lines. Declared cosmetic warnings still exist in runtime state/HUD.

```sh
cargo test --locked --features viewer --all-targets -- --include-ignored
cargo test --locked --lib
cargo clippy --locked --features viewer --all-targets -- -D warnings
cargo fmt --check

BW_OFFSCREEN=1 BW_SMOKE=1 \
BW_MISSION_REPLAY="$PWD/local/analysis/ambush-complete-inputs.json" \
BW_MISSION_REPLAY_TAIL=1800 BW_MISSION_EXPECT_AMBUSH=1 \
BW_CAPTURE="$PWD/local/analysis/ambush-review.png" BW_CAPTURE_FRAME=1840 \
BW_CAPTURE_FRAMES=600,900,1450,1550 \
target/debug/bw-first-mission
```

Replay tail defaults to 120 and is bounded to 120–3600 inputs; normal launch behavior is unchanged. Optional capture series is bounded to 16 frame numbers. Evidence/log paths are in `docs/evidence/ambush-completion-validation.json`. Recordings depend on the earlier tests: first combat → ambush → staff boundary. Existing primary launchers and user-owned previews were left untouched.
