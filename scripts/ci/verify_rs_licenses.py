#!/usr/bin/env python3
"""Check that committed RS notices cover the supported runtime dependency graphs."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

TARGETS = (
    "aarch64-apple-darwin", "x86_64-apple-darwin",
    "aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc",
    "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu",
)
# Workspace-owned code ships under the root LICENSE, not third-party notices.
PRIVATE_PACKAGES = {"nauterm_core", "nauterm_cmd_integration", "nauterm-rs"}


def verify(project: Path, targets: list, offline: bool) -> None:
    notices = json.loads((project / "nauterm-rs/assets/licenses/nauterm-rs-rust.json").read_text(encoding="utf-8"))
    included = {
        (package["name"], package["version"])
        for license_group in notices["licenses"]
        for package in license_group["packages"]
    }
    expected = set()
    for target in targets:
        command = [
            "cargo", "tree", "-p", "nauterm-rs", "--locked", "--color", "never",
            "--edges", "normal", "--prefix", "none", "--format", "{p}", "--target", target,
        ]
        if offline:
            command.append("--offline")
        output = subprocess.run(command, cwd=project, check=True, capture_output=True, text=True, encoding="utf-8").stdout
        for line in output.splitlines():
            package = re.match(r"^([A-Za-z0-9_-]+) v([^\s]+)", line)
            if package and package[1] not in PRIVATE_PACKAGES:
                expected.add((package[1], package[2]))
    missing = sorted(expected - included)
    if missing:
        raise RuntimeError(
            "RS license notices are missing: "
            + ", ".join(f"{name} {version}" for name, version in missing)
            + ". Run make generate-rs-licenses."
        )
    print(f"RS notices cover {len(expected)} runtime packages across {len(targets)} target(s).")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", action="append", dest="targets")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    try:
        verify(Path(__file__).resolve().parents[2], args.targets or TARGETS, args.offline)
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        raise SystemExit(error.returncode)
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error))
