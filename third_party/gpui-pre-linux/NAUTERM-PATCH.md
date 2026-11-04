# Nauterm Wayland native decorations

This is gpui-pre-linux 0.3.10, from zed revision
08cbb88a152c3751a43e853425a70e9f631a5918 (Apache-2.0).
The source is temporarily pinned here so the Wayland native-decoration change
can be built and reviewed with Nauterm. Move the foundation change into the
GPUI dependency fork before replacing this Cargo patch with a published revision.

Only windows requesting a standard opaque titlebar on a compositor without
xdg-decoration use libdecor (`WindowParams` does not carry the later decoration
request). The library and its GTK plugin provide the titlebar,
shadow, resizing and titlebar interactions; Nauterm does not draw these controls.
The existing client-decorated main window keeps its original implementation.

The patch keeps native frame configurations pending until the next GPUI render
tick, so intermediate pointer positions do not repeatedly resize the GPU
surface. The last configuration is retained even if no later motion event
arrives. Native dialogs keep the existing parent/modal rules; normal windows
(such as Settings) remain independent toplevel windows.

Validation: Linux backend/application Clippy, the Nauterm RS test suite and
`make build-rs`. The native-titlebar example's appearance, move, maximize and
close operations were confirmed manually on GNOME Wayland. Full Settings
resize/animation behavior is tracked in the application validation document.

Geometry and scale changes invalidate the previously presented buffer. The next
frame therefore requests presentation even if an interactive compositor grab
temporarily removes keyboard focus. This preserves the real focus state while
avoiding GPUI's inactive-window frame cap for an obsolete buffer size. The pure
presentation model can be checked without a display or GPUI's test runtime:

```sh
rustc --edition 2024 --test third_party/gpui-pre-linux/src/linux/wayland/presentation.rs -o /tmp/nauterm-presentation-tests
/tmp/nauterm-presentation-tests
```
