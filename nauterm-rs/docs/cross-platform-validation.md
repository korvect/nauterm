# Cross-platform validation

Branch: `feat/nauterm-rs`. A build or unit test does not establish that a
platform interaction works. Keep an item open until the stated runtime checks
have been performed on that platform. Use isolated RS data for test sessions;
never point a test at the Flutter client's data or credential service.

## Implementation and current evidence

| Item | Current implementation/evidence | Remaining verification/work |
| --- | --- | --- |
| Linux Open With | GIO MIME associations, desktop-entry launch, XDG icon-theme lookup and native launcher-file chooser are wired into local and remote file actions. Real TXT/MD/RS/JSON/PNG association and icon-cache inspection passed; TXT and PNG defaults agree with `gio mime`. Runtime inspection exposed and fixed an incorrect menu-visibility filter that excluded Shotwell Viewer (`NoDisplay=true`). Isolated GIO subprocess fixtures verify literal Unicode/special-character filename transmission through `%f/%F/%u/%U` and missing/mismatched launcher errors. Saved command editors now resolve icons through their registered executable, including a same-installation `bin/code` CLI wrapper versus the desktop `code` binary; reapplying settings refreshes option image snapshots. | The user confirmed icons are now visible except for VS Code. The installed VS Code CLI resolves to `/usr/share/code/bin/code`, while its desktop entry uses `/usr/share/code/code`; the new wrapper-matching regression covers that layout. Visually recheck VS Code after restarting the updated build. Also verify real default/explicit application windows, chooser cancellation and remote editing. Cache presence and fixture launches do not prove correct icon rendering or actual editor behavior. |
| Windows Open With | Shell association enumeration/default detection; native icons converted to cached PNGs; executable chooser; re-query of the saved handler before invocation. Shared identity/icon tests pass. Open With sources have been type-checked against the x64 and ARM64 Windows API targets using a minimal GPUI owner stub. | Full Windows application build and runtime verification: registered/packaged applications, chooser cancellation, disappeared handlers, Unicode paths and remote editing. |
| PowerShell integration | Per-session startup injection; PSReadLine command boundaries; prompt exit status and directory URI; existing-session AI tracking. Protocol fixture and interactive PTY command/AI handshake passed with portable PowerShell 7.6.9 on Linux. Windows PowerShell fixture is included in Windows unit tests. | Run on Windows PowerShell 5.1 and PowerShell 7 through ConPTY; verify Ctrl+C, multiline input, profile customizations and AI cancellation. |
| CMD integration | No complete command-boundary/exit-code hook yet. | Implement reliable callbacks and AI execution feedback; changing PROMPT alone does not supply a complete protocol. |
| Main-window controls | Linux/Windows client controls, Flutter sizing and glyphs; native Windows hit regions. Linux build and content-inset geometry test pass. | Verify click/drag/resize/maximize/restore, tiling and Windows Snap Layouts. |
| Settings titlebar | Standard opaque titlebars on Wayland compositors without xdg-decoration now use libdecor and its GTK plugin through a local backend patch. The user verified titlebar appearance and interactions in the native-titlebar example, and confirmed the actual Settings maximize/restore animation after removing an unintended transient parent. | Verify repeated open/close, modal overlays, scale changes and X11/KDE server-decoration behavior. |
| Settings resize | Wayland geometry changes now invalidate the old presentation, allowing fresh size buffers to bypass the inactive-window throttle during compositor resize grabs. Development builds optimize GPUI, sum-tree and Taffy. The user reported some improvement in both libdecor and client-border comparison instances. | Still not completely smooth. Further tuning was deferred at the user's request; do not record this as fully verified or resolved. |
| Modal backdrop | Deferred modal bounds now exclude the client shadow/frame insets. | Visually verify both main and Settings windows, including maximized and partially tiled windows. |

## Platform matrix

Record OS/compositor, scale, architecture, application revision and the observed
result for each run. No entries below imply verification merely from existing
code or a green CI build.

| Scope | Linux runtime checks | Windows runtime checks |
| --- | --- | --- |
| Build | Install the GPUI/Vulkan/X11/Wayland prerequisites in `.github/workflows/quality-gates.yml`; build both supported architectures. | MSVC, Windows SDK/`rc.exe`, OpenSSL and pinned Zig; build x86_64 and arm64. |
| Window/input | X11 and Wayland; main and Settings decoration; modal margins; dragging/resizing/tiling; IME; multiple monitors and scale changes. | Titlebar actions, Snap Layouts, DPI transitions, IME, AltGr and application shortcuts. |
| Local terminal | Bash/Zsh/Fish startup; command boundaries and current directory; exit and resize. | ConPTY; PowerShell 5.1/7, CMD and Git Bash; command boundaries/current directory; exit and resize. |
| Fonts | Nerd Font glyphs, CJK and Emoji fallbacks; cursor and cell alignment. | Same checks, including mixed-width text and scale changes. |
| File interaction | Clipboard, drag-and-drop upload, native file dialogs, Open With. | Same checks, plus drive letters, backslashes, UNC paths and Unicode filenames. |
| Data | Keyring and XDG config/data locations. | Credential service and APPDATA config/data locations. |
| Packaging | AppImage, DEB and RPM installation and execution. | ZIP, installer, upgrade and uninstall. |

## Shared regressions

- [ ] Close a window, quit and restart; verify workspace restoration and session
  confirmation without resurrecting intentionally closed workspaces.
- [ ] SSH authentication, reconnect and exit, using an authorized test endpoint.
- [ ] SFTP browsing, transfer, favorites and SSH current-directory linkage.
- [ ] Local, Remote and Dynamic forwarding; verify traffic and shutdown.
- [ ] AI streaming, stop, history, command execution and cancellation; verify
  completion belongs to the approved terminal and command.
- [ ] Terminal recording, playback and retention.
- [ ] Run RS alongside Flutter; verify independent databases, config/data paths
  and keyring service identities (`com.korvect.nauterm.rs` versus
  `com.korvect.nauterm`).

## Checks

Run the existing repository checks before recording implementation validation:

```sh
cargo fmt --all --check
cargo clippy -p nauterm_core -p nauterm-rs --all-targets --locked -- -D warnings
cargo test -p nauterm-rs --locked
cargo test -p nauterm_core --features terminal-ghostty --locked shell_integration
cargo test -p nauterm_core --features terminal-ghostty --locked pty::powershell
make build-rs
```

An opt-in Unix PTY test for an installed or portable PowerShell runs without
user profiles and keeps cache, config, data and runtime state in a temporary
directory:

```sh
python3 scripts/tests/check_powershell_protocol.py --powershell /path/to/pwsh
```

The Linux development host initially had the `libxkbcommon-x11.so.0` runtime
library but lacked the development linker name. Installing the workflow's
`libxkbcommon-x11-dev` package resolved the link failure; the user confirmed
ordinary `make run-rs` now launches. A temporary local linker symlink used
earlier is not a packaging solution or a dependency change.

The Linux checks do not establish Windows runtime behavior. The local Windows
type-check harness compiles the actual editor/Open With/chooser/cache sources
with Windows API bindings, but substitutes the GPUI window owner and file-model
boundary. It does not link the full Windows GPUI client or show native dialogs.
Use the workflow's real Windows build and the platform matrix above for those
checks. The dependency notice verifier currently covers 788 runtime packages
across the six supported targets.

## Optional window diagnostics

The normal build does not enable the diagnostic entry point. A dedicated feature
opens the actual Settings view and a simple reference view with an explicit
temporary data directory, resizes them through the window API and then exits:

```sh
diagnostic_dir="$(mktemp -d)"
NAUTERM_RS_DATA_DIR="$diagnostic_dir/data" \
NAUTERM_RESIZE_BENCH=1 NAUTERM_WINDOW_TRACE=1 \
cargo run -p nauterm-rs --features window-diagnostics --locked
```

`NAUTERM_RESIZE_BENCH_MODE=manual` leaves Settings open for pointer interaction
instead of running the sequence. `NAUTERM_RESIZE_BENCH_DECOR=client` selects the
client-border comparison only in this diagnostic build. Window traces contain
surface identifiers, sizes, focus/presentation state and timings, not terminal
contents. The programmatic sequence measures sizing/rendering costs; it does not
prove that compositor-driven interactive resizing is smooth.

## Read-only Open With diagnostics

```sh
NAUTERM_OPEN_WITH_DIAGNOSTICS=1 \
cargo run -p nauterm-rs --features platform-diagnostics --locked
```

This prints JSON for nine filename extensions (including DEB, Torrent, ZIP and
PDF) using the actual native catalog and icon-cache code. It reports metadata
and icon-loading times, GPUI decoding errors and nontransparent pixel counts. It exits before opening the Nauterm store or a display,
does not launch applications and does not change file associations. A runtime
check also confirmed that an explicit nonexistent data directory remained
absent. Cached bytes and successful image decoding are reported separately from UI
verification. Neither proves that the visible menu refreshed correctly.

The Linux loader parses icon-theme sizes once rather than rescanning the theme
index during every directory-sort comparison. On the development host, the
same fresh-process TXT catalog plus icons fell from 6540 ms to 46 ms, and PNG
from 5543 ms to 37 ms, with identical association/icon-cache results. Settings
and SFTP now publish metadata before loading icons, then refresh the existing
control/menu with icons under the original generation/selection guards. Actual
first-open behavior still needs user confirmation after restarting this build.
