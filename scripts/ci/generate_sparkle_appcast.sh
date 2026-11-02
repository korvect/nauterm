#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:?architecture is required}"
CLIENT="${2:-flutter}"
case "$CLIENT" in
  flutter) APP_PREFIX=Nauterm; FEED_PREFIX=appcast ;;
  rs) APP_PREFIX=Nauterm-RS; FEED_PREFIX=appcast-rs ;;
  *) echo 'Client must be flutter or rs.' >&2; exit 2 ;;
esac
DIST_DIR="${DIST_DIR:-dist}"
PUBSPEC_VERSION="$(awk '/^version: / { print $2; exit }' pubspec.yaml)"
VERSION="${PUBSPEC_VERSION%%+*}"
ARCHIVE="$DIST_DIR/${APP_PREFIX}-${VERSION}-macos-${ARCH}.app.zip"

: "${SPARKLE_PRIVATE_KEY:?SPARKLE_PRIVATE_KEY is required for tagged releases}"
: "${GITHUB_REF_NAME:?GITHUB_REF_NAME is required}"
UPDATE_REPOSITORY="${NAUTERM_UPDATE_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
: "${UPDATE_REPOSITORY:?NAUTERM_UPDATE_REPOSITORY is required}"

if [ ! -f "$ARCHIVE" ]; then
  echo "Sparkle update archive not found: $ARCHIVE" >&2
  exit 1
fi

GENERATE_APPCAST=''
if [ -d build/macos ]; then
  GENERATE_APPCAST="$(find build/macos -type f -path '*/Sparkle/bin/generate_appcast' -print -quit)"
fi
if [ -z "$GENERATE_APPCAST" ] && [ "$CLIENT" = rs ]; then
  GENERATE_APPCAST=target/sparkle/2.9.4/bin/generate_appcast
fi
if [ -z "$GENERATE_APPCAST" ] || [ ! -x "$GENERATE_APPCAST" ]; then
  echo "Sparkle generate_appcast tool was not found under build/macos." >&2
  exit 1
fi

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nauterm-appcast.XXXXXX")"
ARCHIVE_DIR="$WORK_DIR/archives"
KEY_FILE="$WORK_DIR/sparkle-private-key"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p "$ARCHIVE_DIR"
cp "$ARCHIVE" "$ARCHIVE_DIR/"
printf '%s' "$SPARKLE_PRIVATE_KEY" > "$KEY_FILE"
chmod 600 "$KEY_FILE"

"$GENERATE_APPCAST" \
  --ed-key-file "$KEY_FILE" \
  --download-url-prefix \
  "https://github.com/$UPDATE_REPOSITORY/releases/download/$GITHUB_REF_NAME/" \
  -o "$DIST_DIR/${FEED_PREFIX}-${ARCH}.xml" \
  "$ARCHIVE_DIR"

if [ ! -f "$DIST_DIR/${FEED_PREFIX}-${ARCH}.xml" ]; then
  echo "Sparkle did not generate an appcast." >&2
  exit 1
fi
