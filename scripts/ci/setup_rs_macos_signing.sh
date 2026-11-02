#!/usr/bin/env bash
# Optional CI signing credentials. Never use the user's default keychain.
set -euo pipefail
umask 077

if [[ "${1:-}" == cleanup ]]; then
  if [[ -n "${NAUTERM_RS_SIGN_WORK:-}" && -d "$NAUTERM_RS_SIGN_WORK" ]]; then
    keychain="$NAUTERM_RS_SIGN_WORK/signing.keychain-db"
    if [[ -f "$keychain" ]]; then security delete-keychain "$keychain"; fi
    rm -rf "$NAUTERM_RS_SIGN_WORK"
  fi
  exit 0
fi

certificate="${NAUTERM_RS_SIGN_CERTIFICATE:-}"
password="${NAUTERM_RS_SIGN_PASSWORD:-}"
identity="${NAUTERM_RS_SIGN_IDENTITY:-}"
notary_key="${NAUTERM_RS_NOTARY_KEY_BASE64:-}"
notary_id="${NAUTERM_RS_NOTARY_KEY_ID:-}"
notary_issuer="${NAUTERM_RS_NOTARY_ISSUER:-}"
if [[ -z "$certificate$password$identity$notary_key$notary_id$notary_issuer" ]]; then
  echo 'RS distribution signing is not configured; building an ad-hoc signed package.'
  exit 0
fi
if [[ -z "$certificate" || -z "$password" || "$identity" != "Developer ID Application: "* ]]; then
  echo 'Configure the RS Developer ID certificate, password and identity together.' >&2
  exit 1
fi
if [[ -n "$notary_key$notary_id$notary_issuer" ]] && [[ -z "$notary_key" || -z "$notary_id" || -z "$notary_issuer" ]]; then
  echo 'Configure all three RS App Store Connect notarization credentials together.' >&2
  exit 1
fi
: "${GITHUB_ENV:?GITHUB_ENV is required}"
: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
work="$(mktemp -d "$RUNNER_TEMP/nauterm-rs-signing.XXXXXX")"
export NAUTERM_RS_SIGN_WORK="$work"
# Record cleanup paths before importing anything, including failure paths.
python3 - <<'PY'
import os
with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as output:
    for key in ('NAUTERM_RS_SIGN_WORK', 'NAUTERM_RS_SIGN_IDENTITY'):
        value = os.environ[key]
        if '\n' in value or '\r' in value:
            raise SystemExit('Signing settings must be single-line values.')
        output.write(f'{key}={value}\n')
PY
python3 - <<'PY'
import base64, os
from pathlib import Path
work = Path(os.environ['NAUTERM_RS_SIGN_WORK'])
(work / 'certificate.p12').write_bytes(base64.b64decode(os.environ['NAUTERM_RS_SIGN_CERTIFICATE'], validate=True))
key = os.environ.get('NAUTERM_RS_NOTARY_KEY_BASE64', '')
if key:
    (work / 'notary.p8').write_bytes(base64.b64decode(key, validate=True))
    with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as output:
        values = {'NAUTERM_RS_NOTARY_KEY': str(work / 'notary.p8'),
                  'NAUTERM_RS_NOTARY_KEY_ID': os.environ['NAUTERM_RS_NOTARY_KEY_ID'],
                  'NAUTERM_RS_NOTARY_ISSUER': os.environ['NAUTERM_RS_NOTARY_ISSUER']}
        for name, value in values.items():
            if '\n' in value or '\r' in value:
                raise SystemExit('Notarization settings must be single-line values.')
            output.write(f'{name}={value}\n')
PY
keychain="$work/signing.keychain-db"
keychain_password="$(openssl rand -hex 24)"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work/certificate.p12" -P "$password" -k "$keychain" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_password" "$keychain" >/dev/null
prior=()
while IFS= read -r path; do prior+=("$path"); done < <(security list-keychains -d user | python3 -c 'import shlex,sys; print("\n".join(shlex.split(sys.stdin.read())))')
security list-keychains -d user -s "$keychain" "${prior[@]}"
rm "$work/certificate.p12"
