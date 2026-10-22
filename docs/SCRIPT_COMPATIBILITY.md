# Script compatibility investigation

Date: 2026-10-02. Scope: SP_0.1 resource archive only.

## Current execution follow-up

Two original initializer bodies now complete under explicit host capabilities, and the original Pause_Scoring_Timer coroutine is tested across yields and controlled fixture states. General Lua calls/captures and mission gameplay remain incomplete. See [MISSION_EXECUTION.md](MISSION_EXECUTION.md). The bootstrap description below is the preceding milestone.

## Later bounded-bootstrap implementation

`src/lua50.rs` now reads these chunks directly in Rust. Its deliberately restricted evaluator runs only root/bootstrap chunks under a capability-defined host: `RegisterReflectionId` validates an existing decimal graph ID and produces a symbolic ObjectId. This does not assert the original pointer ABI. Unknown calls/opcodes fail closed, execution has finite budgets, and state changes commit transactionally. No filesystem/network/process access exists.

All 54 roots initialize successfully: 4,660 executed instructions, 741 ID resolutions, 42 entity tables and 53 function identities. No registered mission bodies, original coroutine scheduler or gameplay semantics have been executed. Evidence: `evidence/script-bootstrap.json`; details and remaining limitations: [ORIGINAL_RECOVERY.md](ORIGINAL_RECOVERY.md). The Python tool below remains static-only.

## Verified structural results

`tools/lua50_inventory.py` parses PRCS records as a little-endian u32 name length, name bytes, then a Lua 5.0 serialized chunk. It validates the observed header, numeric sentinel, bounded vectors/strings/nesting, constant/global indices, prototype indices, opcode range, debug line-table length, and exact end-of-chunk consumption. It never executes scripts.

All 54 chunks parse successfully:
- 135 total prototypes, including root chunks.
- 20,700 instruction words.
- 87 global names read but not assigned with SETGLOBAL anywhere in this sampled archive.

These 87 names are NOT all proven functions. They include data/constants and may have other initialization paths. Reads are NOT a call graph, and dynamically constructed global names are not captured by this method. Structural acceptance is not full VM verification.

The serialized code retains source paths and debug information. Full decoded code/constants stay under ignored `local/analysis/`; tracked evidence contains structural summaries and symbol names only.

## Executable linkage

`tools/map_script_bindings.py` searches populated DOL data sections for aligned adjacent big-endian pointers: one to an exact NUL-terminated name, followed by one into a populated executable text section.

84 of 87 candidate external names match this pattern. This strongly suggests script registration entries; registration code still needs tracing before calling it confirmed for every entry.

Examples (addresses are Wii virtual addresses, specific to this executable hash):

| Name | Candidate wrapper |
|---|---|
| EndFrame | 0x800d2528 |
| GetTime | 0x800d4c50 |
| WaitFor | 0x800d6fc4 |
| IsDead | 0x800e7e98 |
| AttackTarget | 0x800eb38c |

Machine-readable pairs: `evidence/first-mission-native-bindings.json`.

## Initial PowerPC observations

Capstone 5.0.7, PowerPC 32-bit big-endian, was used for bounded disassembly samples. These are windows, NOT established function extents; a tail branch can end a wrapper before the sample ends, and standard Capstone may not decode Wii-specific paired-single instructions.

- EndFrame loads r3 from r13-0x2c58 and branches directly to 0x800d13a8.
- GetTime loads a float from r13-0x37c4, calls 0x8037b488, and returns integer 1 in r3. Hypothesis: shared game clock pushed as one Lua result.
- WaitFor obtains a numeric value through 0x8037b1ec, adds the float from the same r13-relative location, writes it at +0x14 of the object obtained from r13-0x2c58, clears +0x18, and calls 0x800d13a8.
- Hypothesis: +0x14 is a wake deadline and 0x800d13a8 is a scheduler/yield path. Must inspect its callers/body and compare paused/unpaused gameplay before implementing semantics.

## Scheduler follow-up

Further disassembly of 0x800d13a8 shows a state value of 2 written at object offset +0x1c, zero placed in r4, a pointer loaded from +0x10 into r3, and a tail branch to 0x8037f8f8. That target references the exact diagnostics `attempt to yield across metamethod/C-call boundary` and `cannot yield a C function`. This strengthens identification of a Lua yield path; object layouts and state enum names remain provisional.

The common call at 0x800ee464 saves argument registers to its stack frame and immediately returns in this build. Do NOT infer working argument validation from callers setting r4 before this call. Actual argument checking must be traced elsewhere.

This does not establish scheduler resume order, clock units, pause behavior or a complete VM ABI.

## Level/preload linkage

`tools/object_graph.py` combines the sampled level and preload for inspection: 6,635 unique objects, no duplicate IDs, and 8,149 nonzero Pointer/Resource references, all resolving within that combined set. All 53 cGameScriptResource names match parsed archive scripts. The 54th archive script, EntityInitialise, has no such resource object in this sample; its bootstrapping role must be traced rather than assumed.

The three external names without candidate function-pointer pairs are GRADE, constant, and netvariable. This strengthens—but does not prove—the registration-table interpretation of the other 84 names.

## Runtime implications

1. Preserve Lua 5.0 semantics; a modern Lua binding is not automatically compatible.
2. Preserve coroutine scheduling and clock behavior, including pause handling, rather than replacing WaitFor with wall-clock sleeps.
3. Separate stable game IDs from Bevy Entity handles; scripts and XML use original numeric identities.
4. Keep mission-owned helper globals separate from engine-provided bindings. For example, a name read globally may also be implemented in another mission script.
5. Establish argument validation, return counts, invalid-ID behavior, side effects, and multiplayer authority rules for each binding.
6. Do not attempt to run mission bodies until their host API and safety/error behavior are defined. The later root-only bootstrap has its own narrow explicit safety contract above.

## Reproduce

```sh
python3 tools/lua50_inventory.py local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res.gz --output docs/evidence/first-mission-lua.json --local-decoded local/analysis/SP_0.1-decoded.json
python3 tools/map_script_bindings.py local/extracted/sys/main.dol docs/evidence/filesystem-inventory.json docs/evidence/first-mission-lua.json --output docs/evidence/first-mission-native-bindings.json
.venv/bin/python -m unittest discover -s tests -v
```

## References consulted

- https://www.lua.org/source/5.0/lundump.c.html — serialized prototype order/header.
- https://www.lua.org/source/5.0/lundump.h.html — numeric sentinel/version.
- https://www.lua.org/source/5.0/lopcodes.h.html — opcode order and bit layout. Critical: Lua 5.0 places A in bits 24–31, unlike later layouts.

## Next bounded slice

- Resolve level + preload object references and script-resource identity.
- Trace native registration and scheduler routines; maintain confidence levels.
- Implement a first Rust original-format parser with synthetic tests, followed by terrain/model visualization; no fabricated replacement content.
