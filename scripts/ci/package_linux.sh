#!/usr/bin/env bash
set -euo pipefail

ARCH="${1:?architecture is required}"
DEB_ARCH="${2:?deb architecture is required}"
RPM_ARCH="${3:?rpm architecture is required}"
APPIMAGE_ARCH="${4:?AppImage architecture is required}"

APP_NAME="Nauterm"
APP_ID="com.korvect.nauterm"
BINARY_NAME="nauterm"
DIST_DIR="${DIST_DIR:-dist}"
CLEAN_DIST="${CLEAN_DIST:-1}"
ICON_SOURCE="linux/runner/resources/nauterm.png"
ICON_GENERATED_PNG_DIR="assets/icons/generated/png"

PUBSPEC_VERSION="$(awk '/^version: / { print $2; exit }' pubspec.yaml)"
VERSION="${PUBSPEC_VERSION%%+*}"
PUBSPEC_BUILD_NUMBER="${PUBSPEC_VERSION#*+}"
if [ "$PUBSPEC_BUILD_NUMBER" = "$PUBSPEC_VERSION" ]; then
  PUBSPEC_BUILD_NUMBER="1"
fi
BUILD_NUMBER="${NAUTERM_BUILD_NUMBER:-$PUBSPEC_BUILD_NUMBER}"
UPDATE_REPOSITORY="${NAUTERM_UPDATE_REPOSITORY:-${GITHUB_REPOSITORY:-}}"

HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
  amd64) HOST_ARCH="x86_64" ;;
  aarch64) HOST_ARCH="arm64" ;;
esac
if [ "$HOST_ARCH" != "$ARCH" ]; then
  echo "Linux package architecture $ARCH requires a matching host; current host is $HOST_ARCH." >&2
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

bash scripts/ci/verify_app_icons.sh --generated-only
install -Dm644 "assets/icons/generated/png/app_icon_512.png" "linux/runner/resources/nauterm.png"

if [ "$CLEAN_DIST" = "1" ]; then
  rm -rf "$DIST_DIR"
fi
mkdir -p "$DIST_DIR"

flutter config --enable-linux-desktop
if [ "${NAUTERM_SKIP_PUB_GET:-0}" != "1" ]; then
  flutter pub get
fi

if [ -z "${NAUTERM_MOSH_LIB_DIR:-}" ]; then
  case "$ARCH" in
    x86_64) NAUTERM_MOSH_LIB_DIR="third_party/nauterm_mosh_ffi/linux-x86_64" ;;
    arm64)  NAUTERM_MOSH_LIB_DIR="third_party/nauterm_mosh_ffi/linux-arm64" ;;
  esac
fi
NAUTERM_MOSH_LIB_DIR="$(cd "$NAUTERM_MOSH_LIB_DIR" && pwd)"
export NAUTERM_MOSH_LIB_DIR

if [ -n "${NAUTERM_MOSH_LIB_DIR:-}" ]; then
  MOSH_LIBRARY="$NAUTERM_MOSH_LIB_DIR/libnauterm_mosh_ffi.so"
  if [ ! -f "$MOSH_LIBRARY" ]; then
    echo "Missing prebuilt Mosh library: $MOSH_LIBRARY" >&2
    exit 1
  fi
  case "$ARCH" in
    x86_64) EXPECTED_FILE_PATTERN='x86-64|x86_64' ;;
    arm64) EXPECTED_FILE_PATTERN='ARM aarch64|aarch64' ;;
    *)
      echo "Unsupported Linux architecture: $ARCH" >&2
      exit 1
      ;;
  esac
  if ! file "$MOSH_LIBRARY" | grep -Eq "$EXPECTED_FILE_PATTERN"; then
    echo "Prebuilt Mosh library does not match the expected $ARCH architecture: $MOSH_LIBRARY" >&2
    file "$MOSH_LIBRARY" >&2
    exit 1
  fi
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
flutter build linux \
  --release \
  --no-pub \
  --build-name="$VERSION" \
  --build-number="$BUILD_NUMBER" \
  "${dart_define_args[@]}"

BUNDLE_DIR="$(find build/linux -path '*/release/bundle' -type d | head -n 1)"
if [ -z "$BUNDLE_DIR" ] || [ ! -d "$BUNDLE_DIR" ]; then
  echo "Linux release bundle not found." >&2
  exit 1
fi

create_desktop_file() {
  local path="$1"
  cat > "$path" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=A modern, cross-platform terminal and remote access workspace built for greater control, transparency, and performance.
Exec=$BINARY_NAME
Icon=$APP_ID
Categories=Network;RemoteAccess;TerminalEmulator;
Terminal=false
# Must match the WM_CLASS the running window actually reports. GTK sets it to
# the value passed to g_set_prgname(), which in this app is APPLICATION_ID
# (com.korvect.nauterm). If it does not match, GNOME shows the running window
# as an unrelated entry with the default icon and the process name.
StartupWMClass=$APP_ID
EOF
}

install_icon() {
  local root="$1"
  local size png installed=0
  for size in 16 24 32 48 64 128 256 512; do
    png="$ICON_GENERATED_PNG_DIR/app_icon_$size.png"
    [ -f "$png" ] || continue
    install -Dm644 "$png" \
      "$root/usr/share/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
    installed=1
  done
  # Fallback when generated PNGs are unavailable: use the prepared 512px icon.
  if [ "$installed" -eq 0 ]; then
    install -Dm644 "$ICON_SOURCE" \
      "$root/usr/share/icons/hicolor/512x512/apps/$APP_ID.png"
  fi
  # pixmaps is also scanned by appimagetool and many desktop environments.
  install -Dm644 "$ICON_SOURCE" "$root/usr/share/pixmaps/$APP_ID.png"
}

if [ "${NAUTERM_APPIMAGE_ONLY:-0}" != "1" ]; then
  PKG_ROOT="$DIST_DIR/package-root"
  mkdir -p "$PKG_ROOT/opt/$BINARY_NAME" "$PKG_ROOT/usr/bin" \
    "$PKG_ROOT/usr/share/applications"
  cp -a "$BUNDLE_DIR/." "$PKG_ROOT/opt/$BINARY_NAME/"
  ln -s "/opt/$BINARY_NAME/$BINARY_NAME" "$PKG_ROOT/usr/bin/$BINARY_NAME"
  create_desktop_file "$PKG_ROOT/usr/share/applications/$APP_ID.desktop"
  install_icon "$PKG_ROOT"

  COMMON_FPM_ARGS=(
    --name "$BINARY_NAME"
    --version "$VERSION"
    --iteration "$BUILD_NUMBER"
    --description "A modern, cross-platform terminal and remote access workspace built for greater control, transparency, and performance."
    --license "Proprietary"
    --maintainer "Nauterm"
    --category "net"
    -s dir
    -C "$PKG_ROOT"
  )
  if [ -n "$UPDATE_REPOSITORY" ]; then
    COMMON_FPM_ARGS+=(--url "https://github.com/$UPDATE_REPOSITORY")
  fi

  FPM_PACKAGE_PATHS=(
    "opt/$BINARY_NAME"
    "usr/bin/$BINARY_NAME"
    "usr/share/applications/$APP_ID.desktop"
    "usr/share/icons"
    "usr/share/pixmaps"
  )

  fpm "${COMMON_FPM_ARGS[@]}" \
    -t deb \
    --architecture "$DEB_ARCH" \
    --depends "libgtk-3-0" \
    --depends "libstdc++6" \
    --depends "liblzma5" \
    --depends "libblkid1" \
    --package "$DIST_DIR/${BINARY_NAME}_${VERSION}-${BUILD_NUMBER}_linux-${ARCH}.deb" \
    "${FPM_PACKAGE_PATHS[@]}"

  fpm "${COMMON_FPM_ARGS[@]}" \
    -t rpm \
    --architecture "$RPM_ARCH" \
    --depends "gtk3" \
    --depends "libstdc++" \
    --depends "xz-libs" \
    --depends "util-linux-libs" \
    --package "$DIST_DIR/${BINARY_NAME}-${VERSION}-${BUILD_NUMBER}.linux-${ARCH}.rpm" \
    "${FPM_PACKAGE_PATHS[@]}"
fi

APPDIR="$DIST_DIR/AppDir"
mkdir -p "$APPDIR/usr/lib/$BINARY_NAME" "$APPDIR/usr/share/applications"
cp -a "$BUNDLE_DIR/." "$APPDIR/usr/lib/$BINARY_NAME/"
create_desktop_file "$APPDIR/$APP_ID.desktop"
cp "$APPDIR/$APP_ID.desktop" "$APPDIR/usr/share/applications/$APP_ID.desktop"
install_icon "$APPDIR"
# Intentionally do NOT inject a top-level icon or .DirIcon: an unintegrated
# AppImage is expected to show the default file-manager icon. The icon for the
# integrated desktop entry is supplied below via --icon-file.

cat > "$APPDIR/AppRun" <<EOF
#!/usr/bin/env sh
HERE="\$(dirname "\$(readlink -f "\$0")")"
# Let GNOME/KDE find the icons and desktop entry bundled inside the AppImage.
export XDG_DATA_DIRS="\$HERE/usr/share:\${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
# Hint for BAMF (Ubuntu/GNOME dock) so the running window is matched to the
# correct desktop entry and shows the application name and icon instead of the
# process name.
export BAMF_DESKTOP_FILE_HINT="\$HERE/usr/share/applications/$APP_ID.desktop"
exec "\$HERE/usr/lib/$BINARY_NAME/$BINARY_NAME" "\$@"
EOF
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/lib/$BINARY_NAME/$BINARY_NAME"

LINUXDEPLOY="$DIST_DIR/linuxdeploy-${APPIMAGE_ARCH}.AppImage"
LINUXDEPLOY_VERSION="1-alpha-20251107-1"
case "$APPIMAGE_ARCH" in
  x86_64)
    LINUXDEPLOY_SHA256="c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d"
    ;;
  aarch64)
    LINUXDEPLOY_SHA256="620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff"
    ;;
  *)
    echo "Unsupported linuxdeploy architecture: $APPIMAGE_ARCH" >&2
    exit 1
    ;;
esac
curl -L \
  --fail \
  --show-error \
  "https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${APPIMAGE_ARCH}.AppImage" \
  -o "$LINUXDEPLOY"
printf '%s  %s\n' "$LINUXDEPLOY_SHA256" "$LINUXDEPLOY" | sha256sum --check -
chmod +x "$LINUXDEPLOY"

APPIMAGE_NAME="${APP_NAME}-${VERSION}-linux-${ARCH}.AppImage"
APPIMAGE_PATH="$DIST_DIR/$APPIMAGE_NAME"
APPIMAGE_ARCHIVE="$APPIMAGE_PATH.tar.gz"

# linuxdeploy cannot infer the size of the pixmaps icon; point it at a sized
# hicolor entry so the AppImage gets a properly registered icon.
APPIMAGE_ICON="$APPDIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"
if [ ! -f "$APPIMAGE_ICON" ]; then
  APPIMAGE_ICON="$APPDIR/usr/share/pixmaps/$APP_ID.png"
fi

# LDAI_OUTPUT replaces the deprecated OUTPUT variable in linuxdeploy 1-alpha.
LDAI_OUTPUT="$APPIMAGE_PATH" \
  ARCH="$APPIMAGE_ARCH" \
  APPIMAGE_EXTRACT_AND_RUN=1 \
  "$LINUXDEPLOY" \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/lib/$BINARY_NAME/$BINARY_NAME" \
  --desktop-file "$APPDIR/$APP_ID.desktop" \
  --icon-file "$APPIMAGE_ICON" \
  --output appimage

chmod 0755 "$APPIMAGE_PATH"
rm -f "$APPIMAGE_ARCHIVE"
tar -C "$DIST_DIR" -czf "$APPIMAGE_ARCHIVE" "$APPIMAGE_NAME"

rm -rf "${PKG_ROOT:-}" "$APPDIR" "$LINUXDEPLOY"
