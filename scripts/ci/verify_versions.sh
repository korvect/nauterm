#!/usr/bin/env bash
set -euo pipefail

project_root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
cd "$project_root"

pubspec_version="$(awk '/^version: / { print $2; exit }' pubspec.yaml)"
version="${pubspec_version%%+*}"
rust_version="$(awk '
  /^\[workspace.package\]/ { package = 1; next }
  /^\[/ { package = 0 }
  package && /^version = / { gsub(/["[:space:]]/, "", $3); print $3; exit }
' Cargo.toml)"
for manifest in native/nauterm_core/Cargo.toml nauterm-rs/Cargo.toml; do
  if ! rg -q '^version.workspace = true$' "$manifest"; then
    echo "$manifest must inherit the workspace release version" >&2
    exit 1
  fi
done

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "pubspec.yaml contains an invalid release version: $version" >&2
  exit 1
fi

if [[ "$rust_version" != "$version" ]]; then
  echo "Version mismatch: pubspec.yaml=$version Cargo workspace=$rust_version" >&2
  exit 1
fi

for package in nauterm_core nauterm-rs; do
  locked_version="$(awk -v package="$package" '
    /^\[\[package\]\]/ { selected = 0 }
    $0 == "name = \"" package "\"" { selected = 1; next }
    selected && /^version = / { gsub(/["[:space:]]/, "", $3); print $3; exit }
  ' Cargo.lock)"
  if [[ "$locked_version" != "$version" ]]; then
    echo "Cargo.lock version mismatch for $package: expected $version, got ${locked_version:-<missing>}" >&2
    exit 1
  fi
done

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  expected_tag="v$version"
  if [[ "${GITHUB_REF_NAME:-}" != "$expected_tag" ]]; then
    echo "Tag mismatch: expected $expected_tag, got ${GITHUB_REF_NAME:-<empty>}" >&2
    exit 1
  fi
fi

if [[ $# -gt 0 ]]; then
  artifact_root="$1"
  while IFS= read -r artifact; do
    filename="$(basename -- "$artifact")"
    case "$filename" in
      *.dmg|*.zip|*.deb|*.rpm|*.AppImage|*.AppImage.tar.gz|*.exe)
        if [[ "$filename" != *"$version"* ]]; then
          echo "Package filename does not contain version $version: $filename" >&2
          exit 1
        fi
        ;;
    esac
  done < <(find "$artifact_root" -type f -print)
fi

echo "Release version verified: $version"
