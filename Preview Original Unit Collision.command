#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  printf 'Rust is required. Install rustup, then run this launcher again.\n'
  read -r -p 'Press Enter to close.'
  exit 1
fi
if [[ ! -f local/extracted/files/Data/CompoundFiles/SP_0.1_Level.res ]]; then
  printf 'Original local assets are required. See docs/UNIT_COLLISION.md.\n'
  read -r -p 'Press Enter to close.'
  exit 1
fi
unset BW_SMOKE BW_VERIFY BW_CAPTURE BW_CAPTURE_FRAME BW_MODEL BW_ANIMATION BW_POSE_TIME
unset BW_SCRIPT_SMOKE_DEATH BW_SCRIPT_DAMAGE_FRAME BW_REQUIRE_CAMERA_CLIP BW_REQUIRE_UNIT_CONTACT BW_UNIT_COLLISION_DISABLED BW_REQUIRE_ROUTE BW_PATHFINDING_DISABLED
export BW_SCRIPT_PREVIEW=1 BW_UNIT_PAIR=1
printf 'Original-unit collision and route-planning preview, not a complete mission.\nR: replay movement; P: pause; K: kill moving troop; arrows: orbit; W/S: zoom; Esc: quit.\n'
cargo build --locked --features viewer --bin bw-original-viewer
exec target/debug/bw-original-viewer
