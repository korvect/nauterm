#!/usr/bin/env python3
"""Verify release package inventories for the requested clients."""

import argparse
import os
from pathlib import Path
import re

CLIENTS = {
    "flutter": ("Nauterm", "nauterm", "appcast"),
    "rs": ("Nauterm-RS", "nauterm-rs", "appcast-rs"),
}
PLATFORMS = ("macos", "linux", "windows")
ARCHITECTURES = ("x86_64", "arm64")


def package_names(client, platform, arch, version, build, include_appcast):
    product, binary, appcast = CLIENTS[client]
    prefix = f"{product}-{version}-{platform}-{arch}"
    if platform == "macos":
        names = [f"{prefix}.app.zip", f"{prefix}.dmg"]
        if include_appcast:
            names.append(f"{appcast}-{arch}.xml")
        return names
    if platform == "windows":
        return [f"{prefix}.zip", f"{prefix}-setup.exe"]
    return [
        f"{prefix}.AppImage.tar.gz",
        f"{binary}_{version}-{build}_linux-{arch}.deb",
        f"{binary}-{version}-{build}.linux-{arch}.rpm",
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("version")
    parser.add_argument("build", type=int)
    parser.add_argument("--client", choices=("all", "flutter", "rust"), default="all")
    parser.add_argument("--platform", choices=PLATFORMS)
    parser.add_argument("--arch", choices=ARCHITECTURES)
    args = parser.parse_args()
    if bool(args.platform) != bool(args.arch):
        parser.error("--platform and --arch must be specified together")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", args.version):
        parser.error("invalid release version")
    if not 1 <= args.build <= 65535:
        parser.error("build number must be from 1 through 65535")

    tag = os.environ.get("GITHUB_REF_TYPE") == "tag"
    platforms = (args.platform,) if args.platform else PLATFORMS
    architectures = (args.arch,) if args.arch else ARCHITECTURES
    selected_client = "rs" if args.client == "rust" else args.client
    packages = {
        client: [
            args.directory / name
            for platform in platforms
            for arch in architectures
            for name in package_names(client, platform, arch, args.version, args.build, tag)
        ]
        for client in CLIENTS
        if selected_client in ("all", client)
    }
    missing = [path for paths in packages.values() for path in paths if not path.is_file()]
    if missing:
        parser.exit(1, "Missing release packages:\n" + "\n".join(map(str, missing)) + "\n")

    # A matrix job uses the verified paths as its exact upload list.
    if args.platform and (output := os.environ.get("GITHUB_OUTPUT")):
        with open(output, "a", encoding="utf-8") as stream:
            for client, paths in packages.items():
                stream.write(f"{client}<<PACKAGE_PATHS\n")
                stream.writelines(f"{path.as_posix()}\n" for path in paths)
                stream.write("PACKAGE_PATHS\n")
    print(f"Release package inventories verified for client={args.client}.")


if __name__ == "__main__":
    main()
