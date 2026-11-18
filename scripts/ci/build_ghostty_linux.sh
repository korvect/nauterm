#!/usr/bin/env bash
set -euo pipefail

case "${1:-$(uname -m)}" in
  x86_64) target=x86_64-linux-gnu ;;
  aarch64|arm64) target=aarch64-linux-gnu ;;
  *) echo "Unsupported Linux architecture: ${1:-$(uname -m)}" >&2; exit 1 ;;
esac

# Keep the release version in sync with GHOSTTY_LIB_VERSION in build.rs.
release=libghostty-vt-1.0.1
base_url=https://release.files.ghostty.org/libghostty-vt/1.0.1
work_dir="$(mktemp -d "${RUNNER_TEMP:?}/nauterm-ghostty-vt.XXXXXX")"
cd "$work_dir"

for archive in "$release.tar.gz" "$release-deps.tar.gz"; do
  curl --fail --silent --show-error --location --retry 3 --max-time 120 \
    --output "$archive" "$base_url/$archive"
done

# From the signed SHA256SUMS published with this release.
sha256sum --check --strict <<'EOF'
333dfce97641ef9b536bb9e08907ad86abaaada50e8f7417b70d9284adef37aa  libghostty-vt-1.0.1.tar.gz
91a4e10960a52bf5bf4b15aa2a2cddaff8a89d6cd903d3055bda4fd17de3356d  libghostty-vt-1.0.1-deps.tar.gz
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
