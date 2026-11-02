#!/usr/bin/env sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mosh_dir=${NAUTERM_MOSH_SOURCE_DIR:-"$project_dir/../nauterm-mosh"}
config="$project_dir/licenses/about.toml"
template="$project_dir/licenses/rust-licenses.hbs"
client=${1:-all}
case "$client" in
  all|flutter|rs) ;;
  *) echo 'Usage: scripts/generate_rust_licenses.sh [all|flutter|rs]' >&2; exit 2 ;;
esac

if ! command -v cargo-about >/dev/null 2>&1; then
  echo 'cargo-about is required: cargo install --locked --version 0.9.1 --features cli cargo-about' >&2
  exit 1
fi
if [ "$client" != rs ] && [ ! -f "$mosh_dir/Cargo.toml" ]; then
  echo "nauterm-mosh source not found at $mosh_dir" >&2
  exit 1
fi

# Keep previously generated notices intact if harvesting or rendering fails.
generate() (
  manifest=$1
  output=$2
  mkdir -p "$(dirname -- "$output")"
  temporary=$(mktemp "$(dirname -- "$output")/.rust-notices.XXXXXX")
  trap 'rm -f "$temporary"' EXIT
  offline_arg=''
  if [ "${NAUTERM_LICENSE_OFFLINE:-0}" = 1 ]; then offline_arg=--offline; fi
  cargo about generate \
    --all-features --locked --fail $offline_arg \
    --config "$config" --manifest-path "$manifest" \
    "$template" -o "$temporary"
  chmod 644 "$temporary"
  mv "$temporary" "$output"
)

if [ "$client" != rs ]; then
  generate "$project_dir/native/nauterm_core/Cargo.toml" "$project_dir/assets/licenses/nauterm-rust.json"
  generate "$mosh_dir/Cargo.toml" "$project_dir/assets/licenses/nauterm-mosh-rust.json"
fi
if [ "$client" != flutter ]; then
  generate "$project_dir/nauterm-rs/Cargo.toml" "$project_dir/nauterm-rs/assets/licenses/nauterm-rs-rust.json"
fi
printf '%s\n' "Generated Rust license notices for $client"
