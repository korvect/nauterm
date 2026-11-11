#!/usr/bin/env bash
set -euo pipefail

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

if ((${#dart_define_args[@]} > 0)); then
  exec flutter run "${dart_define_args[@]}"
else
  exec flutter run
fi
