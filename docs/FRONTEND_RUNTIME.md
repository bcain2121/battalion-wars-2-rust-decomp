# Shared frontend execution checkpoint

This is **not a working menu or original boot acceptance**.

`frontend_runtime::Runtime` now executes the unmodified EntityInitialise, PageStack and complete Global roots in a shared VM. It executes an entire page constructor and getter, preserving shared closure/table identities. The constructor's owner is the XML-linked initialization entity, not the GUI page object. Duplicate scripts/construction and unknown pages fail explicitly.

The original Start constructor **and full Start.Open callback now succeed**. Open binds the crosshair sprite through its authored descriptor (script sprite 2144913074 → sprite 2144913068). Its XML `eGUIEvent_OnAB` script callback then invokes the original `gotoMain` closure and PageStack and produces `LoadCampaignData`. The test starts with an empty stack; it executes Open but does **not** pretend that rendering/input/profile loading or the cold-start route has occurred. `dispatch_page_callback` executes only the script part of the event; input gating, optional event sounds and lifecycle belong to its caller. Failed constructors/callbacks roll back the combined state.

## Native inputs

- `GetNumOfGameType`, wrapper 0x800d6bf0, reads catalog per-type count at +0x228 with stride 0x82. Counts retain repeated entries. Original Frontend2 count is 9.
- Numeric GAMETYPE table at 0x8048d9a8: Frontend 0, Frontend2 1, SinglePlayer 2, Skirmish 3, Assault 4, Coop 5, MiniGame 6, Unknown 7.
- Numeric GAMESTATUS table at 0x8048d4a0 is retained, including separate bit flags. These are recovered numbers, not guessed aliases.
- `GetFramesPerSecond`, 0x800df1cc, converts the f32 at r13-0x7a20 to unsigned integer before returning a Lua number. The caller supplies a finite positive rate; test rate 60 is explicit, not recovered live-session timing.
- `EnableControllerErrorMessages` now stores the inverted Lua-truthiness state and returns the prior enabled state. The lazy singleton constructor 0x8024c870 writes disabled=1; `Environment::new` models a fresh environment, not imported mid-session state. Numeric zero and empty strings are true. Later script failure rolls back the setting.
- `GetCursorRoll` remains declaration-only and fails closed.

## Sprite table bindings

`frontend_sprites` implements bounded OpenSprite/CloseSprite state handling, not rendering. Native OpenSprite (0x800db804 → 0x80192f48 → 0x80192eb4) resolves descriptor element 1, checks the table's `objectPtr` light-userdata cache, resolves cScriptSprite +0x18, resets the sprite's raw +0x70 byte and +0x5c float to zero, and caches the pointer. Runtime handles are scoped identities, distinct from authored object IDs; mutable state is deeply cloned.

CloseSprite (0x80193704) clears only that table's pointer to light-userdata zero. It does not delete the shared sprite or reset its fields. Cached reopen does not reset; close/reopen does. Zero light userdata is distinct from nil and remains Lua-truthy. General userdata keys/metatables and function/string descriptor evaluation remain unsupported. Missing resources fail explicitly; a known null mSprite is distinct from unknown data. RenderSprite and native sprite animation/material rendering remain unimplemented.

Original-asset tests sweep callback instruction budgets to verify failures leave sprite state and stack unchanged, then verify Start.Open completes and the authored event reaches LoadCampaignData. An initial assertion incorrectly targeted the hand cursor; decoded Open upvalues and the actual descriptor show that it opens **revConCrosshair**. That test was corrected, not the original script.

## Cold-start correction

The unmodified `FrontendBW2` body selects **Logo**, not Start, on its cold-start branch: PCs 525–527 call `ResetPageStack("Logo")`. It subsequently drives OpenPage, UpdatePage, RenderPage, ClosePage and RunLevel. Its getter table references pages initialized elsewhere; do not replace it with a hand-written Start-to-game route.

Logo requests the original startup THP and then resets to StartPre based on movie frame count. StartPre proceeds to Start. The new cinematic decoder/player is separately tested; it is **not wired to these callbacks yet**.

## Further recovered facts, not implemented APIs

- VIDEO table 0x8048d540: OFF 0, ONCE 1, LOOPED 2.
- PlayVideo wrapper 0x800d5654 sets mode and optionally the asset name; it does not consume its third argument in the recovered wrapper.
- IsVideoFinished 0x800d5740 tests the mode word at 0x804fcd54 for zero. Do not substitute a guessed timer.
- GetTotalFrames/GetFrameNum read movie object +0x24/+0x20 via r13-0x8f0.
- Controller-error policy state and cold initialization are now implemented above; actual controller-error dialog presentation remains unimplemented.
- Native global cos 0x800da98c multiplies a double input by the degree conversion constant, rounds radians to f32, calls cosine and rounds the output to f32. A plain double-precision radians cosine is not its exact arithmetic path.
- GetNumGlobalStrings calls 0x800188e0, reading the first string-bank count. Its population must be recovered/configured; do not hardcode zero merely to unblock GStringID.

## Next integration work

Load the remaining original roots and initialization bindings, install the real font/string/profile/video/UI capabilities, execute FrontendBW2, render/evaluate authored widget properties, connect input and event sound dispatch, and preserve Logo → StartPre → Start → load/profile/campaign/prologue → Game. The mission backend still rejects ArtInitialisation Kill and vehicle movement.

Private traces and original bytes remain under ignored `local/`. No asset redistribution claim is made.
