#!/usr/bin/env python3
"""Embed the same pinned Sparkle SDK as Flutter, with an RS-only update feed."""
import argparse
import base64
import hashlib
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
import urllib.request

VERSION = "2.9.4"
# Sparkle's pinned Package.swift binaryTarget checksum.
SHA256 = "cb6fdbdc8884f15d62a616e79face92b08322410fd2d425edc6596ccbf4ba3b0"
URL = f"https://github.com/sparkle-project/Sparkle/releases/download/{VERSION}/Sparkle-for-Swift-Package-Manager.zip"
RELATIVE = Path("Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework")


def sdk(project: Path) -> Path:
    override = os.environ.get("NAUTERM_SPARKLE_FRAMEWORK_DIR")
    cache = project / "target/sparkle" / VERSION
    candidates = [Path(override)] if override else [
        project / "build/macos/SourcePackages/artifacts/sparkle/Sparkle" / RELATIVE,
        cache / RELATIVE,
    ]
    for candidate in candidates:
        if candidate.is_dir():
            with (candidate / "Resources/Info.plist").open("rb") as stream:
                version = plistlib.load(stream)["CFBundleShortVersionString"]
            if version != VERSION:
                raise RuntimeError(f"Expected Sparkle {VERSION}, found {version}")
            return candidate
    if override:
        raise RuntimeError("NAUTERM_SPARKLE_FRAMEWORK_DIR must point to Sparkle.framework")
    cache.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="sparkle-download-", dir=cache.parent) as temporary:
        work = Path(temporary)
        archive = work / "sdk.zip"
        request = urllib.request.Request(URL, headers={"User-Agent": "Nauterm-RS-build"})
        with urllib.request.urlopen(request, timeout=120) as response, archive.open("wb") as output:
            shutil.copyfileobj(response, output)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
            raise RuntimeError("Sparkle SDK checksum mismatch")
        unpacked = work / "sdk"
        subprocess.run(["ditto", "-x", "-k", str(archive), str(unpacked)], check=True)
        if not (unpacked / RELATIVE).is_dir():
            raise RuntimeError("Sparkle SDK archive has no macOS framework")
        if cache.exists():
            raise RuntimeError("Incomplete Sparkle SDK cache; remove it before retrying")
        unpacked.rename(cache)
    return cache / RELATIVE


def feed(repository: str, arch: str, public_key: str) -> dict:
    if not public_key:
        return {"SUEnableAutomaticChecks": False}
    parts = repository.split("/")
    if (len(parts) != 2 or any(not part or part in (".", "..")
            or any(not (c.isascii() and (c.isalnum() or c in "_.-")) for c in part)
            for part in parts)):
        raise ValueError("NAUTERM_UPDATE_REPOSITORY must be owner/repository")
    if len(base64.b64decode(public_key, validate=True)) != 32:
        raise ValueError("SPARKLE_PUBLIC_ED_KEY must be a base64 Ed25519 public key")
    if arch not in ("arm64", "x86_64"):
        raise ValueError("Sparkle requires a single supported macOS architecture")
    return {
        "SUFeedURL": f"https://github.com/{repository}/releases/latest/download/appcast-rs-{arch}.xml",
        "SUPublicEDKey": public_key,
        "SUEnableAutomaticChecks": False,
    }


def stage(project: Path, app: Path, identity: str) -> None:
    source = sdk(project)
    destination = app / "Contents/Frameworks/Sparkle.framework"
    shutil.copytree(source, destination, symlinks=True)
    notices = app / "Contents/Resources/licenses"
    notices.mkdir(parents=True, exist_ok=True)
    shutil.copy2(project / "assets/licenses/manual/sparkle-LICENSE", notices / "sparkle-LICENSE")
    executable = app / "Contents/MacOS/nauterm-rs"
    arch = subprocess.check_output(["lipo", "-archs", str(executable)], text=True).strip()
    info_path = app / "Contents/Info.plist"
    with info_path.open("rb") as stream:
        info = plistlib.load(stream)
    info.update(feed(os.environ.get("NAUTERM_UPDATE_REPOSITORY", ""), arch,
                     os.environ.get("SPARKLE_PUBLIC_ED_KEY", "").strip()))
    with info_path.open("wb") as stream:
        plistlib.dump(info, stream, sort_keys=False)
    signing = ["codesign", "--force", "--sign", identity]
    signing += ["--timestamp=none"] if identity == "-" else ["--options", "runtime", "--timestamp"]
    version = destination / "Versions/B"
    # Sign nested code before its containing bundle; retain the SDK's symlinks.
    for path in [
        version / "XPCServices/Downloader.xpc",
        version / "XPCServices/Installer.xpc",
        version / "Autoupdate",
        version / "Updater.app",
        destination,
    ]:
        subprocess.run([*signing, str(path)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(destination)], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--identity", required=True)
    args = parser.parse_args()
    stage(args.project, args.app, args.identity)
