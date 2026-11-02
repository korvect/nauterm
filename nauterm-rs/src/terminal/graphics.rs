//! Prepare image textures outside paint; cached generations survive cursor
//! blinks but are retired when they leave the authoritative snapshot.
use super::*;
use crate::terminal_graphics::{Geometry, Image, Key, Layer, Rect};
use std::collections::{HashMap, HashSet};

enum Entry {
    Loading { _task: Task<()> },
    Ready(Arc<RenderImage>),
    Unavailable,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: HashMap<Key, Entry>,
    prepared: Option<PreparedCache>,
}
struct PreparedCache {
    snapshot: crate::terminal_graphics::Snapshot,
    cell: Size<Pixels>,
    images: Arc<[Prepared]>,
}
impl Cache {
    pub fn release(&mut self, cx: &mut App) {
        self.prepared = None;
        let images = self
            .entries
            .drain()
            .filter_map(|(_, entry)| match entry {
                Entry::Ready(image) => Some(image),
                _ => None,
            })
            .collect();
        retire(images, cx);
    }
    pub fn prepare(&mut self, frame: &Frame, cell: Size<Pixels>) -> Arc<[Prepared]> {
        if let Some(prepared) = &self.prepared
            && prepared.cell == cell
            && prepared.snapshot == frame.graphics
        {
            return prepared.images.clone();
        }
        let images: HashMap<_, _> = frame
            .graphics
            .images
            .iter()
            .map(|image| (image.key.id, image))
            .collect();
        let images: Arc<[Prepared]> = frame
            .graphics
            .placements
            .iter()
            .filter_map(|placement| {
                let raw = images.get(&placement.image_id)?;
                let Entry::Ready(image) = self.entries.get(&raw.key)? else {
                    return None;
                };
                Some(Prepared {
                    layer: Layer::for_z(placement.z),
                    geometry: placement
                        .geometry(raw.key, (cell.width.into(), cell.height.into()))?,
                    image: image.clone(),
                })
            })
            .collect();
        self.prepared = Some(PreparedCache {
            snapshot: frame.graphics.clone(),
            cell,
            images: images.clone(),
        });
        images
    }
}
fn retire(images: Vec<Arc<RenderImage>>, cx: &mut App) {
    if images.is_empty() {
        return;
    }
    // Wait until the current window update returns. App::drop_image can then
    // remove atlas entries from every window, including the current one.
    cx.defer(move |cx| {
        for image in images {
            cx.drop_image(image, None);
        }
    });
}
fn render(image: &Image) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(image.key.width, image.key.height, image.bgra())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}
impl Terminal {
    pub(super) fn sync_graphics(&mut self, cx: &mut Context<Self>) {
        let visible: HashSet<_> = self
            .frame
            .graphics
            .images
            .iter()
            .map(|image| image.key)
            .collect();
        let removed: Vec<_> = self
            .graphics
            .entries
            .keys()
            .filter(|key| !visible.contains(key))
            .copied()
            .collect();
        if !removed.is_empty() {
            self.graphics.prepared = None;
        }
        let retired = removed
            .into_iter()
            .filter_map(|key| match self.graphics.entries.remove(&key) {
                Some(Entry::Ready(image)) => Some(image),
                _ => None,
            })
            .collect();
        retire(retired, cx);
        for raw in &self.frame.graphics.images {
            let key = raw.key;
            if self.graphics.entries.contains_key(&key) {
                continue;
            }
            let raw = raw.clone();
            let task = cx.spawn(async move |this, cx| {
                let image = cx
                    .background_executor()
                    .spawn(async move { render(&raw) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.graphics.entries.contains_key(&key)
                        && this
                            .frame
                            .graphics
                            .images
                            .iter()
                            .any(|image| image.key == key)
                    {
                        this.graphics.prepared = None;
                        this.graphics
                            .entries
                            .insert(key, image.map_or(Entry::Unavailable, Entry::Ready));
                        cx.notify();
                    }
                });
            });
            self.graphics
                .entries
                .insert(key, Entry::Loading { _task: task });
        }
    }
}
pub(super) struct Prepared {
    layer: Layer,
    geometry: Geometry,
    image: Arc<RenderImage>,
}
pub(super) fn paint(
    images: &[Prepared],
    layer: Layer,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    for image in images.iter().filter(|image| image.layer == layer) {
        let rect = |rect: Rect| {
            Bounds::new(
                bounds.origin + point(px(rect.x), px(rect.y)),
                size(px(rect.width), px(rect.height)),
            )
        };
        let destination = rect(image.geometry.destination).intersect(&bounds);
        if destination.is_empty() {
            continue;
        }
        window.paint_layer(destination, |window| {
            let _ = window.paint_image(
                destination,
                rect(image.geometry.image),
                Corners::all(px(0.)),
                image.image.clone(),
                0,
                false,
            );
        });
    }
}
