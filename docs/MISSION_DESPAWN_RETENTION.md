# Retained, unspawned units

This extends the existing **living mobile-unit/building** lifecycle subset. It does not complete death effects, physical teardown, ownership replacement or all native list consumers.

## Recovered behavior

- Unit constructor `0x80076b0c` sets r28 to zero at `0x80076b38`; `0x80076f14` initializes unit byte `+0x3e1` from it.
- Despawn event handler `0x8007d374` calls the ordinary teardown at `0x8007d408 -> 0x800784e4`, then copies event byte `+4` into unit `+0x3e1` at `0x8007d414` and returns. The optional flag does not bypass physical despawn.
- Teardown clears the integer presence state `+0x2f8`. It is not a pointer.
- Three inspected list consumers admit nonzero presence **or** the retained flag before their other filters. This does not make a retained unit physically present, active AI, player-owned, alive or eligible for replacement.

Local traces: `despawn-{native,unit-implementation,event-handler-complete,flag-initialization,flag-store-8007d414,flag-consumer,flag-consumer-2,flag-consumer-3}.txt` under ignored `local/analysis/`.

## Runtime scope

`World.despawn_retention` preserves the flag separately. Explicit authored unit initialization supplies the recovered false constructor value for supported classes. `Despawn(id)` means false; `Despawn(id, boolean)` records the supplied flag after the existing validated cleanup. Extra arguments, numeric/string flags, dead targets and unsupported lifecycle classes still fail. Spawn with an extra boolean is not accepted. Capture points now have a separate identity set: the old generic building set also contained them, which could accidentally admit default Despawn without recovered capture cleanup. Both Spawn and Despawn now fail explicitly for capture points, and no default retention byte is supplied for them.

`unit_list_presence` exposes only the recovered shared presence term. Unknown presence/retention and unsupported entity types fail. It is **not** a completed roster, health-total, army, type or replacement query. Player replacement remains explicitly unresolved after losing the current player; retention does not invent a squad or terminal defeat.

## Evidence, not mission acceptance

The unmodified MiniBattles coroutine now reaches its first yield with the original heavy tank physically absent, retained for the shared list term, and unchanged health/activity. The profile-only Gameflow fixture now stops at PC153 `GetPlayerUnit` instead of PC87 Despawn; rollback restores the new flag map too.

The configured isolated audit advances five formerly blocked first-resume probes to `yielded`: Gameflow, MiniBattles, Cutscene_MidGame_1_Tank, Cutscene_MidGame_3_Gunship and Cutscene_StaffBearer. Gameflow_2 now correctly fails earlier at PC6 on capture-point Despawn instead of reaching PC15 after implicitly accepting that unsupported cleanup; an original-data regression test verifies rollback. These are isolated startup probes, **not completed sequences** or a running mission. Shared startup still fails transactionally at `ArtInitialisation pc 3: Kill`. Original intro movement, effects, rendering, ownership and authored victory remain incomplete.

Logs: `mission-retention-{unit-tests,session-tests,audit}.log`, `mission-retention-audit.json`. The earlier failing development logs are retained separately and are not validation passes.
