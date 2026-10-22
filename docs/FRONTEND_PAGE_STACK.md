# Original frontend page-stack execution

## Completed slice

`src/frontend_stack.rs` executes the **unmodified PageStack Lua resource** from the extracted frontend archive using the shared transactional VM. It does not replace those functions with Rust navigation logic.

Supported operations: PushPageStack, PopPageStack, SetPageStack (replace), ResetPageStack and GetPageAtTop. Root initialization creates the original closures, shared captured stack table/counter, NextPage and PageDone globals. Calls are bounded by instruction budgets. Mutations and output validation commit together; errors leave the previous runtime intact. The adapter validates requested/produced page names against the authored catalog.

The original pop retains the bottom page. Reset resets the captured index and calls the original PushPageStack; it does not invent additional clearing behavior. NextPage's initial numeric zero is exposed as no pending page by the adapter.

## Shared VM change

GETTABLE/SETTABLE now support finite safe-integer numeric keys as well as string keys. Numeric keys remain distinct from strings (`1` versus `"1"`), signed zero aliases zero, nil deletes, and both key namespaces share a 10,000-entry/table budget. Table identity, closure captures and cloned transactional state remain shared across functions. The bootstrap setter uses the same helper.

Fractional/nonfinite/out-of-safe-integer-range keys and other key types fail explicitly. SETLIST, general Lua key semantics, metatables and coercion were not added by this slice. Follow-up: [FRONTEND_ARRAYS.md](FRONTEND_ARRAYS.md) adds SETLIST/SETLISTO and isolated original cursor-constructor tests.

## Evidence and scope

The local integration test initializes the original resource and invokes push through Start, LoadCampaignData, Save, MainPre and Main. **These names are supplied by the test: the test does not execute the actual title/profile callbacks or the complete menu flow.** It tests pop, replace, reset, bottom-page retention, unknown-page rejection and a six-instruction interrupted push followed by state comparison and further stack operations.

Synthetic tests cover numeric/string separation, key validation, nil deletion, combined limits and direct VM rollback after a numeric table write followed by an invalid opcode. Retrying with a valid return commits the write.

Validation:

- 123 viewer-library tests.
- 33 integration tests across mission startup, original assets/mission, frontend assets/graph/stack (original-data tests explicitly enabled).
- 15 Python tests.
- Formatting and warning-denying viewer/all-target Clippy.
- Locked battle, original-viewer and play builds.
- Offscreen replacement battle victory/restart/defeat regression retained: `BATTLE_RESTART restored_health_roster_positions=true` and `BATTLE_VERIFY_PASS mode=restart phase=Lost restart=true`.

Logs: `local/analysis/frontend-stack-*.log`. The integration aggregate predates the additional direct numeric rollback test; its separate execution log contains all four stack tests. Total current verified tests: **171**, without double-counting the stack rerun.

## Not completed

No rendered menus, physical input acceptance, page Open/Close/Update lifecycle, cursor behavior, profile persistence, prologue playback or mission launch. PageDone is only reported; it is not consumed by a fabricated transition controller. No authored victory or original mission completion is claimed. The primary launcher remains unchanged.

Next slice: connect actual page callback execution and lifecycle to these original stack requests, implementing required cursor/timing capabilities without permissive native stubs.
