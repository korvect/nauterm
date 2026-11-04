#!/usr/bin/env python3
"""Stage the dynamically loaded Wayland decorator and GTK plugin in an AppDir."""
import argparse
from pathlib import Path
import shutil
import subprocess


def stage(appdir: Path) -> None:
    cache = subprocess.check_output(["ldconfig", "-p"], text=True)
    libraries = [
        Path(line.split("=>", 1)[1].strip()).resolve()
        for line in cache.splitlines()
        if line.strip().startswith("libdecor-0.so.0 ") and "=>" in line
    ]
    candidates = [
        (library, library.parent / "libdecor/plugins-1/libdecor-gtk.so")
        for library in libraries
    ]
    pair = next(((lib, plugin) for lib, plugin in candidates if plugin.is_file()), None)
    if pair is None:
        raise SystemExit("Install libdecor-0-0 and libdecor-0-plugin-1-gtk before packaging RS")
    library, plugin = pair
    destination = appdir / "usr/lib"
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(library, destination / "libdecor-0.so.0")
    plugin_destination = destination / "libdecor/plugins-1"
    plugin_destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(plugin, plugin_destination / plugin.name)
    # linuxdeploy resolves these ELF files' dependencies. Preserve the
    # distribution notices for both dynamically loaded components.
    for package in ("libdecor-0-0", "libdecor-0-plugin-1-gtk"):
        notice = Path("/usr/share/doc") / package / "copyright"
        if not notice.is_file():
            raise SystemExit(f"Missing distribution copyright notice: {notice}")
        target = appdir / "usr/share/doc" / package
        target.mkdir(parents=True, exist_ok=True)
        shutil.copy2(notice, target / "copyright")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("appdir", type=Path)
    stage(parser.parse_args().appdir)
