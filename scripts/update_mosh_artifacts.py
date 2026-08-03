#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
import re
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ARTIFACT_ROOT = ROOT / "third_party" / "nauterm_mosh_ffi"
REPOSITORY = "korvect/nauterm-mosh"
EXPECTED_SCHEMA_VERSION = 1
EXPECTED_ABI_VERSION = 1
TARGETS = {
    ("macos", "arm64"): ("macos-arm64", "libnauterm_mosh_ffi.dylib"),
    ("macos", "x86_64"): ("macos-x86_64", "libnauterm_mosh_ffi.dylib"),
    ("linux", "x86_64"): ("linux-x86_64", "libnauterm_mosh_ffi.so"),
    ("linux", "arm64"): ("linux-arm64", "libnauterm_mosh_ffi.so"),
    ("windows", "x86_64"): ("windows-x86_64", "nauterm_mosh_ffi.dll"),
    ("windows", "arm64"): ("windows-arm64", "nauterm_mosh_ffi.dll"),
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Update Nauterm's bundled Mosh libraries from a GitHub Release."
    )
    parser.add_argument("tag", help="release tag, for example v0.1.0")
    return parser.parse_args()


def download(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "Nauterm"})
    with urllib.request.urlopen(request) as response:
        return response.read()


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def extract_library(archive_path: Path, library: str) -> bytes:
    required_files = {library, "LICENSE", "PROVENANCE.md"}
    if archive_path.name.endswith(".tar.gz"):
        with tarfile.open(archive_path, "r:gz") as archive:
            names = set(archive.getnames())
            if not required_files.issubset(names):
                raise SystemExit(f"incomplete release archive: {archive_path.name}")
            source = archive.extractfile(library)
            if source is None:
                raise SystemExit(f"missing library in {archive_path.name}: {library}")
            return source.read()
    if archive_path.suffix == ".zip":
        with zipfile.ZipFile(archive_path) as archive:
            names = set(archive.namelist())
            if not required_files.issubset(names):
                raise SystemExit(f"incomplete release archive: {archive_path.name}")
            return archive.read(library)
    raise SystemExit(f"unsupported release archive: {archive_path.name}")


def atomic_write(path: Path, data: bytes, mode: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = path.with_name(f".{path.name}.tmp")
    temporary_path.write_bytes(data)
    temporary_path.chmod(mode)
    os.replace(temporary_path, path)


def main() -> None:
    args = parse_args()
    if not re.fullmatch(r"v[^\s]+", args.tag):
        raise SystemExit("tag must start with v and contain no whitespace")

    release_base = f"https://github.com/{REPOSITORY}/releases/download/{args.tag}"
    manifest = json.loads(download(f"{release_base}/release-manifest.json"))
    if manifest.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        raise SystemExit("unsupported release manifest schema")
    if manifest.get("tag") != args.tag:
        raise SystemExit("release manifest tag mismatch")
    if manifest.get("ffi_abi_version") != EXPECTED_ABI_VERSION:
        raise SystemExit("release FFI ABI version mismatch")

    revision = manifest.get("source_revision")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise SystemExit("release manifest has an invalid source revision")

    assets = manifest.get("assets")
    if not isinstance(assets, list):
        raise SystemExit("release manifest has no asset list")
    assets_by_target = {(asset.get("os"), asset.get("arch")): asset for asset in assets}
    if set(assets_by_target) != set(TARGETS):
        raise SystemExit("release manifest target set does not match Nauterm")

    staged_libraries = {}
    with tempfile.TemporaryDirectory(prefix="nauterm-mosh-") as temporary:
        temporary_root = Path(temporary)
        for target, (directory, expected_library) in TARGETS.items():
            asset = assets_by_target[target]
            file_name = asset.get("file")
            library = asset.get("library")
            expected_hash = asset.get("sha256")
            expected_size = asset.get("size")
            if not isinstance(file_name, str) or Path(file_name).name != file_name:
                raise SystemExit(f"invalid release asset name for {target}")
            if library != expected_library:
                raise SystemExit(f"unexpected library name for {target}: {library}")
            archive_data = download(f"{release_base}/{file_name}")
            if len(archive_data) != expected_size or sha256(archive_data) != expected_hash:
                raise SystemExit(f"release asset verification failed: {file_name}")
            archive_path = temporary_root / file_name
            archive_path.write_bytes(archive_data)
            staged_libraries[(directory, library)] = extract_library(
                archive_path,
                library,
            )

    checksum_lines = []
    for (directory, library), library_data in staged_libraries.items():
        destination = ARTIFACT_ROOT / directory / library
        mode = 0o755 if destination.suffix in {".dylib", ".so"} else 0o644
        atomic_write(destination, library_data, mode)
        relative_path = destination.relative_to(ROOT)
        checksum_lines.append(f"{sha256(library_data)}  {relative_path.as_posix()}")

    atomic_write(
        ARTIFACT_ROOT / "SHA256SUMS",
        ("\n".join(checksum_lines) + "\n").encode(),
        0o644,
    )
    atomic_write(
        ARTIFACT_ROOT / "SOURCE_REVISION",
        f"{revision}\n".encode(),
        0o644,
    )
    print(f"Updated bundled Mosh libraries from {REPOSITORY} {args.tag} ({revision})")


if __name__ == "__main__":
    main()
