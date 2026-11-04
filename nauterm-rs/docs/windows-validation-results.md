# Windows adaptation evidence

Working branch: `feat/nauterm-rs`, based on `90e2fb1`. This ledger covers the
Windows adaptation and subsequent desktop fixes. It distinguishes backend
checks from desktop observations; the Windows acceptance work is not complete.

## Host and repeatable checks

The local host is Windows 11 Pro x64 (10.0.26300), Visual Studio 2026 Community
18.11.3, Rust 1.99.1, Zig 0.16.0 and OpenSSL 4 x64.

- `make run-rs` from ordinary Git Bash now loads the VS/OpenSSL environment,
  builds, stages checksummed Mosh plus architecture-matched native runtimes and
  starts `nauterm-rs.exe`. No Python is needed by the Windows launch script.
- `cargo test -p nauterm_core --features integration-tests,test-credential-store --locked`
  passes 298 tests on this host. This includes the real loopback SSH/SFTP and
  forwarding tests and the native LocalPty CMD/Git Bash checks.
- `cargo test -p nauterm-rs -p nauterm_cmd_integration --locked` passes 537 RS
  tests and 13 CMD protocol tests. One RS child-process fixture is intentionally
  ignored in the main run and invoked by its parent argv test.
- `cargo clippy -p nauterm_core -p nauterm-rs -p nauterm_cmd_integration --features integration-tests --all-targets --locked -- -D warnings`,
  workspace rustfmt and `git diff --check` pass. The Windows runtime staging
  path now publishes via temporary/renamed files when a running process maps
  the old DLL; Clippy was verified with the locally launched application alive.
- `python scripts/tests/check_windows_shell_protocol.py --build-cmd --all-powershell`
  passes with CMD, Windows PowerShell 5.1 and PowerShell. It exercises real
  ConPTY children, directory reports, native/cmdlet failure status, command
  boundaries, AI line parking/completion, Ctrl+C, resize and exit. CMD also
  covers `set /p`, custom prompts, blank lines, caret continuation and compound
  multiline commands. It uses temporary data and synthetic commands.
- The real Windows credential store check is separate from the mocked core
  suite: `cargo test -p nauterm_core --no-default-features --features integration-tests --lib application::tests::windows_credential_store --locked`.
  It passes: the same unique fixture account retains different values under
  the two client service names, deleting one leaves the other intact, and the
  fixture credentials are cleaned up.
- `scripts/tests/check_windows_runtime.ps1` passes architecture mismatch,
  incomplete runtime and malformed PE fixtures. On this host the resolver finds
  the installed VC145 x64 redistributables instead of assuming VS 2022 paths.
- `python scripts/ci/verify_rs_licenses.py --target x86_64-pc-windows-msvc --offline`
  covers 617 third-party runtime packages. Workspace-owned code uses the root
  LICENSE. Windows reads the notice inventory explicitly as UTF-8.

Local phase logs are under ignored `build/windows-*.log`; see
`windows-local-validation.md` for the repeatable validation entry point.

## Requirement coverage and remaining observations

| Area | Evidence | Still required |
|---|---|---|
| Open With | Real Windows enumeration: `.txt` 18 handlers, `.png` 22; packaged Photos locator preserved; PRI icons for Photos/Notepad/Paint/Snipping Tool decoded to visible pixels. Actual argv subprocess tests preserve Unicode, spaces, `&`, `%PATH%`. File identity rejects replacement while allowing in-place edits. | Exercise Other Application's actual native dialog, cancel, selected application launch and remote-edit UI. |
| Shell integration | Real CMD/PS 5.1/PS ConPTY protocol tests; native LocalPty worker CMD/Git Bash startup/command/resize/exit. Default Windows shell resolution is shared with history/integration/completion. | Exercise command capture and AI execution/cancellation through the GPUI UI, including existing user profiles. |
| Window controls | Windows controls/titlebar use GPUI's native non-client routes; buttons occlude the parent drag hitbox. The previous custom `zoom_window()` call only maximized. Windows/Linux main-window close saves state and drains services before quitting; macOS retains its separate window-close lifecycle. | Recheck restore, minimize, titlebar double-click/drag, Snap Layouts and all windows after the change. |
| Input and displays | Existing AltGr/shortcut policy tests; platform backend uses native Unicode input. | IME composition, AltGr on an applicable keyboard layout, shortcuts and moving between different-DPI displays. |
| Fonts | Existing layout/fallback/grid model tests and bundled Nerd Font setup compile on Windows. | Observe CJK/Emoji/Nerd Font rendering and cell alignment in the real window. |
| File interaction | Native UTF-16 drag/clipboard API routing audited. Real local file planning/completion tests cover Unicode and shell metacharacters. Local upload/download conflict checks handle slash/case/UNC/verbatim spellings. SFTP presents ordinary local paths while preserving native navigation identities, including literal trailing spaces; display/edit/listing round trips are tested. | Actual clipboard, native pickers and drag/drop uploads; UNC file I/O requires an available share. |
| Data/coexistence | Pure client path separation tests; real Windows Credential Manager service separation check. RS startup selects its own identity and APPDATA directory. | Run both clients simultaneously and observe their complete config/database/recovery lifecycles. |
| Workspace lifecycle | Temp-file journal/lease/restore policy tests pass in the RS suite. | Close window, quit, reopen and restart with real sessions, including crash recovery. |
| SSH/SFTP | Real loopback SSH password authentication with strict temporary known_hosts; directory upload/list/download of 2MB binary and Unicode filenames; cancellation preserves the old target and owned partial resumes correctly. | Real UI authentication variants/reconnection, favorites, SSH-current-directory linkage and real-server behavior. |
| Port forwarding | Local/Remote/Dynamic use real SSH/TCP loopback payloads and stop checks; idle disconnect transitions are tested. | Start/stop/reconnect through the UI and check user-facing status. |
| AI | Stream/request/history models plus real local shell execution protocols. | Actual streaming provider roundtrip, stop/history and full terminal UI command execution. |
| Recording | Shared capture/encryption/checkpoint tests pass in the core suite. | Record, replay and apply retention through RS UI across restarts. |
| Packaging | Native runtime staging works for local x64 builds and runtime architecture selection is tested. | Produce/test release ZIP and installer, then installation, upgrade and uninstall; ARM64 build/runtime still needs separate validation. |

The application was confirmed to open a titled Windows window. Computer Use
did not grant access to `nauterm-rs`, so screenshots and desktop input checks
above must not be inferred from successful backend tests.

One fresh Ghostty feature build initially reported a missing lazily fetched
Highway package. Repeating the same pinned Zig build materialized it; subsequent
feature builds succeeded. No global cache was cleared or dependency pin changed.

External Editor settings prefetch and reuse their catalog, publishing choices
before loading icons. Windows uses the system-recommended `.txt` handlers, with
no application-name whitelist or custom Other selector in settings. A native
test compares the catalog with each handler's recommendation status; this host
returns Notepad and Visual Studio Code. The configured text extensions continue
to determine which file actions use the selected application.
