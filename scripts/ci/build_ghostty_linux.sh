#!/usr/bin/env bash
set -euo pipefail

case "${1:-$(uname -m)}" in
  x86_64) target=x86_64-linux-gnu ;;
  aarch64|arm64) target=aarch64-linux-gnu ;;
  *) echo "Unsupported Linux architecture: ${1:-$(uname -m)}" >&2; exit 1 ;;
esac

# Keep the release version in sync with GHOSTTY_LIB_VERSION in build.rs.
version=1.0.2
release="libghostty-vt-$version"
base_url="https://release.files.ghostty.org/libghostty-vt/$version"
work_dir="$(mktemp -d "${RUNNER_TEMP:?}/nauterm-ghostty-vt.XXXXXX")"
cd "$work_dir"

for archive in "$release.tar.gz" "$release-deps.tar.gz"; do
  curl --fail --silent --show-error --location --retry 3 --max-time 120 \
    --output "$archive" "$base_url/$archive"
done

# From the signed SHA256SUMS published with this release.
sha256sum --check --strict <<EOF
1dcafa8a3357f3a6bbf4f2696e61de611400c5469518716793dfffd35f008b3e  $release.tar.gz
c56156dbea82fdf71f13bf0d704cdf4393b6a04208d7f9691e0d7f60c7f9ca4d  $release-deps.tar.gz
EOF
tar -xzf "$release.tar.gz"
tar -xzf "$release-deps.tar.gz"

cd "$release"
# Use Ghostty's system packaging path with fresh build caches. All Zig packages
# come from the checked dependency archive. Bundle simdutf in the static library.
ZIG_GLOBAL_CACHE_DIR="$work_dir/global-cache" \
ZIG_LOCAL_CACHE_DIR="$work_dir/local-cache" \
  zig build --system "$work_dir/$release-deps" -fno-sys=simdutf \
    -Demit-lib-vt=true -Demit-xcframework=false -Doptimize=ReleaseFast \
    -Dtarget="$target" -Dcpu=baseline --prefix "$work_dir/install"

lib_dir="$work_dir/install/lib"
if [ ! -s "$lib_dir/libghostty-vt.a" ]; then
  echo "libghostty-vt static library missing for $target" >&2
  exit 1
fi
printf 'NAUTERM_GHOSTTY_VT_LIB_DIR=%s\n' "$lib_dir" >> "${GITHUB_ENV:?}"
