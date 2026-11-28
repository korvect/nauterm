#!/usr/bin/env bash
# Compatibility entry point; both clients now use the same native crate.
set -euo pipefail
repository="$(cd "$(dirname "$0")/../.." && pwd)"
exec cargo build --manifest-path "$repository/Cargo.toml" -p nauterm_core "$@"
