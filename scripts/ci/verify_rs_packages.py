#!/usr/bin/env python3
"""Require both architectures of every RS release format before publishing."""
import argparse
from pathlib import Path
import re


def verify(directory: Path, version: str) -> None:
    files = {path.name for path in directory.iterdir() if path.is_file()}
    escaped = re.escape(version)
    patterns = []
    for arch in ("x86_64", "arm64"):
        patterns.extend([
            rf"Nauterm-RS-{escaped}-macos-{arch}\.app\.zip",
            rf"appcast-rs-{arch}\.xml",
            rf"Nauterm-RS-{escaped}-windows-{arch}\.zip",
            rf"Nauterm-RS-{escaped}-windows-{arch}-setup\.exe",
            rf"Nauterm-RS-{escaped}-linux-{arch}\.AppImage\.tar\.gz",
            rf"nauterm-rs_{escaped}-[1-9][0-9]{{0,4}}_linux-{arch}\.deb",
            rf"nauterm-rs-{escaped}-[1-9][0-9]{{0,4}}\.linux-{arch}\.rpm",
        ])
    for pattern in patterns:
        matches = [name for name in files if re.fullmatch(pattern, name)]
        if len(matches) != 1:
            raise ValueError(f"Expected exactly one RS asset matching {pattern}; found {len(matches)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("version")
    args = parser.parse_args()
    verify(args.directory, args.version)
    print("RS release inventory verified for all six targets.")
