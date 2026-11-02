# Nauterm RS third-party notices

`nauterm-rs-rust.json` contains license texts and package names/versions for the
Rust dependency graph. The notice inventory covers every supported platform;
some listed packages are used only by another platform build.

`nauterm-mosh-rust.json` contains the separate notices for the pinned Mosh
protocol engine deployed beside the executable or in `Contents/Frameworks`.

`ghostty-LICENSE` covers the statically linked libghostty-vt implementation.
`lucide-LICENSE` preserves the icon artwork notice from gpui-kit-assets 0.7.3.
On macOS, `sparkle-LICENSE` covers the embedded Sparkle 2.9.4 framework and helpers.

Host OS/distribution marks reuse the SVG artwork in the Nauterm repository's
`assets/icons/os/` directory. The executable embeds those same resources.

Source copies for MPL-2.0 dependencies compiled into this executable, including
their original license files, are included in `sources/`. `sources.json` lists
those packages and their upstream locations. These sources remain available
under their included MPL-2.0 license. Other components retain their own licenses.

In the macOS application bundle, this directory is located at
`Nauterm RS.app/Contents/Resources/licenses` (Finder: Show Package Contents).

The notice inventory can be regenerated from the repository root with
`make generate-rs-licenses` using the locked Rust dependencies.
