# Embedded font fallback on Windows

This is gpui-pre-windows 0.3.12, from Zed revision
`d2a4fb6ded7c470be25d1541281da3882495b65b` (Apache-2.0).

The DirectWrite backend now looks up fallback families in both its private
font collection and the system collection. Each mapping passes the collection
that owns the face to DirectWrite. Layout also passes the requested font
weight so a weight picked for the primary face cannot alter the fallback
font's weight. These changes let Nauterm load its bundled CJK fonts without
installing them into Windows.

Remove the Cargo override when a published GPUI dependency includes these fixes.
