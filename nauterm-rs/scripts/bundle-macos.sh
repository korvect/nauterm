#!/usr/bin/env bash
set -euo pipefail
umask 022

if [[ "$(uname -s)" != Darwin ]]; then
  echo 'The macOS bundle must be built on macOS.' >&2
  exit 1
fi
client="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
repository="$(CDPATH= cd -- "$client/.." && pwd)"
profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *) echo "Usage: bash nauterm-rs/scripts/bundle-macos.sh [debug|release]" >&2; exit 2 ;;
esac
build_number="${NAUTERM_BUILD_NUMBER:-1}"
if [[ ! "$build_number" =~ ^[0-9]+$ ]] || [[ ${#build_number} -gt 5 ]] || (( 10#$build_number < 1 || 10#$build_number > 65535 )); then
  echo 'NAUTERM_BUILD_NUMBER must be an integer from 1 through 65535.' >&2
  exit 1
fi

bash "$repository/scripts/ci/verify_versions.sh" >&2
bash "$repository/scripts/ci/verify_app_icons.sh" --generated-only >&2
work="$(mktemp -d "${TMPDIR:-/tmp}/nauterm-rs-bundle.XXXXXX")"
staging=''
cleanup() {
  rm -rf "$work"
  if [[ -n "$staging" ]]; then rm -rf "$staging"; fi
}
trap cleanup EXIT

build_args=(build --manifest-path "$repository/Cargo.toml" -p nauterm-rs --locked --message-format=json)
if [[ "$profile" == release ]]; then build_args+=(--release); fi
cargo "${build_args[@]}" > "$work/build.jsonl"
cargo metadata --manifest-path "$repository/Cargo.toml" --locked --no-deps --format-version 1 > "$work/metadata.json"
executable="$(python3 - "$work/build.jsonl" <<'PY'
import json, sys
artifacts = (json.loads(line) for line in open(sys.argv[1], encoding='utf-8'))
executables = [item['executable'] for item in artifacts
    if item.get('reason') == 'compiler-artifact'
    and item.get('target', {}).get('name') == 'nauterm-rs'
    and item.get('executable')]
if len(executables) != 1:
    raise SystemExit('Cargo did not report exactly one Nauterm RS executable.')
print(executables[0])
PY
)"
output="$(dirname -- "$executable")"
bundle="$output/Nauterm RS.app"
staging="$(mktemp -d "$output/.nauterm-rs-bundle.XXXXXX")"
app="$staging/Nauterm RS.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses" "$app/Contents/Frameworks"
python3 "$repository/scripts/stage_rs_mosh.py" --project "$repository" --executable "$executable" --destination "$app/Contents/Frameworks" >&2
install -m 755 "$executable" "$app/Contents/MacOS/nauterm-rs"
# Use the same Icon Composer document as Flutter's Xcode target. actool emits
# Assets.car and the legacy fallback, plus the matching bundle declarations.
xcrun --sdk macosx actool \
  --compile "$app/Contents/Resources" \
  --platform macosx --minimum-deployment-target 12.0 \
  --app-icon Nauterm --output-format human-readable-text \
  --output-partial-info-plist "$work/asset-info.plist" \
  "$repository/assets/icons/Nauterm.icon" >&2
python3 - "$work/metadata.json" "$client/assets/macos/Info.plist" "$app/Contents/Info.plist" "$build_number" "$work/asset-info.plist" <<'PY'
import json, plistlib, sys
metadata = json.load(open(sys.argv[1], encoding='utf-8'))
package = next(package for package in metadata['packages'] if package['name'] == 'nauterm-rs')
with open(sys.argv[2], 'rb') as source:
    info = plistlib.load(source)
with open(sys.argv[5], 'rb') as source:
    info.update(plistlib.load(source))
info['CFBundleShortVersionString'] = package['version']
info['CFBundleVersion'] = str(int(sys.argv[4]))
with open(sys.argv[3], 'wb') as target:
    plistlib.dump(info, target, sort_keys=False)
PY
if [[ -f "$client/assets/licenses/nauterm-rs-rust.json" ]]; then
  python3 "$repository/scripts/package_rs_notices.py" \
    --project "$repository" --build-log "$work/build.jsonl" \
    --destination "$app/Contents/Resources/licenses"
elif [[ "$profile" == release ]]; then
  echo 'RS Rust license notices are missing. Run make generate-rs-licenses.' >&2
  exit 1
fi

identity="${NAUTERM_RS_SIGN_IDENTITY:--}"
python3 "$repository/scripts/stage_rs_sparkle.py" --project "$repository" --app "$app" --identity "$identity" >&2
codesign_args=(--force --sign "$identity")
if [[ "$identity" == - ]]; then
  codesign_args+=(--timestamp=none)
else
  codesign_args+=(--options runtime --timestamp)
fi
codesign "${codesign_args[@]}" "$app/Contents/Frameworks/libnauterm_mosh_ffi.dylib" >&2
codesign --verify --strict "$app/Contents/Frameworks/libnauterm_mosh_ffi.dylib" >&2
codesign "${codesign_args[@]}" "$app" >&2
codesign --verify --strict "$app" >&2
if [[ -n "${NAUTERM_RS_NOTARY_KEY:-}${NAUTERM_RS_NOTARY_PROFILE:-}" ]]; then
  if [[ "$profile" != release || "$identity" == - ]]; then
    echo 'Notarization requires a release build signed with Developer ID.' >&2
    exit 1
  fi
  notary_args=()
  if [[ -n "${NAUTERM_RS_NOTARY_PROFILE:-}" ]]; then
    notary_args+=(--keychain-profile "$NAUTERM_RS_NOTARY_PROFILE")
  else
    : "${NAUTERM_RS_NOTARY_KEY_ID:?notary key ID is required}"
    : "${NAUTERM_RS_NOTARY_ISSUER:?notary issuer is required}"
    notary_args+=(--key "$NAUTERM_RS_NOTARY_KEY" --key-id "$NAUTERM_RS_NOTARY_KEY_ID" --issuer "$NAUTERM_RS_NOTARY_ISSUER")
  fi
  ditto -c -k --sequesterRsrc --keepParent "$app" "$work/notarize.zip"
  xcrun notarytool submit "$work/notarize.zip" "${notary_args[@]}" --wait --output-format json > "$work/notary.json"
  python3 - "$work/notary.json" <<'PYNOTARY'
import json, sys
result = json.load(open(sys.argv[1], encoding='utf-8'))
if result.get('status') != 'Accepted':
    raise SystemExit(f"Notarization was not accepted: {result.get('status')} ({result.get('id')})")
PYNOTARY
  xcrun stapler staple "$app" >&2
  xcrun stapler validate "$app" >&2
  spctl --assess --type execute --verbose=2 "$app" >&2
fi


# Publish only a fully assembled bundle. Cargo's target/target-triple directory
# comes from its artifact output, including a custom CARGO_TARGET_DIR.
if [[ -e "$bundle" ]]; then mv "$bundle" "$staging/previous.app"; fi
if ! mv "$app" "$bundle"; then
  if [[ -e "$staging/previous.app" ]]; then mv "$staging/previous.app" "$bundle"; fi
  exit 1
fi
if [[ "$profile" == release ]]; then
  version="$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$bundle/Contents/Info.plist")"
  arch="$(lipo -archs "$bundle/Contents/MacOS/nauterm-rs")"
  dist="${DIST_DIR:-$repository/dist}"
  mkdir -p "$dist"
  ditto -c -k --sequesterRsrc --keepParent "$bundle" "$dist/Nauterm-RS-$version-macos-$arch.app.zip"
fi
printf '%s\n' "$bundle"
