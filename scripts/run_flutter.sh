#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" == Linux && -z "${NAUTERM_MOSH_LIB_DIR:-}" ]]; then
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
  if [[ -n "${!name:-}" ]]; then
    dart_define_args+=("--dart-define=${name}=${!name}")
  fi
done

exec flutter run "${dart_define_args[@]}"
