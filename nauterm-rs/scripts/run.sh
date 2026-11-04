#!/usr/bin/env bash
set -euo pipefail

client="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
repository="$(CDPATH= cd -- "$client/.." && pwd)"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # Keep the MSVC/OpenSSL environment in the same process that launches RS.
    MSYS2_ARG_CONV_EXCL='*' powershell.exe -NoProfile -ExecutionPolicy Bypass \
      -File "$(cygpath -w "$client/scripts/windows.ps1")" -Mode Run "$@"
    exit $?
    ;;
esac
if [[ "$(uname -s)" == Darwin ]]; then
  bundle="$(bash "$client/scripts/bundle-macos.sh" debug)"
  # Run inside the app bundle so AppKit finds its identity and compiled icon.
  # Keep the terminal attached for logs and Ctrl-C, as with cargo run.
  exec "$bundle/Contents/MacOS/nauterm-rs" "$@"
fi

bash "$client/scripts/build.sh" >&2
exec cargo run --manifest-path "$repository/Cargo.toml" -p nauterm-rs --locked -- "$@"
