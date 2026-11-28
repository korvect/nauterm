#!/usr/bin/env sh
set -eu

MODE="${1:-debug}"
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
CORE_CRATE_DIR="$PROJECT_ROOT/native/nauterm_core"
MOSH_LIB_DIR="${NAUTERM_MOSH_LIB_DIR:-}"
DEFAULT_MOSH_REPO_DIR="$(CDPATH= cd -- "$PROJECT_ROOT/../nauterm-mosh" 2>/dev/null && pwd || true)"
MOSH_REPO_DIR="${NAUTERM_MOSH_REPO_DIR:-$DEFAULT_MOSH_REPO_DIR}"
MOSH_CORE_CRATE_DIR="$MOSH_REPO_DIR/nauterm_mosh_ffi"

case "$(uname -s)" in
  Darwin)
    MOSH_LIBRARY_NAME="libnauterm_mosh_ffi.dylib"
    ;;
  Linux)
    MOSH_LIBRARY_NAME="libnauterm_mosh_ffi.so"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    MOSH_LIBRARY_NAME="nauterm_mosh_ffi.dll"
    ;;
  *)
    echo "Unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

if [ -n "$MOSH_LIB_DIR" ] && [ ! -f "$MOSH_LIB_DIR/$MOSH_LIBRARY_NAME" ]; then
  echo "Missing prebuilt $MOSH_LIBRARY_NAME in NAUTERM_MOSH_LIB_DIR=$MOSH_LIB_DIR" >&2
  exit 1
fi

if [ -z "$MOSH_LIB_DIR" ] && { [ -z "$MOSH_REPO_DIR" ] || [ ! -f "$MOSH_CORE_CRATE_DIR/Cargo.toml" ]; }; then
  echo "Missing nauterm-mosh workspace. Set NAUTERM_MOSH_REPO_DIR to a checkout containing nauterm_mosh_ffi." >&2
  exit 1
fi

if [ "$MODE" = "release" ]; then
  cargo build --manifest-path "$CORE_CRATE_DIR/Cargo.toml" --release
  if [ -z "$MOSH_LIB_DIR" ]; then
    cargo build --manifest-path "$MOSH_CORE_CRATE_DIR/Cargo.toml" --release
  fi
  NATIVE_PROFILE="release"
  MACOS_CONFIGURATION="Release"
else
  cargo build --manifest-path "$CORE_CRATE_DIR/Cargo.toml"
  if [ -z "$MOSH_LIB_DIR" ]; then
    cargo build --manifest-path "$MOSH_CORE_CRATE_DIR/Cargo.toml"
  fi
  NATIVE_PROFILE="debug"
  MACOS_CONFIGURATION="Debug"
fi

if [ "$(uname -s)" = "Darwin" ]; then
  FRAMEWORKS_DIR="$PROJECT_ROOT/build/macos/Build/Products/$MACOS_CONFIGURATION/Nauterm.app/Contents/Frameworks"
  if [ -d "$FRAMEWORKS_DIR" ]; then
    for library in libnauterm_core.dylib libnauterm_mosh_ffi.dylib; do
      case "$library" in
        libnauterm_core.dylib)
          SOURCE_DYLIB="$PROJECT_ROOT/target/$NATIVE_PROFILE/$library"
          ;;
        *)
          if [ -n "$MOSH_LIB_DIR" ]; then
            SOURCE_DYLIB="$MOSH_LIB_DIR/$library"
          else
            SOURCE_DYLIB="$MOSH_REPO_DIR/target/$NATIVE_PROFILE/$library"
          fi
          ;;
      esac
      APP_DYLIB="$FRAMEWORKS_DIR/$library"
      if [ -f "$SOURCE_DYLIB" ]; then
        cp "$SOURCE_DYLIB" "$APP_DYLIB"
        codesign --force --sign - --timestamp=none "$APP_DYLIB" >/dev/null 2>&1 || true
      fi
    done
  fi
fi
