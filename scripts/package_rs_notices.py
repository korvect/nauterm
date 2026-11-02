#!/usr/bin/env python3
"""Stage RS notices and source copies using Cargo's actual compiler artifacts."""
import argparse
import json
from pathlib import Path
import shutil


def stage_notices(project: Path, build_log: Path, destination: Path) -> None:
    assets = project / "nauterm-rs/assets/licenses"
    notices_file = assets / "nauterm-rs-rust.json"
    notices = json.loads(notices_file.read_text(encoding="utf-8"))
    source_packages = {
        (package["name"], package["version"])
        for license_group in notices["licenses"]
        if license_group["id"] == "MPL-2.0"
        for package in license_group["packages"]
    }
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(notices_file, destination / notices_file.name)
    shutil.copy2(project / "assets/licenses/nauterm-mosh-rust.json", destination / "nauterm-mosh-rust.json")
    shutil.copy2(assets / "README.md", destination / "README.md")
    shutil.copy2(assets / "manual/lucide-LICENSE", destination / "lucide-LICENSE")
    shutil.copy2(project / "assets/licenses/manual/ghostty-LICENSE", destination / "ghostty-LICENSE")
    shutil.copy2(project / "assets/licenses/manual/nerd-fonts-LICENSE", destination / "nerd-fonts-LICENSE")
    copied = set()
    with build_log.open(encoding="utf-8") as stream:
        for line in stream:
            artifact = json.loads(line)
            if artifact.get("reason") != "compiler-artifact":
                continue
            package_id = artifact.get("package_id", "")
            name, separator, version = package_id.rsplit("#", 1)[-1].rpartition("@")
            package = name, version
            if not separator or package not in source_packages or package in copied:
                continue
            source = Path(artifact["manifest_path"]).parent
            if (not package_id.startswith("registry+")
                    or source.name != f"{name}-{version}"
                    or source.parent.parent.name != "src"
                    or source.parent.parent.parent.name != "registry"
                    or not (source / "Cargo.toml").is_file()):
                raise RuntimeError(f"Cannot locate covered source for {name} {version}")
            if any(path.is_symlink() for path in source.rglob("*")):
                raise RuntimeError(f"Unexpected symlink in covered source for {name} {version}")
            shutil.copytree(
                source,
                destination / "sources" / f"{name}-{version}",
                ignore=shutil.ignore_patterns(".cargo-ok"),
            )
            copied.add(package)
    (destination / "sources.json").write_text(
        json.dumps([
            {"name": name, "version": version, "license": "MPL-2.0",
             "source": f"sources/{name}-{version}",
             "upstream": f"https://crates.io/crates/{name}/{version}"}
            for name, version in sorted(copied)
        ], indent=2) + "\n",
        encoding="utf-8",
    )
    for path in destination.rglob("*"):
        path.chmod(0o755 if path.is_dir() else 0o644)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    stage_notices(args.project, args.build_log, args.destination)
