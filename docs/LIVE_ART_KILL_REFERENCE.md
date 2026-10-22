# Live Art Kill reference — 2026-10-08

## Scope

Local Dolphin 2609 / original RBWE01 is a **reference**, not the Rust runtime. The original title, fresh save, campaign prologue and first mission were reached through controller inputs. No program-memory/register writes or cinematic skips were used. Private captures remain under `local/analysis/native-reference-debug-20261008/`.

JIT debugging was enabled explicitly (`-d`, `Interface.DebugModeEnabled=True`). A breakpoint at 0x802c86d4 was verified and removed before the mission. Earlier sessions' missed breakpoints establish nothing about whether the original calls Kill.

## First Art object: complete entry/exit observations

At 0x8009fb70, caller 0x800e9e58, the concrete vtable was 0x804aeb90. Entry instructions matched the verified DOL. Object +0x0c was 250009261; its translation matched authored `Mat` (the first capture helper incorrectly searched `mMatrix`, so its empty position-match list is not a position mismatch).

| Field | Entry | Exit at 0x8009fcfc |
|---|---:|---:|
| Main flags +0x14 | 0x02000000 | 0x02100000 |
| Spatial byte +0x75 | 0 | 1 |
| Component byte +0xa0 | 0 | 1 |
| Lifecycle byte +0x130 | 5 | 1 |

These were the only changed bytes in the captured 0x220-byte owner span. Entry had instance flags zero, null record head, null auxiliary, zero attachments, non-null model with flags zero, and the expected tiny model-resource sphere. This is a live observation, not proof of universal constructor/load invariants.

The removal query visited one candidate, authored ID 550034232, with main flags 0x02000000. Its model flags were 0x80000008. The owner spatial byte became 1; the candidate byte remained 0, consistent with the recovered model-bit gate in virtual +0x1a4. Both mode-specific grid spans were captured; the complete native traversal backend is still unimplemented.

Explosion spawn 0x800bd5b0 received the captured profile, owner's transform, owner pointer, and zero r6/r8/r9. Integer RNG cursor changed from 0 at the first Kill entry to 1 at the second; attribution of that draw is not yet established. Do not assign it to delayed child selection without a verified call trace.

## Owner callback correction in Rust

The old destruction backend received an immutable owner view and could not report a callback changing that owner's component byte. Native explosion setup at 0x800ba504..0x800ba544 obtains the owner's component and invokes its virtual +0x4c. Live vtable reads resolve these to:

- Owner +0x4c → 0x800a1840, returning owner +0x88 for non-null owner.
- Component +0x4c → 0x8000f1d8, ORing bit 0 at component +0x18 (owner +0xa0).

`mission_destruction::EffectOutcome` now permits a concrete explosion backend to report `ActivateOwnerComponent`. The kernel applies it before marking DEAD and before tail cleanup. It is not inferred merely because an explosion profile exists. Unsupported effect/outcome combinations fail, and later failures roll back both owner and backend.

Tests cover ordering, the observed owner scalar changes, preserving unrelated bits, and rollback. They use a mock backend: **not full native replay, production explosion creation, or successful shared startup**.

## Incomplete observations and preserved state

The second Art entry identifies 250010226 and independently matches its authored position. It also starts with lifecycle 5, main flags 0x02000000, zero instance/component flags and no records/auxiliary/attachments. Its partial spatial trace visited IDs 250008347, 250008348, 250007844, 550031244 in that order. Subsequent expected breakpoints were missed; the reason remains unresolved. The trace is incomplete, not evidence of missing original calls or an exhaustive candidate list.

A probe labeled `art1-explosion-constructor-entry.json` actually stopped at **0x800ba038 inside constructor setup**, not function entry. Its r3 is not the effect pointer, and its LR belongs to an earlier nested call. True constructor entry is 0x800b9fa4. Do not use that file as allocation/caller evidence.

After the missed stops, the owned emulator was explicitly interrupted and verified paused at 0x802c86d4. The first object's full entry/exit snapshots, second entry, partial trace and correction notes are preserved. Do not replay actions blindly; inspect session status and breakpoint behavior before further capture.

## Validation

- `cargo test --locked --lib mission_destruction`: 8 passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed (default features).
- `cargo fmt --check`: passed.
- Viewer-feature library suite: 176 passed; destruction/startup integrations including original-data tests: 13 passed.
- Viewer-feature all-target Clippy with warnings denied: passed.
- Logs: `local/analysis/live-kill-owner-{tests,clippy,viewer-tests,integration-tests,viewer-clippy}.log`.

Original shared startup remains blocked on the absent production `Kill` backend. No first Rust gameplay tick or authored victory is claimed.
