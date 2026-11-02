//! Product Markdown skin over Base's retained parser/layout primitives.
//! Rendering never loads document images or opens links without a user click.
use super::{Button, ButtonCustomVariant};
use crate::palette::Palette;
use crate::typography::FontWeights;
use gpui_kit::{
    assets::IconName,
    base::text::{
        MarkdownExtensions, MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView,
        TextViewState, TextViewStyle, markdown_ast::Node,
    },
    component::Sizable,
    prelude::*,
    *,
};
use std::sync::Arc;

pub struct MarkdownDocument {
    state: Entity<TextViewState>,
    extensions: MarkdownExtensions,
    source: String,
}
impl MarkdownDocument {
    pub fn new(cx: &mut App) -> Self {
        Self {
            state: cx.new(|cx| TextViewState::markdown("", cx)),
            extensions: MarkdownExtensions::default()
                .soft_line_breaks(true)
                .plugin(PassiveInline)
                .plugin(PassiveHtml),
            source: String::new(),
        }
    }
    pub fn state(&self) -> &Entity<TextViewState> {
        &self.state
    }
    pub fn set_source(&mut self, source: &str, cx: &mut App) {
        if self.source == source {
            return;
        }
        self.source.clear();
        self.source.push_str(source);
        // Base recognizes an appended prefix and only reparses its mutable tail.
        self.state
            .update(cx, |state, cx| state.set_text(source, cx));
    }
    pub fn view(&self, palette: Palette) -> AnyElement {
        let mut code = div()
            .min_w_0()
            .px(px(10.))
            .pt(px(36.))
            .pb(px(10.))
            .rounded(px(7.))
            .text_size(px(12.))
            .font_family("monospace")
            .border_1()
            .border_color(palette.border)
            .line_height(relative(1.45));
        let mut inline = div().text_size(px(12.)).font_family("monospace");
        let mut list = div().min_w(px(20.)).text_color(palette.text);
        let mut quote = div()
            .pl(px(10.))
            .pr(px(8.))
            .py(px(7.))
            .border_l_2()
            .border_color(palette.accent)
            .bg(palette.background)
            .text_color(palette.muted)
            .text_size(px(13.))
            .line_height(relative(1.4));
        let mut cell = div().px(px(8.)).py(px(6.)).text_size(px(12.));
        let mut head = div().text_size(px(12.)).font_weight(FontWeights::SEMIBOLD);
        let style = TextViewStyle::default()
            .with_foreground(palette.text.into())
            .with_muted_foreground(palette.muted.into())
            .with_link(palette.accent.into())
            .with_border(palette.border.into())
            .with_selection(palette.accent.alpha(0.24).into())
            .with_dark(
                palette.background.r * 0.2126
                    + palette.background.g * 0.7152
                    + palette.background.b * 0.0722
                    < 0.5,
            )
            .with_code_background(palette.control.into())
            .with_inline_code(HighlightStyle {
                color: Some(palette.text.into()),
                background_color: Some(palette.control.into()),
                ..Default::default()
            })
            .with_inline_code_style(inline.style().clone())
            .with_list_style(list.style().clone())
            .with_blockquote_style(quote.style().clone())
            .with_table_cell(cell.style().clone())
            .with_table_head(head.style().clone())
            .with_paragraph_gap(rems(0.5625))
            .with_code_block(code.style().clone())
            .with_heading(|level| {
                let mut heading = div()
                    .text_size(px(match level {
                        1 => 18.,
                        2 => 16.,
                        3 => 14.,
                        _ => 13.,
                    }))
                    .font_weight(FontWeights::SEMIBOLD)
                    .line_height(relative(1.3));
                heading.style().clone()
            });
        TextView::new(&self.state)
            .selectable(true)
            .scrollable(false)
            .markdown_extensions(self.extensions.clone())
            .style(style)
            // Even an old parse still in flight uses this resolver, so neither
            // a remote URL, file: URL nor data image can bypass the text policy.
            .image_source(|_| ImageSource::Custom(Arc::new(|_, _| None)))
            .on_link_click(|href, event, _, cx| {
                let activate = match event {
                    ClickEvent::Mouse(click) => {
                        matches!(click.up.button, MouseButton::Left | MouseButton::Middle)
                    }
                    ClickEvent::Keyboard(_) => true,
                    ClickEvent::Touch(click) => !click.long_press,
                };
                if activate && let Some(url) = external_link(href) {
                    cx.open_url(url.as_str());
                }
            })
            .code_block_actions(move |block, _, _| {
                let code = block.code();
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(10.))
                    .text_color(palette.muted)
                    .child(
                        div()
                            .max_w(px(100.))
                            .overflow_hidden()
                            .child(block.lang().unwrap_or_else(|| "Code".into())),
                    )
                    .child(
                        Button::new("copy-code")
                            .xsmall()
                            .ghost()
                            .custom(ButtonCustomVariant::for_surface(palette))
                            .size(px(20.))
                            .p_0()
                            .icon_size(px(13.))
                            .text_color(palette.muted)
                            .icon(IconName::Copy)
                            .tooltip("Copy code")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()))
                            }),
                    )
            })
            .text_size(px(13.))
            .line_height(relative(1.45))
            .into_any_element()
    }
}

fn external_link(href: &str) -> Option<url::Url> {
    let url = url::Url::parse(href.trim()).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then_some(url)
}
fn image_label(alt: &str) -> String {
    if alt.trim().is_empty() {
        "Image".into()
    } else {
        alt.into()
    }
}
struct PassiveInline;
impl MarkdownPlugin for PassiveInline {
    fn name(&self) -> &str {
        "nauterm-passive-inline"
    }
    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let label = match node {
            Node::Image(image) => image_label(&image.alt),
            Node::ImageReference(image) => image_label(&image.alt),
            Node::Html(html) => html.value.clone(),
            _ => return None,
        };
        Some(
            MarkdownNode::new(self.name(), ())
                .text(label)
                .markdown(cx.node_source(node).unwrap_or_default().to_owned()),
        )
    }
}
struct PassiveHtml;
impl MarkdownPlugin for PassiveHtml {
    fn is_block(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "nauterm-passive-html"
    }
    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Html(html) = node else {
            return None;
        };
        Some(
            MarkdownNode::new(self.name(), ())
                .text(html.value.clone())
                .markdown(cx.node_source(node).unwrap_or_default().to_owned()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{external_link, image_label};
    #[test]
    fn assistant_links_only_accept_absolute_web_urls() {
        for link in ["https://example.com/path", "http://localhost:8080"] {
            assert!(external_link(link).is_some());
        }
        for link in [
            "javascript:alert(1)",
            "file:///tmp/private",
            "data:text/html,test",
            "mailto:a@example.com",
            "//example.com",
            "/tmp/file",
            "https://",
        ] {
            assert!(external_link(link).is_none(), "{link}");
        }
        assert_eq!(image_label("diagram"), "diagram");
        assert_eq!(image_label(""), "Image");
    }
}
