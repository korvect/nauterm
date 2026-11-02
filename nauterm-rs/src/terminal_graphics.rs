//! Snapshot-owned Kitty image data and reference-compatible placement geometry.
//! No session, GPU, application, clipboard or filesystem access in this model.
use nauterm_core::terminal::{TerminalGraphicImage, TerminalGraphicPlacement};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub id: u32,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub struct Image {
    pub key: Key,
    pub rgba: Arc<[u8]>,
}
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        // Native generations identify immutable pixel data. Cursor blinks and
        // repeated snapshots must not compare every byte of a large image.
        self.key == other.key
    }
}
impl Eq for Image {}
impl Image {
    fn from_native(image: TerminalGraphicImage) -> Option<Self> {
        let len = (image.width as usize)
            .checked_mul(image.height as usize)?
            .checked_mul(4)?;
        if image.width == 0 || image.height == 0 || image.rgba.len() != len {
            return None;
        }
        Some(Self {
            key: Key {
                id: image.id,
                generation: image.generation,
                width: image.width,
                height: image.height,
            },
            rgba: image.rgba.into(),
        })
    }
    /// GPUI RenderImage takes BGRA, with the original straight alpha.
    pub fn bgra(&self) -> Vec<u8> {
        let mut pixels = self.rgba.to_vec();
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        pixels
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub images: Vec<Image>,
    pub placements: Vec<Placement>,
}
impl Snapshot {
    pub fn from_native(
        images: Vec<TerminalGraphicImage>,
        placements: Vec<TerminalGraphicPlacement>,
    ) -> Self {
        Self {
            images: images.into_iter().filter_map(Image::from_native).collect(),
            placements: placements.into_iter().map(Placement::from).collect(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    BelowBackground,
    BelowText,
    AboveText,
}
impl Layer {
    pub fn for_z(z: i32) -> Self {
        if z < -0x4000_0000 {
            Self::BelowBackground
        } else if z < 0 {
            Self::BelowText
        } else {
            Self::AboveText
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub destination: Rect,
    /// Bounds of the full texture, scaled/translated so its source crop fills
    /// destination. GPUI clips and computes UVs without allocating crop images.
    pub image: Rect,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub image_id: u32,
    pub placement_id: u32,
    pub z: i32,
    pub column: i32,
    pub row: i32,
    pub columns: u32,
    pub rows: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}
impl From<TerminalGraphicPlacement> for Placement {
    fn from(p: TerminalGraphicPlacement) -> Self {
        Self {
            image_id: p.image_id,
            placement_id: p.placement_id,
            z: p.z_index,
            column: p.viewport_column,
            row: p.viewport_row,
            columns: p.columns,
            rows: p.rows,
            source_x: p.source_x,
            source_y: p.source_y,
            source_width: p.source_width,
            source_height: p.source_height,
        }
    }
}
impl Placement {
    pub fn geometry(self, image: Key, cell: (f32, f32)) -> Option<Geometry> {
        if self.columns == 0
            || self.rows == 0
            || image.width == 0
            || image.height == 0
            || !cell.0.is_finite()
            || !cell.1.is_finite()
            || cell.0 <= 0.
            || cell.1 <= 0.
        {
            return None;
        }
        // Flutter intersects the requested source with the image first, then
        // stretches that intersection into the entire destination cell grid.
        let source_width = if self.source_width == 0 {
            image.width
        } else {
            self.source_width
        };
        let source_height = if self.source_height == 0 {
            image.height
        } else {
            self.source_height
        };
        let right = self.source_x.saturating_add(source_width).min(image.width);
        let bottom = self
            .source_y
            .saturating_add(source_height)
            .min(image.height);
        let width = right.checked_sub(self.source_x)?;
        let height = bottom.checked_sub(self.source_y)?;
        if width == 0 || height == 0 {
            return None;
        }
        let destination = Rect {
            x: self.column as f32 * cell.0,
            y: self.row as f32 * cell.1,
            width: self.columns as f32 * cell.0,
            height: self.rows as f32 * cell.1,
        };
        let scale = (
            destination.width / width as f32,
            destination.height / height as f32,
        );
        let image = Rect {
            x: destination.x - self.source_x as f32 * scale.0,
            y: destination.y - self.source_y as f32 * scale.1,
            width: image.width as f32 * scale.0,
            height: image.height as f32 * scale.1,
        };
        [
            destination.x,
            destination.y,
            destination.width,
            destination.height,
            image.x,
            image.y,
            image.width,
            image.height,
        ]
        .into_iter()
        .all(f32::is_finite)
        .then_some(Geometry { destination, image })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> TerminalGraphicImage {
        TerminalGraphicImage {
            id: 7,
            generation: 11,
            width: 1,
            height: 1,
            rgba: vec![240, 10, 30, 128],
        }
    }
    fn placement() -> Placement {
        Placement {
            image_id: 7,
            placement_id: 1,
            z: 0,
            column: 2,
            row: 3,
            columns: 5,
            rows: 2,
            source_x: 20,
            source_y: 10,
            source_width: 40,
            source_height: 20,
        }
    }
    fn key() -> Key {
        Key {
            id: 7,
            generation: 11,
            width: 100,
            height: 50,
        }
    }
    #[test]
    fn graphics_validate_native_rgba_and_convert_channels_without_changing_alpha() {
        let decoded = Image::from_native(image()).unwrap();
        assert_eq!(decoded.bgra(), [30, 10, 240, 128]);
        for (width, height, pixels) in [
            (0, 1, vec![]),
            (2, 2, vec![0; 15]),
            (u32::MAX, u32::MAX, vec![]),
        ] {
            assert!(
                Image::from_native(TerminalGraphicImage {
                    width,
                    height,
                    rgba: pixels,
                    ..image()
                })
                .is_none()
            );
        }
    }
    #[test]
    fn graphics_revision_identity_reuses_a_snapshot_but_replaced_ids_invalidate_it() {
        let first = Snapshot::from_native(vec![image()], vec![]);
        assert_eq!(first, Snapshot::from_native(vec![image()], vec![]));
        assert_ne!(
            first,
            Snapshot::from_native(
                vec![TerminalGraphicImage {
                    generation: 12,
                    ..image()
                }],
                vec![]
            )
        );
    }
    #[test]
    fn graphics_crop_scales_the_full_texture_to_the_requested_cell_rectangle() {
        let g = placement().geometry(key(), (8., 16.)).unwrap();
        assert_eq!(
            g.destination,
            Rect {
                x: 16.,
                y: 48.,
                width: 40.,
                height: 32.
            }
        );
        assert_eq!(
            g.image,
            Rect {
                x: -4.,
                y: 32.,
                width: 100.,
                height: 80.
            }
        );
        // The destination's edges select exactly x=20..60, y=10..30 in texture pixels.
        assert_eq!((g.destination.x - g.image.x) / g.image.width * 100., 20.);
        assert_eq!((g.destination.y - g.image.y) / g.image.height * 50., 10.);
    }
    #[test]
    fn graphics_clip_default_sources_and_keep_negative_viewport_origins() {
        let p = Placement {
            column: -1,
            row: -2,
            source_x: 80,
            source_y: 40,
            source_width: 0,
            source_height: 0,
            ..placement()
        };
        let g = p.geometry(key(), (8., 16.)).unwrap();
        assert_eq!(
            g.destination,
            Rect {
                x: -8.,
                y: -32.,
                width: 40.,
                height: 32.
            }
        );
        assert_eq!(
            g.image,
            Rect {
                x: -168.,
                y: -160.,
                width: 200.,
                height: 160.
            }
        );
        for p in [
            Placement { source_x: 100, ..p },
            Placement { columns: 0, ..p },
            Placement {
                source_y: u32::MAX,
                ..p
            },
        ] {
            assert!(p.geometry(key(), (8., 16.)).is_none());
        }
        assert!(p.geometry(key(), (f32::INFINITY, 16.)).is_none());
    }
    #[test]
    fn graphics_layer_boundaries_match_the_reference_painter() {
        for (z, expected) in [
            (i32::MIN, Layer::BelowBackground),
            (-0x4000_0001, Layer::BelowBackground),
            (-0x4000_0000, Layer::BelowText),
            (-1, Layer::BelowText),
            (0, Layer::AboveText),
            (i32::MAX, Layer::AboveText),
        ] {
            assert_eq!(Layer::for_z(z), expected);
        }
    }
    #[test]
    fn graphics_native_png_placeholder_and_delete_use_the_same_snapshot_mapping() {
        use nauterm_core::{
            session::SessionManager,
            terminal::{TerminalEmulatorBackend, TerminalOptions},
        };
        // A private passive VT has no PTY, socket, database, keyring or file.
        let mut terminal = SessionManager::default();
        let id = terminal.create_replay(
            8,
            2,
            TerminalOptions {
                emulator_backend: TerminalEmulatorBackend::Ghostty,
                ..Default::default()
            },
        );
        assert_ne!(id, 0);
        assert!(terminal.resize(id, 8, 2, 8, 16));
        assert!(
            terminal.write_replay(
                id,
                concat!(
                    "\x1b_Ga=t,f=100,i=1,q=2;iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA",
                    "DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==\x1b\\",
                    "\x1b_Ga=p,i=1,p=1,U=1,c=1,r=1,q=2;\x1b\\",
                    "\x1b[38;2;0;0;1;58;2;0;0;1m\u{10eeee}\u{0305}\u{0305}\x1b[0m"
                )
                .as_bytes()
                .to_vec(),
            )
        );
        let native = terminal.snapshot(id).unwrap();
        assert_eq!(native.cells[0].text_len, 0);
        let graphics = Snapshot::from_native(native.graphic_images, native.graphic_placements);
        assert_eq!(graphics.images.len(), 1);
        assert_eq!(graphics.images[0].rgba.len(), 4);
        assert_eq!(graphics.placements.len(), 1);
        let geometry = graphics.placements[0]
            .geometry(graphics.images[0].key, (8., 16.))
            .unwrap();
        assert_eq!(
            geometry.destination,
            Rect {
                x: 0.,
                y: 0.,
                width: 8.,
                height: 16.
            }
        );
        assert!(terminal.write_replay(id, b"\x1b_Ga=d,d=A,q=2;\x1b\\".to_vec()));
        // Visible-placement deletion does not affect virtual prototypes.
        assert_eq!(terminal.snapshot(id).unwrap().graphic_images.len(), 1);
        assert!(terminal.write_replay(id, b"\x1b_Ga=d,d=I,i=1,q=2;\x1b\\".to_vec()));
        let deleted = terminal.snapshot(id).unwrap();
        let deleted = Snapshot::from_native(deleted.graphic_images, deleted.graphic_placements);
        assert!(deleted.images.is_empty());
        assert!(deleted.placements.is_empty());
        assert!(terminal.close(id));
    }
}
