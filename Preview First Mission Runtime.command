#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")"
if [[ -f "$HOME/.cargo/env" ]]; then source "$HOME/.cargo/env"; fi
exec cargo run --locked --features viewer --bin bw-first-mission
