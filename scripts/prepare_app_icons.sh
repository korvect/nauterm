#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_COMPOSER_ICON="$ROOT_DIR/assets/icons/Nauterm.icon"
GENERATED_ROOT="$ROOT_DIR/assets/icons/generated"
PNG_DIR="$GENERATED_ROOT/png"
MAC_APP_ICON_DIR="$ROOT_DIR/macos/Runner/Assets.xcassets/AppIcon.appiconset"
MAC_COMPOSER_ICON="$ROOT_DIR/macos/Runner/Resources/Nauterm.icon"
WINDOWS_ICON="$ROOT_DIR/windows/runner/resources/app_icon.ico"
LINUX_ICON="$ROOT_DIR/linux/runner/resources/nauterm.png"

bash "$ROOT_DIR/scripts/ci/verify_app_icons.sh" --generated-only

mkdir -p \
  "$MAC_APP_ICON_DIR" \
  "$(dirname "$MAC_COMPOSER_ICON")" \
  "$(dirname "$WINDOWS_ICON")" \
  "$(dirname "$LINUX_ICON")"

for size in 16 32 64 128 256 512 1024; do
  cp "$PNG_DIR/app_icon_$size.png" "$MAC_APP_ICON_DIR/app_icon_$size.png"
done

rm -rf "$MAC_COMPOSER_ICON"
cp -R "$SOURCE_COMPOSER_ICON" "$MAC_COMPOSER_ICON"
cp "$GENERATED_ROOT/app_icon.ico" "$WINDOWS_ICON"
cp "$PNG_DIR/app_icon_1024.png" "$LINUX_ICON"

bash "$ROOT_DIR/scripts/ci/verify_app_icons.sh"
echo "Prepared platform app icons from assets/icons/."
