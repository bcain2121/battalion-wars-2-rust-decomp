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
  if [[ -d extracted/files/Data ]] && [[ ! -d local/extracted/files/Data ]]; then
    printf 'Found extracted/files/Data — copying into local/extracted/files/...\n'
    mkdir -p local/extracted
    cp -R extracted/files local/extracted/
  elif [[ -d DATA/files/Data ]] && [[ ! -d local/extracted/files/Data ]]; then
    printf 'Found Dolphin DATA/files/Data — copying into local/extracted/files/...\n'
    mkdir -p local/extracted
    cp -R DATA/files local/extracted/
  fi

  WBFS_PATH=""
  for candidate in \
    "Battalion Wars 2 (USA)/Battalion Wars 2 (USA).wbfs" \
    "Battalion Wars 2 (USA)/Battalion Wars 2 (USA).iso" \
    *.wbfs \
    *.iso; do
    if [[ -f "$candidate" ]]; then
      WBFS_PATH="$candidate"
      break
    fi
  done

  if [[ ! -d local/extracted/files/Data ]] && [[ -z "$WBFS_PATH" ]] && command -v osascript >/dev/null 2>&1; then
    printf 'No ROM found in project folder — opening file picker...\n'
    WBFS_PATH="$(osascript -e 'try' -e 'POSIX path of (choose file with prompt "Select your Battalion Wars 2 (.wbfs or .iso) ROM file:")' -e 'on error' -e '""' -e 'end try' 2>/dev/null || true)"
  fi

  if [[ ! -d local/extracted/files/Data ]] && [[ -n "$WBFS_PATH" ]] && [[ -f "$WBFS_PATH" ]]; then
    printf 'First-run setup: extracting assets from "%s" into local/extracted/files/...\n' "$WBFS_PATH"
    PYTHON_BIN="python3"
    if [[ -x .venv/bin/python ]]; then
      PYTHON_BIN=".venv/bin/python"
    elif ! python3 -c "import cryptography" >/dev/null 2>&1; then
      printf 'Setting up Python virtual environment (.venv) for ROM extraction...\n'
      python3 -m venv .venv
      .venv/bin/pip install --quiet -r requirements-research.txt
      PYTHON_BIN=".venv/bin/python"
    fi
    "$PYTHON_BIN" tools/inventory_filesystem.py "$WBFS_PATH" \
      --extract-all \
      --extract-root local/extracted/files || true
  fi

  if [[ -f local/extracted/files/Data/CompoundFiles/SP_0.1.out.gz ]] && [[ ! -f local/extracted/files/Data/CompoundFiles/SP_0.1.out ]]; then
    printf 'Decompressing .gz compound and startup archives in local/extracted/files/Data/...\n'
    find local/extracted/files/Data -name '*.gz' -print0 | while IFS= read -r -d '' gzfile; do
      stem="${gzfile%.gz}"
      if [[ ! -f "$stem" ]]; then
        gzip -dc "$gzfile" > "$stem"
      fi
    done
  fi
fi

if [[ ! -f local/extracted/files/Data/CompoundFiles/SP_0.1.out ]]; then
  printf '\nLocal game assets are not yet set up (Bring Your Own Disc/ROM).\n'
  printf 'To play, drop your Battalion Wars 2 (USA) [RBWE01] .wbfs or .iso ROM into this folder\n'
  printf '(or select it in the file picker when prompted) and run this launcher again.\n'
  printf 'See README.md for details.\n\n'
  read -r -p 'Press Enter to close.'
  exit 1
fi
# Do not inherit automated replay/capture settings into a human play session.
unset BW_SMOKE BW_CAPTURE BW_VERIFY BW_OFFSCREEN BW_BATTLE_VERIFY BW_CAPTURE_RESULT
unset BW_PRESENTATION BW_PRESENTATION_VERIFY
unset BW_MODEL BW_ANIMATION BW_POSE_TIME BW_SCRIPT_PREVIEW BW_UNIT_PAIR
unset BW_SCRIPT_SMOKE_DEATH BW_SCRIPT_DAMAGE_FRAME BW_UNIT_COLLISION_DISABLED BW_PATHFINDING_DISABLED
unset BW_REQUIRE_ROUTE BW_REQUIRE_UNIT_CONTACT BW_REQUIRE_CAMERA_CLIP BW_CAPTURE_FRAME
unset BW_MENU_SMOKE BW_SKIP_MENU BW_LEVEL BW_REPLAY
# Explicit JSON arguments retain the older editable sandbox.
if [[ $# -gt 0 ]]; then
  cargo build --locked --features viewer --bin bw-play
  exec target/debug/bw-play "$1"
fi
for asset in SP_0.1_Level.xml SP_0.1_Level_preload.xml SP_0.1_Level.res Frontend2_Level.xml Frontend2_Level_preload.xml Frontend2_Level.res; do
  if [[ ! -f "local/extracted/files/Data/CompoundFiles/$asset" ]]; then
    printf 'Missing local original asset: %s\n' "$asset"
    read -r -p 'Press Enter to close.'
    exit 1
  fi
done
printf '\nBattalion Wars 2 — Native Rust / Bevy Port\n'
printf 'All 20 Campaign Missions (SP_0.1..SP_5.4) + All 16 Multiplayer Maps (MP1..MP17)\n'
printf 'Controls: WASD move, Mouse/Arrows aim, LMB/Space fire, RMB/Tab lock-on, C/G/X squad, T transfer, Esc pause.\n\n'
cargo build --locked --features viewer --bin bw-first-mission
exec target/debug/bw-first-mission
