//! SFTP breadcrumbs keep native path syntax and use this frame's available width.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Part {
    label: String,
    path: String,
}
fn parts(path: &str, windows: bool) -> Vec<Part> {
    let path = path.trim();
    if path.is_empty() {
        return vec![];
    }
    let separator = if windows { '\\' } else { '/' };
    let normalized = if windows {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    };
    // Windows canonicalization returns verbatim paths. Keep that prefix in
    // navigation targets, but don't display it as a fake server/share crumb.
    let verbatim = windows && normalized.starts_with("\\\\?\\");
    let normalized = if verbatim {
        if let Some(rest) = normalized.strip_prefix("\\\\?\\UNC\\") {
            format!("\\\\{rest}")
        } else {
            normalized[4..].to_owned()
        }
    } else {
        normalized
    };
    let mut names = normalized
        .split(separator)
        .filter(|name| !name.is_empty())
        .peekable();
    let mut current = if normalized.starts_with(separator) {
        separator.to_string()
    } else {
        String::new()
    };
    let mut result = vec![];
    if windows && normalized.starts_with("\\\\") {
        let server = names.next().unwrap_or_default();
        let share = names.next().unwrap_or_default();
        current = format!("\\\\{server}\\{share}");
        result.push(Part {
            label: format!("{server}\\{share}"),
            path: current.clone(),
        });
    }
    for name in names {
        if !current.is_empty() && !current.ends_with(separator) {
            current.push(separator);
        }
        current.push_str(name);
        if windows && result.is_empty() && name.len() == 2 && name.ends_with(':') {
            current.push(separator);
        }
        result.push(Part {
            label: name.into(),
            path: current.clone(),
        });
    }
    if result.is_empty() {
        result.push(Part {
            label: normalized.clone(),
            path: normalized,
        });
    }
    if verbatim {
        for part in &mut result {
            part.path = if let Some(unc) = part.path.strip_prefix("\\\\") {
                format!("\\\\?\\UNC\\{unc}")
            } else {
                format!("\\\\?\\{}", part.path)
            };
        }
    }
    result
}
fn visible_start(widths: &[f32], available: f32) -> usize {
    let total = |start: usize| {
        widths[start..].iter().sum::<f32>() + widths.len().saturating_sub(start + 1) as f32 * 15.
    };
    if widths.is_empty() || total(0) <= available {
        return 0;
    }
    (1..widths.len())
        .find(|start| 22. + 15. + total(*start) <= available)
        .unwrap_or(widths.len().saturating_sub(1))
}
impl FilePane {
    pub(super) fn edit_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_path = true;
        self.path.update(cx, |input, cx| {
            input.set_value(self.history.path.clone(), window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }
    pub(super) fn breadcrumbs(&self, cx: &mut Context<Self>) -> AnyElement {
        let parts = parts(&self.history.path, self.remote.is_none() && cfg!(windows));
        let owner = cx.weak_entity();
        let p = self.palette();
        canvas(
            move |bounds, window, cx| {
                let mut style = window.text_style();
                style.font_weight = FontWeights::MEDIUM;
                let widths: Vec<_> = parts
                    .iter()
                    .map(|part| {
                        let label: SharedString = part.label.clone().into();
                        let run = style.to_run(label.len());
                        f32::from(
                            window
                                .text_system()
                                .shape_line(label, px(12.), &[run], None)
                                .width(),
                        ) + 23.
                    })
                    .collect();
                let available = f32::from(bounds.size.width);
                let start = visible_start(&widths, available);
                let edit = owner.clone();
                let mut row = div()
                    .id("sftp-breadcrumbs")
                    .w_full()
                    .h(px(32.))
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        let _ = edit.update(cx, |pane, cx| pane.edit_path(window, cx));
                    });
                if start > 0 {
                    let target = parts[start - 1].path.clone();
                    let navigate = owner.clone();
                    row = row
                        .child(
                            Button::new("sftp-breadcrumb-overflow")
                                .ghost()
                                .toolbar()
                                .custom(crate::components::ButtonCustomVariant::for_surface(p))
                                .ink_ripple(false)
                                .w(px(22.))
                                .p_0()
                                .label("…")
                                .tooltip(
                                    parts[..start]
                                        .iter()
                                        .map(|part| part.label.as_str())
                                        .collect::<Vec<_>>()
                                        .join(" / "),
                                )
                                .on_click(move |_, window, cx| {
                                    let _ = navigate.update(cx, |pane, cx| {
                                        pane.navigate(target.clone(), Navigation::Visit, window, cx)
                                    });
                                }),
                        )
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(15.))
                                .text_color(p.text),
                        );
                }
                let mut room = (available - if start > 0 { 37. } else { 0. }).max(0.);
                for (index, part) in parts.iter().enumerate().skip(start) {
                    if index > start {
                        row = row.child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(15.))
                                .text_color(p.text),
                        );
                        room = (room - 15.).max(0.);
                    }
                    let width = widths[index].min(room);
                    room = (room - width).max(0.);
                    let navigate = owner.clone();
                    let target = part.path.clone();
                    row = row.child(
                        Button::new(("sftp-breadcrumb", index))
                            .ghost()
                            .toolbar()
                            .custom(crate::components::ButtonCustomVariant::for_surface(p))
                            .ink_ripple(false)
                            .h(px(28.))
                            .w(px(width))
                            .px(px(2.))
                            .content_start()
                            .content_gap(px(3.))
                            .text_size(px(12.))
                            .line_height(px(18.))
                            .icon(
                                Icon::new(IconName::Folder)
                                    .size(px(16.))
                                    .text_color(rgb(0x5ec4f2)),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(part.label.clone()),
                            )
                            .raw_tooltip(part.path.clone())
                            .on_click(move |_, window, cx| {
                                let _ = navigate.update(cx, |pane, cx| {
                                    pane.navigate(target.clone(), Navigation::Visit, window, cx)
                                });
                            }),
                    );
                }
                let mut element = row.child(div().flex_1().h_full()).into_any_element();
                element.layout_as_root(bounds.size.map(AvailableSpace::Definite), window, cx);
                element.prepaint_at(bounds.origin, window, cx);
                element
            },
            |_, mut element, window, cx| element.paint(window, cx),
        )
        .w_full()
        .h(px(32.))
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn breadcrumbs_keep_posix_home_drive_and_unc_navigation_targets() {
        let targets = |path, windows| {
            parts(path, windows)
                .into_iter()
                .map(|part| part.path)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            targets("/var/log/nginx", false),
            ["/var", "/var/log", "/var/log/nginx"]
        );
        assert_eq!(targets("~/src", false), ["~", "~/src"]);
        assert_eq!(targets("/", false), ["/"]);
        assert_eq!(
            targets("C:\\Users\\fixture", true),
            ["C:\\", "C:\\Users", "C:\\Users\\fixture"]
        );
        assert_eq!(
            targets("\\\\server\\share\\folder", true),
            ["\\\\server\\share", "\\\\server\\share\\folder"]
        );
        assert_eq!(
            targets(r"\\?\C:\Users", true),
            [r"\\?\C:\", r"\\?\C:\Users"]
        );
        assert_eq!(
            targets(r"\\?\UNC\server\share\folder", true),
            [r"\\?\UNC\server\share", r"\\?\UNC\server\share\folder"]
        );
        assert!(targets("", false).is_empty());
    }
    #[test]
    fn shrinking_breadcrumbs_hides_the_prefix_and_retains_the_current_directory() {
        assert_eq!(visible_start(&[40., 60., 80.], 220.), 0);
        assert_eq!(visible_start(&[40., 60., 80.], 150.), 2);
        assert_eq!(visible_start(&[40., 60., 80.], 60.), 2);
        assert_eq!(visible_start(&[], 0.), 0);
    }
}
