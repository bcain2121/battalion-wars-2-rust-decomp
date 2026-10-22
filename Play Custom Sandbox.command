#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
exec './Play Battalion Wars Rust.command' mods/sandbox/level.json
