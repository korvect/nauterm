//! Product resources compose with GPUI Kit's existing icon catalog.
use gpui_kit::{AssetSource, SharedString, assets::IconName};
use std::borrow::Cow;
include!(concat!(env!("OUT_DIR"), "/host_icons.rs"));

pub(crate) const GROUP_ICON: IconName = IconName::FolderTree;

/// Product glyphs without a matching Lucide treatment.
#[derive(Clone, Copy)]
pub enum Glyph {
    Stop,
    PlusBold,
    CheckBold,
    LocateFixed,
}
impl Glyph {
    pub fn path(self) -> &'static str {
        match self {
            Self::Stop => "nauterm/stop.svg",
            Self::PlusBold => "nauterm/plus-bold.svg",
            Self::CheckBold => "nauterm/check-bold.svg",
            Self::LocateFixed => "nauterm/locate-fixed.svg",
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
        "nauterm/plus-bold.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" d="M12 5v14M5 12h14"/></svg>"#,
    ),
    (
        "nauterm/check-bold.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="m5 12 4 4L19 6"/></svg>"#,
    ),
    (
        "nauterm/locate-fixed.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="2" x2="5" y1="12" y2="12"/><line x1="19" x2="22" y1="12" y2="12"/><line x1="12" x2="12" y1="2" y2="5"/><line x1="12" x2="12" y1="19" y2="22"/><circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="3"/></svg>"#,
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
