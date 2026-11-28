#!/usr/bin/env bash
set -euo pipefail

# Keep the release version in sync with GHOSTTY_LIB_VERSION in build.rs.
version=1.0.2
archive="ghostty-vt-$version.xcframework.zip"
url="https://release.files.ghostty.org/libghostty-vt/$version/$archive"
work_dir="$(mktemp -d "${RUNNER_TEMP:?}/nauterm-ghostty-vt.XXXXXX")"
cd "$work_dir"

curl --fail --silent --show-error --location --retry 3 --max-time 120 \
  --output "$archive" "$url"

# From the signed SHA256SUMS published with this release.
printf '%s  %s\n' 15477209ce86ad1c944f217e925c84acf796ecc51aba7a935aaa1a7769890bd6 "$archive" |
  shasum -a 256 --check

lib_dir=ghostty-vt.xcframework/macos-arm64_x86_64
unzip -q "$archive" "$lib_dir/*"
archs="$(lipo -archs "$lib_dir/libghostty-vt.a")"
for arch in arm64 x86_64; do
  if [[ " $archs " != *" $arch "* ]]; then
    echo "libghostty-vt archive missing the $arch slice: $archs" >&2
    exit 1
  fi
done

printf 'NAUTERM_GHOSTTY_VT_LIB_DIR=%s\n' "$work_dir/$lib_dir" >> "${GITHUB_ENV:?}"
