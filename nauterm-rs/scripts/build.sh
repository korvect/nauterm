#!/usr/bin/env bash
set -euo pipefail
repository="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    MSYS2_ARG_CONV_EXCL='*' powershell.exe -NoProfile -ExecutionPolicy Bypass \
      -File "$(cygpath -w "$repository/nauterm-rs/scripts/windows.ps1")" -Mode Build "$@"
    exit $?
    ;;
esac
work="$(mktemp -d "${TMPDIR:-/tmp}/nauterm-rs-build.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cargo build --manifest-path "$repository/Cargo.toml" -p nauterm-rs --locked --message-format=json-render-diagnostics "$@" > "$work/build.jsonl"
if [[ -n "${NAUTERM_RS_BUILD_LOG:-}" ]]; then cp "$work/build.jsonl" "$NAUTERM_RS_BUILD_LOG"; fi
executable="$(python3 - "$work/build.jsonl" <<'PY'
import json, sys
artifacts = (json.loads(line) for line in open(sys.argv[1], encoding='utf-8'))
executables = [item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact'
    and item.get('target', {}).get('name') == 'nauterm-rs' and item.get('executable')]
if len(executables) != 1:
    raise SystemExit('Cargo did not report exactly one Nauterm RS executable.')
print(executables[0])
PY
)"
python3 "$repository/scripts/stage_rs_mosh.py" --project "$repository" --executable "$executable"
printf '%s\n' "$executable"
