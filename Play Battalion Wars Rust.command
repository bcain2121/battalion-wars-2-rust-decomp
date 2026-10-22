#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  printf 'Rust is required. Install rustup, then run this launcher again.\n'
  read -r -p 'Press Enter to close.'
  exit 1
fi
if [[ ! -f local/extracted/files/Data/CompoundFiles/SP_0.1.out ]]; then
  printf 'Local extracted assets are missing. See docs/PLAYABLE_BATTLE.md.\n'
  read -r -p 'Press Enter to close.'
  exit 1
fi
# Do not inherit automated replay/capture settings into a human play session.
unset BW_SMOKE BW_CAPTURE BW_VERIFY BW_OFFSCREEN BW_BATTLE_VERIFY BW_CAPTURE_RESULT
unset BW_PRESENTATION BW_PRESENTATION_VERIFY
unset BW_MODEL BW_ANIMATION BW_POSE_TIME BW_SCRIPT_PREVIEW BW_UNIT_PAIR
unset BW_SCRIPT_SMOKE_DEATH BW_SCRIPT_DAMAGE_FRAME BW_UNIT_COLLISION_DISABLED BW_PATHFINDING_DISABLED
unset BW_REQUIRE_ROUTE BW_REQUIRE_UNIT_CONTACT BW_REQUIRE_CAMERA_CLIP BW_CAPTURE_FRAME
# Explicit JSON arguments retain the older editable sandbox.
# Default: honest original-scene restoration preview, not the replacement battle.
if [[ $# -gt 0 ]]; then
  cargo build --locked --features viewer --bin bw-play
  exec target/debug/bw-play "$1"
fi
for asset in SP_0.1_Level.xml SP_0.1_Level_preload.xml SP_0.1_Level.res; do
  if [[ ! -f "local/extracted/files/Data/CompoundFiles/$asset" ]]; then
    printf 'Missing local original asset: %s\n' "$asset"
    read -r -p 'Press Enter to close.'
    exit 1
  fi
done
printf '\nOriginal battlefield restoration preview (native Rust / Bevy).\n'
printf 'Static scene inspection, not a playable original mission yet.\n'
printf 'WASD move view; arrows look; Tab next view; Q/E zoom; R reset.\n\n'
cargo build --locked --features viewer --bin bw-original-viewer
export BW_PRESENTATION=1
exec target/debug/bw-original-viewer
