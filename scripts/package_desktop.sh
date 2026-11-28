#!/usr/bin/env bash
set -euo pipefail

client="${1:?client is required}"
case "$client" in
  flutter|rs) ;;
  *) echo 'Client must be flutter or rs.' >&2; exit 2 ;;
esac

platform="$(uname -s)"
host_arch="$(uname -m)"
case "$platform" in
  MINGW*|MSYS*|CYGWIN*) host_arch="${PROCESSOR_ARCHITECTURE:-$host_arch}" ;;
esac
case "$host_arch" in
  aarch64|ARM64) host_arch=arm64 ;;
  amd64|AMD64) host_arch=x86_64 ;;
esac

arch="${2:-$host_arch}"
case "$arch" in
  arm64|x86_64) ;;
  *) echo "Unsupported package architecture: $arch. Use arm64 or x86_64." >&2; exit 2 ;;
esac

if [[ "${NAUTERM_APPIMAGE_ONLY:-0}" == 1 && "$platform" != Linux ]]; then
  echo 'AppImage packaging requires Linux.' >&2
  exit 2
fi

case "$platform" in
  Darwin)
    if [[ "$arch" != "$host_arch" ]]; then
      echo "macOS package architecture $arch requires a matching host; current host is $host_arch." >&2
      exit 1
    fi
    if [[ "$client" == rs ]]; then
      exec bash nauterm-rs/scripts/bundle-macos.sh release
    fi
    exec bash scripts/ci/package_macos.sh "$arch"
    ;;
  Linux)
    case "$arch" in
      arm64) set -- arm64 arm64 aarch64 aarch64 ;;
      x86_64) set -- x86_64 amd64 x86_64 x86_64 ;;
    esac
    exec bash scripts/ci/package_linux.sh "$@" "$client"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    exec pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/ci/package_windows.ps1 \
      -Arch "$arch" -Client "$client"
    ;;
  *)
    echo "Unsupported packaging platform: $platform." >&2
    exit 2
    ;;
esac
