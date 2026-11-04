# Nauterm RS

Nauterm's Rust and GPUI desktop client. The Flutter client lives at the
repository root; both clients use the same native core.

## Development

Use the Rust toolchain pinned in `../rust-toolchain.toml` and the native build
prerequisites listed in the [main README](../README.md#building-from-source).
On Linux, GPUI builds both X11 and Wayland support, so the X11 development
libraries are required even in a Wayland desktop session. Ubuntu uses the same
packages as `.github/workflows/quality-gates.yml`:

```sh
sudo apt-get install -y clang cmake libasound2-dev libclang-dev \
  libdbus-1-dev libdecor-0-0 libdecor-0-plugin-1-gtk \
  libfontconfig1-dev libfreetype6-dev libgtk-3-dev \
  libpipewire-0.3-dev libssl-dev libudev-dev libvulkan-dev \
  libwayland-dev libxcb1-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libxkbcommon-x11-dev pkg-config
```

`unable to find library -lxkbcommon-x11` means the development package is
missing; the installed `libxkbcommon-x11.so.0` runtime alone does not provide
the linker name. Install `libxkbcommon-x11-dev` before `make run-rs`.

On GNOME Wayland, standard window titlebars use libdecor's GTK plugin.
The main window retains its product-owned controls. A local Cargo patch in
`third_party/gpui-pre-linux` contains the backend integration pending migration
to the dependency fork. `cargo run -p nauterm-rs --example native_titlebar`
opens a data-free window for manual decoration/resize testing.

Run these commands from the repository root:

```sh
make build-rs
make run-rs
make check-rs
cargo fmt --all --check
cargo clippy -p nauterm_core -p nauterm-rs --all-targets --locked -- -D warnings
cargo test -p nauterm-rs --locked
```

`make run-rs` creates an application bundle on macOS so AppKit can resolve the
application identity, compiled icon and bundled frameworks. Mosh libraries are
staged by the build scripts.

Automated checks use pure models, mocks, in-memory databases or isolated
temporary files. They must not open persistent application databases, access
real credentials, connect to live services or enumerate hardware. UI and
interaction verification is manual.

## Architecture

- `native/nauterm_core/` provides terminal emulation, transports, encrypted
  storage, synchronization and recordings. Flutter calls its dynamic C ABI;
  RS calls its Rust APIs with the `terminal-ghostty` feature. RS does not load
  the core dynamic library or call its exported C functions.
- The Cargo workspace shares one lockfile, release version and `target/`
  directory. Repeated dependency versions are declared in the root
  `Cargo.toml`; member crates retain their required features.
- GPUI Kit is pinned in this client's `Cargo.toml` to the `v0.8.1` branch of
  [korvect/gpui-kit](https://github.com/korvect/gpui-kit/tree/v0.8.1).
  Foundation changes belong in that fork.
- [Product components](src/components/README.md) provide shared behavior with
  separate workspace, settings and terminal skins. Settings, including AI and
  Sync & Backup, retain their RS-specific page layouts.

Startup renders configuration and workspace metadata before opening encrypted
resources on the database worker. Only regions that need those resources wait
for them; navigation and toolbars remain available.

## Application data

All local RS data is independent from Nauterm, including its database, config,
known hosts, themes, history, recordings, backups and workspace recovery state.

| Platform | RS directories |
| --- | --- |
| macOS | `~/Library/Application Support/Nauterm RS` |
| Windows | `%APPDATA%/Nauterm RS` |
| Linux | `$XDG_CONFIG_HOME/nauterm-rs` and `$XDG_DATA_HOME/nauterm-rs`, with the usual home-directory defaults |

`NAUTERM_RS_DATA_DIR` overrides RS directories for development. RS does not
copy or fall back to the Flutter client's data. Each client has one
`nauterm.sqlite`, including its credential records. RS uses the OS keyring
service `com.korvect.nauterm.rs`; Nauterm uses `com.korvect.nauterm`.

Both clients use the same encrypted sync format and default remote filename,
`nauterm-sync.enc`. Configure the same remote target and Master Key to synchronize
resources between their independent databases. Provider credentials, local key
caches, preferences and workspace state stay local. The current schema 5 reader
accepts schema 4 snapshots; new snapshots always use the current schema version.

## Packaging

Run the platform-appropriate targets from the repository root:

```sh
make generate-rs-licenses

# macOS: local debug bundle, release bundle/archive, signed Sparkle feed
make bundle-rs-macos
make package-rs-macos
make appcast-rs-macos

# Linux
make package-rs-linux-x86_64
make package-rs-linux-arm64

# Windows
make package-rs-windows-x86_64
make package-rs-windows-arm64
```

Release packages use `DIST_DIR` (default `dist`). The release profile enables
ThinLTO, one codegen unit and stripped symbols. Platform signing and publishing
are configured in the [release workflow](../.github/workflows/release-packages.yml).

macOS bundles compile the shared `assets/icons/Nauterm.icon` source using
`actool`. Sparkle uses the RS-specific `appcast-rs-arm64.xml` and
`appcast-rs-x86_64.xml` feeds. `NAUTERM_UPDATE_REPOSITORY` and
`SPARKLE_PUBLIC_ED_KEY` configure the client; `SPARKLE_PRIVATE_KEY` and
`GITHUB_REF_NAME` are required when generating a signed release feed.
`NAUTERM_SPARKLE_FRAMEWORK_DIR` can supply an existing Sparkle framework.
