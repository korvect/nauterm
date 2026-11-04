//! Product resources compose with GPUI Kit's existing icon catalog.
use gpui_kit::{AssetSource, SharedString, assets::IconName};
use std::borrow::Cow;
include!(concat!(env!("OUT_DIR"), "/host_icons.rs"));

pub(crate) const GROUP_ICON: IconName = IconName::FolderTree;

/// Product glyphs without a matching Lucide treatment.
#[derive(Clone, Copy)]
pub enum Glyph {
    Stop,
}
impl Glyph {
    pub fn path(self) -> &'static str {
        match self {
            Self::Stop => "nauterm/stop.svg",
        }
    }
    pub fn icon(self) -> gpui_kit::component::Icon {
        gpui_kit::component::Icon::default().path(self.path())
    }
}
const PRODUCT_ICONS: &[(&str, &[u8])] = &[
    (
        "nauterm/stop.svg",
        include_bytes!("../../assets/icons/ui/stop.svg"),
    ),
    (
        "nauterm/window-restore.svg",
        include_bytes!("../../assets/icons/ui/window-restore.svg"),
    ),
];
pub fn icon(name: IconName) -> gpui_kit::component::Icon {
    gpui_kit::component::Icon::new(name)
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = PRODUCT_ICONS
            .iter()
            .chain(HOST_ICONS.iter())
            .find(|(name, _)| *name == path)
        {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::AllAssets.list(path)?;
        paths.extend(
            PRODUCT_ICONS
                .iter()
                .chain(HOST_ICONS.iter())
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::from(*name)),
        );
        Ok(paths)
    }
}
