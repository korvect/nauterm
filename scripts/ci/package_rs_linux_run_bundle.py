#!/usr/bin/env python3
"""Build a self-contained development AppDir for `make run-rs`."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import tempfile


def desktop_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def desktop_entry(
    bundle: Path, app_id: str, app_name: str, description: str, icon_hash: str
) -> str:
    exec_path = desktop_escape(str((bundle / "AppRun").resolve()))
    icon = bundle / "usr/share/pixmaps" / f"{app_id}-{icon_hash}.png"
    icon_path = desktop_escape(str(icon.resolve()))
    return (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "NoDisplay=true\n"
        f"Name={app_name}\n"
        f"Comment={description}\n"
        f'Exec="{exec_path}" %U\n'
        f"Icon={icon_path}\n"
        "MimeType=x-scheme-handler/ssh;x-scheme-handler/sftp;x-scheme-handler/telnet;\n"
        "Categories=Network;RemoteAccess;TerminalEmulator;\n"
        "Terminal=false\n"
        f"StartupWMClass={app_id}\n"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--bundle", type=Path, required=True)
    args = parser.parse_args()

    project = args.project.resolve()
    executable = args.executable.resolve()
    bundle = args.bundle.resolve()
    identity = json.loads((project / "nauterm-rs/application.json").read_text(encoding="utf-8"))
    linux = json.loads((project / "nauterm-rs/linux/application.json").read_text(encoding="utf-8"))
    installed_app_id = identity["id"]
    app_id = f"{installed_app_id}.dev"
    app_name = identity["name"]
    description = identity["description"]
    binary_name = linux["binary_name"]
    icon_source = project / linux["icon"]
    if not icon_source.is_file():
        raise SystemExit(f"Missing application icon: {icon_source}")
    icon_hash = hashlib.sha256(icon_source.read_bytes()).hexdigest()[:16]
    icon_name = f"{app_id}-{icon_hash}.png"
    data_home = Path(os.environ.get("XDG_DATA_HOME") or (Path.home() / ".local/share"))
    if not data_home.is_absolute():
        raise SystemExit("XDG_DATA_HOME must be an absolute path")
    user_applications_dir = data_home / "applications"
    user_applications_dir.mkdir(parents=True, exist_ok=True)
    user_desktop_file = user_applications_dir / f"{app_id}.desktop"

    staging = bundle.with_name(f".{bundle.name}.staging-{os.getpid()}")
    if staging.exists():
        shutil.rmtree(staging)
    try:
        executable_dir = staging / "usr/lib" / binary_name
        applications_dir = staging / "usr/share/applications"
        icon_dir = staging / "usr/share/icons/hicolor/1024x1024/apps"
        pixmaps_dir = staging / "usr/share/pixmaps"
        executable_dir.mkdir(parents=True)
        applications_dir.mkdir(parents=True)
        icon_dir.mkdir(parents=True)
        pixmaps_dir.mkdir(parents=True)

        bundled_executable = executable_dir / binary_name
        shutil.copy2(executable, bundled_executable)
        subprocess.run(
            [
                "python3",
                str(project / "scripts/stage_rs_mosh.py"),
                "--project",
                str(project),
                "--executable",
                str(executable),
                "--destination",
                str(executable_dir),
            ],
            check=True,
        )
        shutil.copy2(icon_source, icon_dir / icon_name)
        shutil.copy2(icon_source, pixmaps_dir / icon_name)

        app_run = staging / "AppRun"
        app_run.write_text(
            "#!/usr/bin/env sh\n"
            'HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"\n'
            'export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"\n'
            f"export NAUTERM_RS_DEV_APP_ID={shlex.quote(app_id)}\n"
            f"export BAMF_DESKTOP_FILE_HINT={shlex.quote(str(user_desktop_file))}\n"
            f'exec "$HERE/usr/lib/{binary_name}/{binary_name}" "$@"\n',
            encoding="utf-8",
        )
        app_run.chmod(0o755)

        desktop_file = desktop_entry(bundle, app_id, app_name, description, icon_hash)
        (applications_dir / f"{app_id}.desktop").write_text(desktop_file, encoding="utf-8")
        (staging / f"{app_id}.desktop").write_text(desktop_file, encoding="utf-8")

        if bundle.exists():
            shutil.rmtree(bundle)
        staging.rename(bundle)
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            prefix=f".{app_id}.",
            suffix=".desktop",
            dir=user_applications_dir,
            delete=False,
        ) as output:
            output.write(desktop_file)
            temporary_desktop_file = Path(output.name)
        temporary_desktop_file.chmod(0o644)
        temporary_desktop_file.replace(user_desktop_file)

        # Migrate only the old launcher generated for this AppDir.
        legacy_file = user_applications_dir / f"{installed_app_id}.desktop"
        legacy_entry = desktop_entry(bundle, installed_app_id, app_name, description, icon_hash)
        try:
            if not legacy_file.is_symlink() and legacy_file.read_bytes() == legacy_entry.encode():
                legacy_file.unlink()
        except FileNotFoundError:
            pass
    finally:
        if staging.exists():
            shutil.rmtree(staging)


if __name__ == "__main__":
    main()
