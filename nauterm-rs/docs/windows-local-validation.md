# Windows x64 local validation

Use PowerShell 7 in an x64 Visual Studio developer environment with the Desktop development
with C++ workload and a Windows SDK (`cl.exe`, MSVC `link.exe`, `rc.exe`).
Install Git, Python 3.10+, rustup and Zig 0.16.0. The check script installs the
repository's pinned Rust toolchain/components in rustup's user directory.
Configure x64 OpenSSL headers, import libraries and runtime DLLs. The same
OpenSSL 4.0 setup helper used in CI now also supports a local shell:

```powershell
./scripts/ci/setup_windows_openssl.ps1 -Arch x86_64
```

Run from the repository root:

```powershell
pwsh -NoProfile -File ./scripts/tests/check_windows_rs.ps1 -Launch
```

The script checks formatting, runs Clippy and RS unit tests, executes the
PowerShell 5.1 fixture and native CMD protocol model tests, builds the x64 executable and stages the pinned
Mosh runtime. `-Launch` opens the application using a new RS data directory
under the reported temporary validation directory. It restores its process
environment on exit. Omitting `-Launch` runs only the build/check phases.

`summary.json` and per-phase logs are stored in the validation directory whose
path is printed when the script exits, including on failure. A successful
script result establishes build/unit-test success and application launch only;
the UI and service checks below still require observation.
CMD model tests do not establish working CMD command capture or AI execution;
the Windows console interceptor and its client integration remain unfinished.

## Manual observations

Record the source revision, Windows version, display scaling and result:

- Main window: minimize/maximize/restore/close; double-click/drag the titlebar;
  resize then click each button once; check Snap Layouts and move between
  displays at different DPI.
- Settings: native titlebar; close/reopen; open an AI provider dialog and check
  that the backdrop stays within the visible window.
- Input: IME composition, CJK/Emoji, AltGr combinations and configured
  shortcuts. Check Nerd Font glyphs and cursor/cell alignment.
- PowerShell 5.1 and 7: run a successful command, a failing native command,
  multiline input and Ctrl+C. Change to a directory containing spaces/Unicode
  and inspect directory tracking. Test AI execution and cancellation on an
  authorized fixture command; confirm reported output and exit status.
- CMD and Git Bash: startup, input, resize and exit. CMD's complete command
  capture remains an implementation gap; do not record it as verified.
- Open With: inspect registered applications and their icons for a text file;
  test System Default and an explicit application. Use Other to select an EXE,
  cancel it, and test paths containing spaces, Unicode and `&`. Re-test after a
  configured executable is moved or removed. Exercise remote-file editing on
  an authorized SSH/SFTP test endpoint.
- Clipboard, native file chooser, local drag/drop and uploads: include Unicode
  paths, drive letters and UNC paths.
- Close/quit/restart and workspace restoration; SSH authentication/reconnect;
  SFTP transfers/favorites/directory linkage; all forwarding modes; AI
  stream/stop/history; recording/playback/retention.
- Run RS alongside Flutter and check their independent config/data directories
  and credential-service identities.
- Test ZIP/installer, upgrade and uninstall separately from this debug build.

Record failures with the action, expected/observed result and the relevant
phase log or screenshot. Credentials and private endpoint details are not
needed in the validation report.
