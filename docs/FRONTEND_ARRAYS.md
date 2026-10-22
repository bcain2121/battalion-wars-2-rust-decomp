# Lua arrays for original frontend cursors

Completed bounded slice: the shared mission VM now implements Lua **5.0** SETLIST and SETLISTO. The existing restricted bootstrap interpreter is unchanged.

## Semantics and limits

The Lua 5.0 `lvm.c` and `lopcodes.h` references retained under `local/tools/` define 32-entry flush groups. Fixed SETLIST takes `(Bx & 31) + 1` values and writes numeric keys starting at `(Bx & ~31) + 1`. Open SETLISTO takes the actual open-call result count, then consumes the open result boundary. Writes run in descending index order, as in the reference interpreter. This is not the different Lua 5.1 B/C encoding.

All keys go through the same bounded table setter used by GETTABLE/SETTABLE. Missing registers, missing/invalid open result boundaries, non-table targets and table limits fail explicitly. A failed resume rolls back arrays, captured state, host state and the thread. No numeric loops, metatables or additional native APIs were introduced.

## Original-data verification

The test executes the complete original EntityInitialise bootstrap against the frontend XML graph, yielding real SpriteID object references. It then executes the unmodified, zero-capture GetMainCursor and GetCrosshairCursor child prototypes from Global in **isolated fixtures**. Both produce their original nested arrays: sprite reference, two-component offset and GetCursorRoll reference. The observed offsets are `(0,20)` and `(0,0)` respectively.

GetCursorRoll is declared but not implemented or invoked. Storing a callback reference does not establish cursor motion/rendering support. Global's complete root, page initialization, lifecycle and physical input have NOT been executed by this test. No script instructions are altered, but selecting isolated child prototypes is not full frontend integration.

## Validation

Five array tests cover fixed 32/33 flush boundaries, open multi-result and empty-result construction, consumed-top rejection, malformed sources, atomic rollback after a partial write reaches capacity, and the two original cursor constructors.

Current suite: **176 passing tests** (123 viewer-library, 38 integration, 15 Python). All original-data integration tests were explicitly enabled. Formatting, warning-denying viewer/all-target Clippy, locked builds and the offscreen replacement battle restart regression pass. Logs: `local/analysis/frontend-arrays-*.log`.

## Remaining work

The full original menu still is not rendered or interactive, and the first mission remains unfinished. A subsequent shared-runtime checkpoint now executes the complete Global root, Start constructor and authored Start A/B script callback; see [FRONTEND_RUNTIME.md](FRONTEND_RUNTIME.md). That does not change the isolated scope of the array tests above. Remaining work includes the other roots/initializers, actual FrontendBW2 driver, cursor/input/rendering, profile I/O and page lifecycle. The launcher remains unchanged.
