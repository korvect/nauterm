#!/usr/bin/env bash
set -euo pipefail

client="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
repository="$(CDPATH= cd -- "$client/.." && pwd)"
if [[ "$(uname -s)" == Darwin ]]; then
  bundle="$(bash "$client/scripts/bundle-macos.sh" debug)"
  # Run inside the app bundle so AppKit finds its identity and compiled icon.
  # Keep the terminal attached for logs and Ctrl-C, as with cargo run.
  exec "$bundle/Contents/MacOS/nauterm-rs" "$@"
fi

bash "$client/scripts/build.sh" >&2
exec cargo run --manifest-path "$repository/Cargo.toml" -p nauterm-rs --locked -- "$@"
