#!/usr/bin/env python3
"""Opt-in PowerShell protocol check on a Unix PTY, with isolated state.

Usage: python3 scripts/tests/check_powershell_protocol.py --powershell /path/to/pwsh
Windows PowerShell 5.1's protocol fixture is also run by the core Windows tests.
"""
import argparse
import base64
import errno
import os
from pathlib import Path
import select
import shutil
import subprocess
import tempfile
import time


def encoded(script):
    return base64.b64encode(script.encode("utf-16le")).decode("ascii")


def interactive(powershell, script, environment, directory):
    import fcntl
    import pty
    import struct
    import termios

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
    process = subprocess.Popen(
        [powershell, "-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand", encoded(script)],
        stdin=slave, stdout=slave, stderr=slave, env=environment, cwd=directory,
        start_new_session=True,
    )
    os.close(slave)
    output = bytearray()
    queries = 0

    def pump(timeout):
        nonlocal queries
        if not select.select([master], [], [], timeout)[0]:
            return
        try:
            chunk = os.read(master, 65536)
        except OSError as error:
            # Linux can close the PTY just before waitpid observes child exit.
            if error.errno == errno.EIO:
                return
            raise
        output.extend(chunk)
        if len(output) > 256 * 1024:
            raise RuntimeError("PowerShell fixture exceeded its output limit")
        # Console cursor queries can span reads. Reply as the terminal emulator
        # would, before submitting more input to PSReadLine.
        count = output.count(b"\x1b[6n")
        for _ in range(count - queries):
            os.write(master, b"\x1b[1;1R")
        queries = count

    def until(marker, start):
        deadline = time.monotonic() + 12
        while marker not in output[start:]:
            if process.poll() is not None:
                raise RuntimeError("PowerShell exited before the expected protocol event")
            if time.monotonic() >= deadline:
                raise RuntimeError(f"PowerShell did not emit {marker!r}")
            pump(0.05)

    def settle():
        deadline = time.monotonic() + 0.3
        while time.monotonic() < deadline and process.poll() is None:
            pump(0.02)

    try:
        until(b"]133;B;aid=", 0)
        settle()
        start = len(output)
        os.write(master, b"Write-Output 'NAUTERM_PTY_OK'\r")
        until(b"]4545;CommandExited;", start)
        if b"]4545;CommandStarted;" not in output[start:] or b";0\x07" not in output[start:]:
            raise RuntimeError("Missing successful command boundaries")
        settle()
        start = len(output)
        os.write(master, b"\x18\x15")
        until(b"]777;nauterm-line-ready=pty-fixture\x07", start)
        start = len(output)
        os.write(master, b"Write-Output 'NAUTERM_AI_PTY_OK'\r")
        until(b"]777;nauterm-command-end=pty-fixture;0\x07", start)
        settle()
        os.write(master, b"exit\r")
        deadline = time.monotonic() + 5
        while process.poll() is None and time.monotonic() < deadline:
            pump(0.05)
        if process.poll() is None:
            raise RuntimeError("PowerShell did not exit after the test")
        if process.returncode != 0:
            raise RuntimeError("PowerShell fixture exited unsuccessfully")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--powershell", default="pwsh")
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("This PTY check requires Unix; use the core Windows unit tests on Windows")
    powershell = shutil.which(args.powershell)
    if not powershell:
        parser.error("PowerShell executable was not found")
    powershell = str(Path(powershell).resolve())
    root = Path(__file__).resolve().parents[2] / "native/nauterm_core/src/pty"
    hooks = (root / "powershell.ps1").read_text(encoding="utf-8")
    fixture = (root / "powershell_test.ps1").read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="nauterm-powershell-check-") as temporary:
        base = Path(temporary)
        directory = base / "workspace 用户's #100%"
        directory.mkdir()
        runtime = base / "runtime"
        runtime.mkdir(mode=0o700)
        environment = dict(os.environ, XDG_CACHE_HOME=str(base / "cache"),
                           XDG_CONFIG_HOME=str(base / "config"), XDG_DATA_HOME=str(base / "data"),
                           XDG_RUNTIME_DIR=str(runtime), POWERSHELL_TELEMETRY_OPTOUT="1", TERM="xterm-256color")
        script = "$env:NAUTERM_SHELL_INTEGRATION_TOKEN='fixture-token'; function global:prompt { 'fixture> ' }\n" + hooks + "\n" + fixture
        result = subprocess.run([powershell, "-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded(script)],
                                env=environment, cwd=directory, capture_output=True, timeout=30)
        if result.returncode or b"NAUTERM_POWERSHELL_PROTOCOL_OK" not in result.stdout:
            raise RuntimeError("PowerShell protocol fixture failed: " + result.stderr.decode(errors="replace"))
        interactive(powershell, "$env:NAUTERM_SHELL_INTEGRATION_TOKEN='pty-fixture';\n" + hooks, environment, directory)
    print("PowerShell protocol, interactive command boundaries and AI handshake passed.")


if __name__ == "__main__":
    main()
