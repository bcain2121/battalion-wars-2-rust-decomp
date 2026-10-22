# Native spatial cell traversal

`src/mission_spatial_cell.rs` implements the typed cell-traversal layer at 0x8032b460..0x8032b770. It does **not** construct registrations, decode native memory, dispatch the outer grid, or install a production Kill backend.

Recovered behavior:

- Reject cells whose aggregate category mask does not intersect the query mask.
- Visit branch children in low-x/low-z, high-x/low-z, low-x/high-z, high-x/high-z order.
- Clip child traversal against the XZ query rectangle expanded by the query radius plus the supplied tree-level maximum record radius. Rectangle edges are inclusive.
- Visit matching leaf records in stored order, retaining duplicate identities.
- Use the strict sphere-distance comparison; exact tangency is excluded.
- Increment callback count before invoking the callback. A false result stops traversal immediately and returns the complemented accumulated count, including that final callback.
- Do not inspect subsequent records after a stop, or sphere data of category-masked records.

## Arithmetic verification

The leaf paired-single words must not be interpreted using Capstone's modern-vector mnemonics. Operand decoding of 0x104200b2 gives `ps_mul f2,f2,f2`; 0x10021094 gives `ps_sum0 f0,f2,f2,f2`; 0x102300dc gives `ps_madds0 f1,f3,f3,f0`. The low lane computes rounded x²+y² followed by fused z*z+sum. The following `mfcr`/rotate-mask selects CR0 LT, hence `<`, not `<=`.

The Rust implementation uses explicit `f32::mul_add` for the final fused term. It rejects non-finite inputs, negative radii and arithmetic overflow as unsupported rather than guessing native NaN/overflow behavior. Finite normal arithmetic is the intended subset; PPC exceptional/denormal behavior is not independently accepted.

Depth 32 and work 1,000,000 are explicit implementation safety bounds, not native limits. A typed leaf rejects counts above the nonnegative signed-byte range. This is not a validator for native leaf allocation sizes. Callback mutations must be enclosed in the caller's transaction; this module only reads the tree.

## Tests and remaining work

`tests/spatial_cell.rs` covers child/record order, duplicates, complemented stop counts, lazy mask/stop behavior, strict tangency, inclusive child boundaries, expansion and invalid geometry. These are synthetic structural tests, not whole-grid comparison against the native captures.

The outer three-layer grid walk and two-grid manager dispatcher are now implemented in `mission_spatial_grid`. The manager checks mode bit 1 first (+0x4c11c), then bit 2 (+0x28034), and ignores higher mode bits. Importantly, it **decodes** an inner complemented early-stop count and returns a normal count; it does not propagate the complement. Tests cover cross-grid ordering, duplicate identities, early stopping before an unavailable second grid, disabled-grid laziness and missing selected-grid errors.

Live membership and registration mutations remain to be implemented. `mission_spatial_removal::remove_using_grids` now connects the typed dispatcher directly to the destruction neighbor filter and cleanup sequence. It snapshots immutable grids from a transaction-local backend, resolves record identities through that backend, stops at the 31st eligible candidate, then performs volume removal and owner/neighbor byte writes. The entire backend commits only on success. Missing selected grids or unknown record ownership fail rather than masquerading as empty space.

Integration tests exercise the actual grid/cell/filter path with synthetic registrations and a mock volume/instance backend, including duplicates, unread unknown records after capacity stop, and injected failures at every backend event. A production world-registration loader and concrete volume backend are still absent; this is not original-mission startup acceptance. The saved grid images alone do not establish all pointed-to leaf contents. Do not substitute authored IDs for live record identity or claim that this implements the original scene's spatial backend.
