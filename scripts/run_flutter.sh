#!/usr/bin/env bash
set -euo pipefail

platform="$(uname -s)"
set -- run
case "$platform" in
  MINGW*|MSYS*|CYGWIN*)
    # The Windows debug runner can coexist with the installed client when it
    # uses a separate profile. Keep this profile across rebuilds and cleans.
    export NAUTERM_DATA_DIR="${NAUTERM_DATA_DIR:-${LOCALAPPDATA:?LOCALAPPDATA is unavailable}\NautermDev}"
    set -- "$@" -d windows
    ;;
esac

if [[ -z "${NAUTERM_MOSH_LIB_DIR:-}" ]]; then
  project_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
  case "$platform" in
    Darwin)
      case "$(uname -m)" in
        arm64) mosh_arch=macos-arm64 ;;
        x86_64) mosh_arch=macos-x86_64 ;;
        *)
          echo "Unsupported macOS architecture for the bundled Mosh library: $(uname -m)" >&2
          exit 1
          ;;
      esac
      mosh_library_name=libnauterm_mosh_ffi.dylib
      ;;
    Linux)
      case "$(uname -m)" in
        x86_64|amd64) mosh_arch=linux-x86_64 ;;
        aarch64|arm64) mosh_arch=linux-arm64 ;;
        *)
          echo "Unsupported Linux architecture for the bundled Mosh library: $(uname -m)" >&2
          exit 1
          ;;
      esac
      mosh_library_name=libnauterm_mosh_ffi.so
      ;;
    MINGW*|MSYS*|CYGWIN*)
      windows_arch="${PROCESSOR_ARCHITECTURE:-$(uname -m)}"
      case "$windows_arch" in
        ARM64|arm64|aarch64) mosh_arch=windows-arm64 ;;
        AMD64|amd64|x86_64) mosh_arch=windows-x86_64 ;;
        *)
          echo "Unsupported Windows architecture for the bundled Mosh library: $windows_arch" >&2
          exit 1
          ;;
      esac
      mosh_library_name=nauterm_mosh_ffi.dll
      ;;
    *)
      echo "Unsupported platform for the bundled Mosh library: $platform" >&2
      exit 1
      ;;
  esac

  mosh_library_dir="$project_root/third_party/nauterm_mosh_ffi/$mosh_arch"
  if [[ ! -f "$mosh_library_dir/$mosh_library_name" ]]; then
    echo "Missing bundled Mosh library: $mosh_library_dir/$mosh_library_name" >&2
    exit 1
  fi

  case "$platform" in
    MINGW*|MSYS*|CYGWIN*)
      export NAUTERM_MOSH_LIB_DIR="$(cygpath -w "$mosh_library_dir")"
      ;;
    *)
      export NAUTERM_MOSH_LIB_DIR="$mosh_library_dir"
      ;;
  esac
fi

for name in \
  NAUTERM_UPDATE_REPOSITORY \
  NAUTERM_POSTHOG_API_KEY \
  NAUTERM_POSTHOG_HOST \
  NAUTERM_GITHUB_CLIENT_ID \
  NAUTERM_GOOGLE_CLIENT_ID \
  NAUTERM_GOOGLE_CLIENT_SECRET \
  NAUTERM_ONEDRIVE_CLIENT_ID \
  NAUTERM_DROPBOX_CLIENT_ID; do
  if [[ -n "${!name:-}" ]]; then
    set -- "$@" "--dart-define=${name}=${!name}"
  fi
done

exec flutter "$@"
