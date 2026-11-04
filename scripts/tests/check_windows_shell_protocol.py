#!/usr/bin/env python3
"""Exercise real Windows ConPTY shell integrations in isolated temporary data.

Pass --cmd-launcher to the compiled native integration helper, --powershell to
PowerShell 5.1/7, or both. No user profile/history, network or application data
is touched. Checks cooked input, command boundaries/status, Unicode CWD, AI
parking/completion/cancellation, resize and exit. Requires Windows 10 1809+.
"""
import argparse
import base64
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time


class COORD(C.Structure):
    _fields_ = [("X", C.c_short), ("Y", C.c_short)]


class STARTUPINFO(C.Structure):
    _fields_ = [("cb", W.DWORD), ("reserved", W.LPWSTR), ("desktop", W.LPWSTR),
               ("title", W.LPWSTR), ("x", W.DWORD), ("y", W.DWORD),
               ("width", W.DWORD), ("height", W.DWORD), ("cx", W.DWORD),
               ("cy", W.DWORD), ("fill", W.DWORD), ("flags", W.DWORD),
               ("show", W.WORD), ("reserved2", W.WORD), ("reserved3", C.c_void_p),
               ("stdin", W.HANDLE), ("stdout", W.HANDLE), ("stderr", W.HANDLE)]


class STARTUPINFOEX(C.Structure):
    _fields_ = [("startup", STARTUPINFO), ("attributes", C.c_void_p)]


class PROCESSINFO(C.Structure):
    _fields_ = [("process", W.HANDLE), ("thread", W.HANDLE),
               ("pid", W.DWORD), ("tid", W.DWORD)]


class ConPTY:
    def __init__(self, arguments, directory, environment):
        self.k = C.WinDLL("kernel32", use_last_error=True)
        # Explicit signatures are essential for 64-bit handles and pointers.
        declarations = {
            "CreatePipe": ([C.POINTER(W.HANDLE), C.POINTER(W.HANDLE), C.c_void_p, W.DWORD], W.BOOL),
            "CreatePseudoConsole": ([COORD, W.HANDLE, W.HANDLE, W.DWORD, C.POINTER(W.HANDLE)], C.c_long),
            "ResizePseudoConsole": ([W.HANDLE, COORD], C.c_long),
            "ClosePseudoConsole": ([W.HANDLE], None),
            "CloseHandle": ([W.HANDLE], W.BOOL),
            "GetStdHandle": ([W.DWORD], W.HANDLE),
            "SetStdHandle": ([W.DWORD, W.HANDLE], W.BOOL),
            "InitializeProcThreadAttributeList": ([C.c_void_p, W.DWORD, W.DWORD, C.POINTER(C.c_size_t)], W.BOOL),
            "UpdateProcThreadAttribute": ([C.c_void_p, W.DWORD, C.c_size_t, C.c_void_p, C.c_size_t, C.c_void_p, C.c_void_p], W.BOOL),
            "DeleteProcThreadAttributeList": ([C.c_void_p], None),
            "CreateProcessW": ([W.LPCWSTR, W.LPWSTR, C.c_void_p, C.c_void_p, W.BOOL, W.DWORD, C.c_void_p, W.LPCWSTR, C.c_void_p, C.POINTER(PROCESSINFO)], W.BOOL),
            "ReadFile": ([W.HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p], W.BOOL),
            "WriteFile": ([W.HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p], W.BOOL),
            "GetExitCodeProcess": ([W.HANDLE, C.POINTER(W.DWORD)], W.BOOL),
            "TerminateProcess": ([W.HANDLE, W.UINT], W.BOOL),
            "WaitForSingleObject": ([W.HANDLE, W.DWORD], W.DWORD),
        }
        for name, (arguments_type, result) in declarations.items():
            function = getattr(self.k, name)
            function.argtypes = arguments_type
            function.restype = result
        self.input = W.HANDLE()
        self.output = W.HANDLE()
        input_read, output_write = W.HANDLE(), W.HANDLE()
        self.check(self.k.CreatePipe(C.byref(input_read), C.byref(self.input), None, 0))
        self.check(self.k.CreatePipe(C.byref(self.output), C.byref(output_write), None, 0))
        self.console = W.HANDLE()
        status = self.k.CreatePseudoConsole(COORD(120, 30), input_read, output_write, 0, C.byref(self.console))
        self.k.CloseHandle(input_read)
        self.k.CloseHandle(output_write)
        if status: raise RuntimeError(f"CreatePseudoConsole failed: {status:#x}")
        size = C.c_size_t()
        self.k.InitializeProcThreadAttributeList(None, 1, 0, C.byref(size))
        attributes = C.create_string_buffer(size.value)
        self.check(self.k.InitializeProcThreadAttributeList(attributes, 1, 0, C.byref(size)))
        self.check(self.k.UpdateProcThreadAttribute(attributes, 0, 0x20016, self.console, C.sizeof(W.HANDLE), None, None))
        startup = STARTUPINFOEX()
        startup.startup.cb = C.sizeof(startup)
        startup.attributes = C.cast(attributes, C.c_void_p)
        command = C.create_unicode_buffer(subprocess.list2cmdline(arguments))
        env = C.create_unicode_buffer("\0".join(f"{key}={value}" for key, value in sorted(environment.items())) + "\0\0")
        self.process = PROCESSINFO()
        # A test host commonly has redirected output. Such handles override
        # ConPTY's standard streams even with bInheritHandles=FALSE. Launch
        # with absent handles so the console loader supplies CONIN/CONOUT.
        saved_handles = [(index, self.k.GetStdHandle(index)) for index in (-10, -11, -12)]
        try:
            for index, _ in saved_handles: self.k.SetStdHandle(index, None)
            self.check(self.k.CreateProcessW(None, command, None, None, False, 0x80000 | 0x400,
                                           env, str(directory), C.byref(startup), C.byref(self.process)))
        finally:
            for index, handle in saved_handles: self.k.SetStdHandle(index, handle)
        self.k.DeleteProcThreadAttributeList(attributes)
        self.k.CloseHandle(self.process.thread)
        self.chunks = queue.Queue()
        self.data = bytearray()
        self.queries = 0
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()

    @staticmethod
    def check(result):
        if not result: raise C.WinError(C.get_last_error())

    def read(self):
        buffer = C.create_string_buffer(65536)
        count = W.DWORD()
        while self.k.ReadFile(self.output, buffer, len(buffer), C.byref(count), None) and count.value:
            self.chunks.put(buffer.raw[:count.value])

    def write(self, data):
        count = W.DWORD()
        self.check(self.k.WriteFile(self.input, data, len(data), C.byref(count), None))
        assert count.value == len(data)

    def pump(self, timeout=.1):
        try: self.data.extend(self.chunks.get(timeout=timeout))
        except queue.Empty: return
        if len(self.data) > 1024 * 1024: raise RuntimeError("Fixture output limit exceeded")
        queries = self.data.count(b"\x1b[6n")
        for _ in range(queries - self.queries): self.write(b"\x1b[1;1R")
        self.queries = queries

    def until(self, marker, start=0, timeout=15):
        deadline = time.monotonic() + timeout
        while marker not in self.data[start:]:
            self.pump()
            if time.monotonic() >= deadline:
                raise RuntimeError(f"Missing {marker!r}; output: {bytes(self.data[start:])!r}")
        return bytes(self.data[start:])

    def settle(self):
        deadline = time.monotonic() + .25
        while time.monotonic() < deadline: self.pump(.03)

    def close(self):
        if self.k.WaitForSingleObject(self.process.process, 0) != 0:
            self.k.TerminateProcess(self.process.process, 1)
            self.k.WaitForSingleObject(self.process.process, 5000)
        self.k.ClosePseudoConsole(self.console)
        self.k.CloseHandle(self.input)
        self.k.CloseHandle(self.output)
        self.k.CloseHandle(self.process.process)


def run(arguments, directory, environment, powershell):
    session = ConPTY(arguments, directory, environment)
    try:
        session.until(b"]133;B;aid=")
        session.settle()

        def command(text, code, ai=False):
            session.settle()
            start = len(session.data)
            session.write(text.encode("utf-8") + b"\r")
            marker = (f"]777;nauterm-command-end=conpty-fixture;{code}" if ai else "]4545;CommandExited;").encode()
            data = session.until(marker, start)
            if b"]4545;CommandStarted;" not in data: raise RuntimeError("Missing command start")
            if not ai and (b";" + str(code).encode() + b"\x07") not in data:
                raise RuntimeError(f"Unexpected command status: {data!r}")
            return data

        command("Write-Output NAUTERM_OK" if powershell else "echo NAUTERM_OK", 0)
        command("cmd.exe /d /c exit 23", 23)
        if powershell:
            command("Write-Error NAUTERM_EXPECTED_FAILURE", 1)
        else:
            command("echo PRESERVES_STATUS", 23)
            command("prompt CUSTOM $P$G", 23)
            session.settle()
            start = len(session.data)
            session.write(b"set /p fixture=ENTER_VALUE: \r")
            session.until(b"ENTER_VALUE:", start)
            session.write(b"fixture input\r")
            data = session.until(b"]4545;CommandExited;", start)
            assert data.count(b"]4545;CommandStarted;") == 1, "set /p input was misclassified as a command"
        text = f"Set-Location -LiteralPath '{str(directory).replace(chr(39), chr(39)*2)}'" if powershell else f'cd /d "{directory}"'
        data = command(text, 0)
        session.until(b"]7;file:///", len(session.data) - len(data))
        if b"%E7%94%A8%E6%88%B7" not in session.data: raise RuntimeError("Unicode directory was not reported")
        # Park unfinished input. It must not execute before or with the AI line.
        session.settle()
        start = len(session.data)
        session.write(b"echo SHOULD_NOT_RUN")
        session.write(b"\x18\x15" if powershell else b"\x18")
        session.until(b"]777;nauterm-line-ready=conpty-fixture\x07", start)
        if not powershell:
            session.write(b"1;conpty-fixture\x19")
            session.until(b"]777;nauterm-command-ready=conpty-fixture\x07", start)
        command("Write-Output NAUTERM_AI_OK" if powershell else "echo NAUTERM_AI_OK", 0, ai=True)
        if not powershell:
            session.settle()
            start = len(session.data)
            session.write(b"\x18")
            session.until(b"]777;nauterm-line-ready=conpty-fixture\x07", start)
            session.write(b"3;conpty-fixture\x19")
            session.until(b"]777;nauterm-command-ready=conpty-fixture\x07", start)
            start = len(session.data)
            session.write(b"echo AI_FIRST\recho AI_SECOND\rcmd.exe /d /c exit 19\r")
            data = session.until(b"]777;nauterm-command-end=conpty-fixture;19\x07", start)
            assert data.count(b"]4545;CommandStarted;") == 3
            assert data.count(b"]777;nauterm-command-end=") == 1, "AI completed before the last CMD line"
            command('cd /d .', 0)
            for submission in [
                b"\recho AI_BLANK\r\r",
                b"echo AI_CARET^\r_CONTINUED\r",
                b"(\recho AI_BLOCK_FIRST\recho AI_BLOCK_LAST\r)\r",
            ]:
                session.settle()
                start = len(session.data)
                session.write(b"\x18")
                session.until(b"]777;nauterm-line-ready=conpty-fixture\x07", start)
                session.write(f"{submission.count(bytes([13]))};conpty-fixture\x19".encode())
                session.until(b"]777;nauterm-command-ready=conpty-fixture\x07", start)
                start = len(session.data)
                session.write(submission)
                data = session.until(b"]777;nauterm-command-end=conpty-fixture;0\x07", start)
                assert data.count(b"]777;nauterm-command-end=") == 1
        # A resize must preserve the reader and completion protocol.
        assert session.k.ResizePseudoConsole(session.console, COORD(96, 24)) == 0
        command("Write-Output RESIZED" if powershell else "echo RESIZED", 0)
        # Cancel a native executable, then observe the next real prompt.
        session.settle()
        start = len(session.data)
        session.write(b"\x18\x15" if powershell else b"\x18")
        session.until(b"]777;nauterm-line-ready=conpty-fixture\x07", start)
        if not powershell:
            session.write(b"1;conpty-fixture\x19")
            session.until(b"]777;nauterm-command-ready=conpty-fixture\x07", start)
        start = len(session.data)
        session.write(b"ping.exe -n 30 127.0.0.1\r")
        session.until(b"]4545;CommandStarted;", start)
        time.sleep(.2)
        session.write(b"\x03")
        session.until(b"]777;nauterm-command-end=conpty-fixture;", start)
        session.settle()
        session.write(b"exit\r")
        assert session.k.WaitForSingleObject(session.process.process, 5000) == 0, "Shell did not exit"
    finally:
        session.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cmd-launcher", type=Path)
    parser.add_argument("--build-cmd", action="store_true", help="Build helper and read target-matched OUT_DIR from Cargo JSON")
    parser.add_argument("--target", help="Optional Cargo target (otherwise honors CARGO_BUILD_TARGET)")
    parser.add_argument("--all-powershell", action="store_true", help="Test system Windows PowerShell and installed pwsh")
    parser.add_argument("--powershell", action="append", default=[])
    args = parser.parse_args()
    if os.name != "nt": parser.error("Requires Windows ConPTY")
    if args.build_cmd:
        command = ["cargo", "build", "-p", "nauterm_cmd_integration", "--locked", "--message-format=json"]
        if args.target: command.extend(["--target", args.target])
        result = subprocess.run(command, check=True, capture_output=True, text=True)
        directories = []
        for line in result.stdout.splitlines():
            event = json.loads(line)
            if event.get("reason") == "build-script-executed" and "nauterm_cmd_integration" in event["package_id"]:
                directories.append(Path(event["out_dir"]))
        if len(directories) != 1: raise RuntimeError("Cargo did not report exactly one CMD helper build")
        args.cmd_launcher = directories[0] / "nauterm-cmd.exe"
    if args.all_powershell:
        args.powershell.append(str(Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"))
        modern = shutil.which("pwsh.exe")
        if modern: args.powershell.append(modern)
    if not args.cmd_launcher and not args.powershell: parser.error("Select CMD and/or PowerShell to test")
    with tempfile.TemporaryDirectory(prefix="nauterm-conpty-") as temporary:
        directory = Path(temporary) / "workspace 用户's #100% & more"
        directory.mkdir()
        environment = dict(os.environ, NAUTERM_CMD_TOKEN="conpty-fixture",
                           NAUTERM_SHELL_INTEGRATION_TOKEN="conpty-fixture",
                           POWERSHELL_TELEMETRY_OPTOUT="1", PSREADLINE_HISTORYSAVE_NOTHING="1")
        # Engines reconstruct their own standard module paths when this is
        # absent. A PS7 host's PSModulePath can otherwise make PS5.1 import the
        # host's incompatible PSReadLine. This affects only fixture children.
        environment = {key: value for key, value in environment.items() if key.casefold() != "psmodulepath"}
        if args.cmd_launcher:
            run([str(args.cmd_launcher.resolve()), os.environ["COMSPEC"]], directory, environment, False)
            print("CMD ConPTY boundaries, status, Unicode cwd, AI, cancel, resize, exit passed.")
        sources = Path(__file__).resolve().parents[2] / "native/nauterm_core/src/pty"
        hooks = (sources / "powershell.ps1").read_text(encoding="utf-8")
        fixture = (sources / "powershell_test.ps1").read_text(encoding="utf-8")
        for powershell in args.powershell:
            script = "$env:NAUTERM_SHELL_INTEGRATION_TOKEN='fixture-token'; function global:prompt { 'fixture> ' };\n" + hooks + "\n" + fixture
            encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
            checked = subprocess.run([powershell, "-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
                                     env=environment, cwd=directory, capture_output=True, timeout=30)
            if checked.returncode or b"NAUTERM_POWERSHELL_PROTOCOL_OK" not in checked.stdout:
                raise RuntimeError(f"PowerShell protocol fixture failed: {checked.stderr!r}")
            script = "Import-Module PSReadLine; Set-PSReadLineOption -HistorySaveStyle SaveNothing;\n" + hooks
            encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
            run([powershell, "-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand", encoded], directory, environment, True)
            print(f"{powershell}: ConPTY boundaries, status, Unicode cwd, AI, cancel, resize, exit passed.")


if __name__ == "__main__": main()
