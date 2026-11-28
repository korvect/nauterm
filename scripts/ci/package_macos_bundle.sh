#!/usr/bin/env bash
set -euo pipefail

app="${1:?app bundle path is required}"
output="${2:?package output prefix is required}"
volume_name="${3:?disk image volume name is required}"
app_name="${app##*/}"

work="$(mktemp -d "${TMPDIR:-/tmp}/nauterm-dmg.XXXXXX")"
trap 'rm -rf "$work"' EXIT
stage="$work/$volume_name"
mkdir -p "$stage"

ditto -c -k --sequesterRsrc --keepParent "$app" "$work/package.app.zip"
ditto "$app" "$stage/$app_name"
# Refresh the copied bundle timestamp for Finder's icon cache.
touch "$stage/$app_name"
ln -s /Applications "$stage/Applications"

if ! diskutil image create from --format UDZO "$stage" "$work/package.dmg"; then
  echo "diskutil image create failed; falling back to hdiutil create." >&2
  hdiutil create -volname "$volume_name" -srcfolder "$stage" -ov -format UDZO "$work/package.dmg"
fi

mv -f "$work/package.app.zip" "$output.app.zip"
mv -f "$work/package.dmg" "$output.dmg"
