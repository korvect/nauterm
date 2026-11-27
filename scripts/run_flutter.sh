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

if [[ "$platform" == Linux && -z "${NAUTERM_MOSH_LIB_DIR:-}" ]]; then
  project_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
  case "$(uname -m)" in
    x86_64|amd64) mosh_arch=linux-x86_64 ;;
    aarch64|arm64) mosh_arch=linux-arm64 ;;
    *) mosh_arch="" ;;
  esac
  if [[ -n "$mosh_arch" && -f "$project_root/third_party/nauterm_mosh_ffi/$mosh_arch/libnauterm_mosh_ffi.so" ]]; then
    export NAUTERM_MOSH_LIB_DIR="$project_root/third_party/nauterm_mosh_ffi/$mosh_arch"
  fi
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
