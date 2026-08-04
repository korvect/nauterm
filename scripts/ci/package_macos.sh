#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:?architecture is required}"
APP_NAME="Nauterm"
DIST_DIR="${DIST_DIR:-dist}"
CLEAN_DIST="${CLEAN_DIST:-1}"
APP_PATH="build/macos/Build/Products/Release/${APP_NAME}.app"
PUBSPEC_VERSION="$(awk '/^version: / { print $2; exit }' pubspec.yaml)"
VERSION="${PUBSPEC_VERSION%%+*}"
PUBSPEC_BUILD_NUMBER="${PUBSPEC_VERSION#*+}"
if [ "$PUBSPEC_BUILD_NUMBER" = "$PUBSPEC_VERSION" ]; then
  PUBSPEC_BUILD_NUMBER="1"
fi
BUILD_NUMBER="${NAUTERM_BUILD_NUMBER:-$PUBSPEC_BUILD_NUMBER}"
DMG_VOLUME_NAME="${APP_NAME} ${VERSION} ${ARCH}"
UPDATE_REPOSITORY="${NAUTERM_UPDATE_REPOSITORY:-${GITHUB_REPOSITORY:-}}"

HOST_ARCH="$(uname -m)"
if [ "$HOST_ARCH" != "$ARCH" ]; then
  echo "macOS package architecture $ARCH requires a matching host; current host is $HOST_ARCH." >&2
  exit 1
fi

if ! [[ "$BUILD_NUMBER" =~ ^[0-9]+$ ]] || [ "$BUILD_NUMBER" -lt 1 ] || [ "$BUILD_NUMBER" -gt 65535 ]; then
  echo "Build number must be an integer from 1 through 65535: $BUILD_NUMBER" >&2
  exit 1
fi

case "$DIST_DIR" in
  ""|"."|".."|/*|../*|*/..|*/../*)
    echo "DIST_DIR must be a relative child of the project directory: $DIST_DIR" >&2
    exit 1
    ;;
esac

bash scripts/prepare_app_icons.sh

if [ "$CLEAN_DIST" = "1" ]; then
  rm -rf "$DIST_DIR"
fi
mkdir -p "$DIST_DIR"

if [ "${GITHUB_REF_TYPE:-}" = "tag" ] && [ -z "${SPARKLE_PUBLIC_ED_KEY:-}" ]; then
  echo "SPARKLE_PUBLIC_ED_KEY is required for tagged releases." >&2
  exit 1
fi

if [ -n "${SPARKLE_PUBLIC_ED_KEY:-}" ]; then
  if [ -z "$UPDATE_REPOSITORY" ]; then
    echo "NAUTERM_UPDATE_REPOSITORY is required when Sparkle updates are enabled." >&2
    exit 1
  fi
  mkdir -p macos/Flutter/ephemeral
  ESCAPED_PUBLIC_KEY="${SPARKLE_PUBLIC_ED_KEY//\//\$(SLASH)}"
  {
    printf '%s\n' 'SLASH = /'
    printf 'SPARKLE_PUBLIC_ED_KEY = %s\n' "$ESCAPED_PUBLIC_KEY"
    printf 'SPARKLE_FEED_URL = https:/$()/github.com/%s/releases/latest/download/appcast-%s.xml\n' \
      "$UPDATE_REPOSITORY" "$ARCH"
  } > macos/Flutter/ephemeral/Sparkle.xcconfig
else
  rm -f macos/Flutter/ephemeral/Sparkle.xcconfig
  echo "Warning: SPARKLE_PUBLIC_ED_KEY is empty; this package cannot use Sparkle updates." >&2
fi

# GitHub-hosted runners do not have an Apple signing identity. Package those
# builds with an ad-hoc signature; local builds keep Xcode's normal automatic
# signing behavior.
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
  export CODE_SIGN_IDENTITY="-"
  unset DEVELOPMENT_TEAM
fi

if [ -z "${NAUTERM_MOSH_LIB_DIR:-}" ]; then
  NAUTERM_MOSH_LIB_DIR="third_party/nauterm_mosh_ffi/macos-$ARCH"
fi
NAUTERM_MOSH_LIB_DIR="$(cd "$NAUTERM_MOSH_LIB_DIR" && pwd)"
export NAUTERM_MOSH_LIB_DIR

if [ -n "${NAUTERM_MOSH_LIB_DIR:-}" ]; then
  MOSH_LIBRARY="$NAUTERM_MOSH_LIB_DIR/libnauterm_mosh_ffi.dylib"
  if [ ! -f "$MOSH_LIBRARY" ]; then
    echo "Missing prebuilt Mosh library: $MOSH_LIBRARY" >&2
    exit 1
  fi
  if ! lipo -archs "$MOSH_LIBRARY" | tr ' ' '\n' | grep -Fxq "$ARCH"; then
    echo "Prebuilt Mosh library does not contain the expected $ARCH architecture: $MOSH_LIBRARY" >&2
    lipo -info "$MOSH_LIBRARY" >&2
    exit 1
  fi
fi

flutter config --enable-macos-desktop
if [ "${NAUTERM_SKIP_PUB_GET:-0}" != "1" ]; then
  flutter pub get
fi
dart_define_args=()
for name in \
  NAUTERM_UPDATE_REPOSITORY \
  NAUTERM_POSTHOG_API_KEY \
  NAUTERM_POSTHOG_HOST \
  NAUTERM_GITHUB_CLIENT_ID \
  NAUTERM_GOOGLE_CLIENT_ID \
  NAUTERM_GOOGLE_CLIENT_SECRET \
  NAUTERM_ONEDRIVE_CLIENT_ID \
  NAUTERM_DROPBOX_CLIENT_ID; do
  if [ -n "${!name:-}" ]; then
    dart_define_args+=("--dart-define=${name}=${!name}")
  fi
done
flutter build macos \
  --release \
  --no-pub \
  --build-name="$VERSION" \
  --build-number="$BUILD_NUMBER" \
  "${dart_define_args[@]}"

if [ ! -d "$APP_PATH" ]; then
  echo "Expected app bundle not found: $APP_PATH" >&2
  exit 1
fi

ditto -c -k --sequesterRsrc --keepParent \
  "$APP_PATH" \
  "$DIST_DIR/${APP_NAME}-${VERSION}-macos-${ARCH}.app.zip"

DMG_PATH="$DIST_DIR/${APP_NAME}-${VERSION}-macos-${ARCH}.dmg"
DMG_STAGE_PARENT="$(mktemp -d "${TMPDIR:-/tmp}/${APP_NAME}.dmg.XXXXXX")"
DMG_STAGE="$DMG_STAGE_PARENT/$DMG_VOLUME_NAME"
cleanup_dmg_stage() {
  rm -rf "$DMG_STAGE_PARENT"
}
trap cleanup_dmg_stage EXIT

mkdir -p "$DMG_STAGE"
ditto "$APP_PATH" "$DMG_STAGE/${APP_NAME}.app"
# The mounted path is part of macOS' Finder/LaunchServices icon cache key. Give
# the copied bundle a fresh container timestamp so rebuilding the same version
# does not keep showing an icon cached for an older DMG payload.
touch "$DMG_STAGE/${APP_NAME}.app"
ln -s /Applications "$DMG_STAGE/Applications"
rm -f "$DMG_PATH"

if ! diskutil image create from --format UDZO "$DMG_STAGE" "$DMG_PATH"; then
  echo "diskutil image create failed; falling back to hdiutil create." >&2
  hdiutil create \
    -volname "$DMG_VOLUME_NAME" \
    -srcfolder "$DMG_STAGE" \
    -ov \
    -format UDZO \
    "$DMG_PATH"
fi
