use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "default", alias = "defaultIcon")]
    Initial,
    #[serde(rename = "osIcon")]
    OsIcon,
    #[default]
    #[serde(rename = "osBadge", other)]
    OsBadge,
}
impl Global for Mode {}
impl Mode {
    pub const ALL: [Self; 3] = [Self::Initial, Self::OsBadge, Self::OsIcon];
    pub fn label(self) -> &'static str {
        match self {
            Self::Initial => "Default",
            Self::OsBadge => "With OS Badge",
            Self::OsIcon => "OS Icon",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Platform {
    pub os: Option<String>,
    pub distro: Option<String>,
}
impl Platform {
    pub fn icon(&self) -> Option<(&'static str, u32)> {
        let text = format!(
            "{} {}",
            self.os.as_deref().unwrap_or_default(),
            self.distro.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        OS_ICONS
            .iter()
            .find(|(patterns, _, _)| patterns.iter().any(|pattern| text.contains(pattern)))
            .map(|(_, slug, color)| (*slug, *color))
    }
}
// Preserve the reference's matching priority as well as its asset IDs/colors.
const OS_ICONS: &[(&[&str], &str, u32)] = &[
    (&["ubuntu"], "ubuntu", 0xe95420),
    (&["debian"], "debian", 0xa81d33),
    (&["centos"], "centos", 0x262577),
    (&["rhel", "redhat"], "redhat", 0xee0000),
    (&["fedora"], "fedora", 0x51a2da),
    (&["alma"], "almalinux", 0x000000),
    (&["rocky"], "rockylinux", 0x10b981),
    (&["alpine"], "alpine", 0x0d597f),
    (&["arch"], "arch", 0x1793d1),
    (&["manjaro"], "manjaro", 0x35bfa4),
    (&["gentoo"], "gentoo", 0x54487a),
    (&["opensuse", "suse"], "opensuse", 0x73ba25),
    (&["mint"], "linuxmint", 0x86be43),
    (&["kali"], "kalilinux", 0x557c94),
    (&["nixos", "nix"], "nixos", 0x5277c3),
    (&["pop!_os", "popos", "pop_os"], "popos", 0x48b9c7),
    (&["elementary"], "elementary", 0x64baff),
    (&["zorin"], "zorin", 0x15a6f0),
    (&["mxlinux", "mx linux"], "mxlinux", 0x000000),
    (&["void"], "voidlinux", 0x478061),
    (&["slackware"], "slackware", 0x000000),
    (&["garuda"], "garudalinux", 0x8839ef),
    (&["endeavour"], "endeavouros", 0x7f7fff),
    (&["artix"], "artixlinux", 0x10a0cc),
    (&["cachyos", "cachy"], "cachyos", 0x00aa88),
    (&["nobara"], "nobaralinux", 0x000000),
    (&["asahi"], "asahilinux", 0xa61200),
    (&["graphene"], "grapheneos", 0x0053a3),
    (&["qubes"], "qubesos", 0x3874d8),
    (&["reactos", "react"], "reactos", 0x0088cc),
    (&["freebsd"], "freebsd", 0xab2b28),
    (&["openbsd"], "openbsd", 0xf2ca30),
    (&["netbsd"], "netbsd", 0xff6600),
    (&["bsd"], "bsd", 0xab2b28),
    (&["proxmox"], "proxmox", 0xe57000),
    (&["vmware", "esxi"], "vmware", 0x607078),
    (&["truenas", "freenas"], "truenas", 0x0095d5),
    (&["unraid"], "unraid", 0xf15a2c),
    (&["synology"], "synology", 0xb5b5b6),
    (&["qnap"], "qnap", 0x0c2e82),
    (&["openwrt"], "openwrt", 0x00b5e2),
    (&["opnsense"], "opnsense", 0xe44a20),
    (&["pfsense"], "pfsense", 0x212121),
    (&["mikrotik", "routeros"], "mikrotik", 0x293239),
    (&["android"], "android", 0x3ddc84),
    (&["raspberry", "raspi"], "raspberrypi", 0xa22846),
    (&["linux", "gnu"], "linux", 0xfcc624),
    (&["macos", "darwin", "osx"], "apple", 0x000000),
    (&["windows", "win32", "winnt"], "windows", 0x0078d4),
    (&["unix"], "linux", 0xfcc624),
];

pub fn color(name: &str) -> u32 {
    if name.is_empty() {
        return 0x7b8f9a;
    }
    let hash = name.encode_utf16().fold(0i64, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i64::from(unit))
    });
    let colors = [
        0x4d9cbd, 0x9965ba, 0xbd6767, 0x60b86a, 0xbda050, 0x547abd, 0xbd558a, 0x54bdb2, 0x86bd55,
        0x7458bd, 0xbd8255, 0x54bd82,
    ];
    colors[(hash.unsigned_abs() % colors.len() as u64) as usize]
}

pub fn host(name: &str) -> HostAvatar {
    HostAvatar {
        name: name.to_owned(),
        platform: Platform::default(),
        dimension: 40.,
    }
}

pub fn group() -> Div {
    div()
        .size(px(40.))
        .flex_shrink_0()
        .rounded(px(10.))
        .bg(rgb(0x075e92))
        .text_color(rgb(0xffffff))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(crate::assets::GROUP_ICON).with_size(px(20.)))
}
#[derive(IntoElement)]
pub struct HostAvatar {
    name: String,
    platform: Platform,
    dimension: f32,
}
impl HostAvatar {
    pub fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
    pub fn dimension(mut self, dimension: f32) -> Self {
        self.dimension = dimension;
        self
    }
}
impl RenderOnce for HostAvatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial = self
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or("?".into());
        let dimension = self.dimension;
        let mode = cx.try_global::<Mode>().copied().unwrap_or_default();
        let icon = self.platform.icon();
        let replace = mode == Mode::OsIcon && icon.is_some();
        let mut view = div()
            .size(px(dimension))
            .flex_shrink_0()
            .relative()
            .rounded(px(dimension / 4.))
            .bg(rgb(if replace {
                icon.unwrap().1
            } else {
                color(&self.name)
            }))
            .text_color(rgb(0xffffff))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(if dimension <= 24. {
                12.
            } else if dimension <= 36. {
                15.
            } else {
                20.
            }))
            .font_weight(FontWeight::SEMIBOLD);
        if replace {
            view = view.child(
                svg()
                    .path(SharedString::from(format!(
                        "nauterm/os/system-{}.svg",
                        icon.unwrap().0
                    )))
                    // GPUI's SVG paint requires its own color; the parent's
                    // text color does not satisfy that element-level guard.
                    .text_color(rgb(0xffffff))
                    .size(px(dimension * 0.6)),
            );
        } else {
            view = view.child(initial);
        }
        if let Some((slug, brand)) = icon.filter(|_| mode == Mode::OsBadge) {
            let compact = dimension <= 24.;
            let badge = if compact { 10. } else { dimension * 0.4 };
            let inset = if compact { 1.5 } else { dimension * 0.0625 };
            view = view.child(
                div()
                    .absolute()
                    .right(px(if compact { -2. } else { -3. }))
                    .bottom(px(if compact { -2. } else { -3. }))
                    .size(px(badge))
                    .rounded(px(if compact { 3. } else { 4. }))
                    .bg(rgb(brand))
                    .when(compact, |badge| {
                        badge.border_1().border_color(
                            crate::palette::Palette::new(cx.theme().is_dark()).surface,
                        )
                    })
                    .p(px(inset))
                    .child(
                        svg()
                            .path(SharedString::from(format!("nauterm/os/system-{slug}.svg")))
                            .text_color(rgb(0xffffff))
                            .size_full(),
                    ),
            );
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn initial_color_matches_the_live_reference() {
        assert_eq!(super::color("127.0.0.1"), 0x547abd);
        assert_eq!(super::color(""), 0x7b8f9a);
    }
    #[test]
    fn platform_precedence_and_config_names_follow_the_reference() -> anyhow::Result<()> {
        assert_eq!(
            Platform {
                os: Some("Linux".into()),
                distro: Some("Ubuntu 24.04".into())
            }
            .icon(),
            Some(("ubuntu", 0xe95420))
        );
        assert_eq!(
            Platform {
                os: Some("Darwin".into()),
                ..Default::default()
            }
            .icon(),
            Some(("apple", 0))
        );
        assert_eq!(
            Platform {
                os: Some("unrecognized appliance".into()),
                ..Default::default()
            }
            .icon(),
            None
        );
        assert_eq!(serde_json::from_str::<Mode>("\"default\"")?, Mode::Initial);
        assert_eq!(
            serde_json::from_str::<Mode>("\"defaultIcon\"")?,
            Mode::Initial
        );
        assert_eq!(serde_json::to_string(&Mode::Initial)?, "\"default\"");
        assert_eq!(Mode::default(), Mode::OsBadge);
        let host = crate::model::Host {
            platform: Platform {
                os: Some("Linux".into()),
                distro: Some("Ubuntu".into()),
            },
            ..Default::default()
        };
        assert!(serde_json::to_value(host)?.get("platform").is_none());
        Ok(())
    }
    #[test]
    fn embedded_platform_assets_and_component_icons_are_available_together() -> anyhow::Result<()> {
        use gpui_kit::AssetSource;
        for (_, slug, _) in OS_ICONS {
            let path = format!("nauterm/os/system-{slug}.svg");
            let data = crate::assets::Assets.load(&path)?.unwrap();
            assert!(std::str::from_utf8(&data)?.contains("<svg"), "{path}");
        }
        assert_eq!(
            crate::assets::Assets.list("nauterm/os/")?.len(),
            crate::assets::HOST_ICONS.len()
        );
        let kit = gpui_kit::assets::AllAssets.list("icons/")?;
        assert!(!kit.is_empty());
        assert!(crate::assets::Assets.load(&kit[0])?.is_some());
        Ok(())
    }
}
