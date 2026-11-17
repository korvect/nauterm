#!/usr/bin/env bash
set -euo pipefail

library="${1:?Usage: $0 <path-to-Mosh-dynamic-library>}"

if [ ! -f "$library" ]; then
  echo "Missing Mosh dynamic library: $library" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin)
    symbols="$(nm -gU "$library")"
    ;;
  Linux)
    symbols="$(nm -D --defined-only "$library")"
    ;;
  *)
    echo "ABI symbol inspection is supported on macOS and Linux." >&2
    exit 1
    ;;
esac

expected_symbols=(
  nauterm_mosh_abi_version
  nauterm_mosh_transport_create
  nauterm_mosh_transport_free
  nauterm_mosh_transport_queue_input
  nauterm_mosh_transport_resize
  nauterm_mosh_transport_notify_network_changed
  nauterm_mosh_transport_set_wakeup_callback
  nauterm_mosh_transport_drain
  nauterm_mosh_transport_commit_screen
  nauterm_mosh_transport_clear_pending_output
  nauterm_mosh_bytes_free
  nauterm_mosh_string_free
)

for symbol in "${expected_symbols[@]}"; do
  if ! grep -Eq "[[:space:]]_?${symbol}$" <<<"$symbols"; then
    echo "Missing Mosh ABI export: $symbol" >&2
    exit 1
  fi
done

echo "Mosh ABI exports verified in $library"
