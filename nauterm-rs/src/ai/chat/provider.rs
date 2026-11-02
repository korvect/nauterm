use super::*;

impl Chat {
    pub(super) fn provider_controls(&self, busy: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let rows = self.providers.clone();
        let selected = self.provider.clone();
        let model = self.model.read(cx).value().trim().to_owned();
        let current = rows.iter().find(|provider| provider.uuid == selected);
        let label = if !model.is_empty() {
            model.clone()
        } else {
            current
                .map(|provider| provider.name.clone())
                .unwrap_or_else(|| {
                    crate::localization::text(
                        cx,
                        if selected.is_some() {
                            "Unavailable provider"
                        } else {
                            "Choose provider"
                        },
                    )
                    .to_string()
                })
        };
        let full_label = current
            .map(|provider| {
                if model.is_empty() {
                    provider.name.clone()
                } else {
                    format!("{} / {model}", provider.name)
                }
            })
            .unwrap_or_else(|| label.clone());
        let owner = cx.weak_entity();
        let language = crate::localization::language(cx);
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .justify_end()
            .child(
                Button::new("ai-provider-model")
                    .ghost()
                    .custom(ButtonCustomVariant::for_surface(p))
                    .h(px(26.))
                    .min_w_0()
                    .max_w_full()
                    .px(px(3.))
                    .content_gap(px(3.))
                    .text_size(px(11.5))
                    .line_height(relative(1.5))
                    .text_color(p.muted)
                    .child(crate::components::FittingLabel::new(
                        full_label.clone(),
                        label,
                    ))
                    .trailing_icon(IconName::ChevronDown)
                    .icon_size(px(14.))
                    .localized_accessibility_label("AI provider and model")
                    .raw_tooltip(full_label)
                    .disabled(busy || self.provider_loading)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu.width(260.).palette(p);
                        for provider in &rows {
                            let mut models: Vec<String> = provider
                                .model
                                .split(',')
                                .map(str::trim)
                                .filter(|model| !model.is_empty())
                                .map(str::to_owned)
                                .collect();
                            if provider.uuid == selected
                                && !model.is_empty()
                                && !models.contains(&model)
                            {
                                models.push(model.clone());
                            }
                            if models.is_empty() {
                                models.push(String::new());
                            }
                            let mut seen = std::collections::BTreeSet::new();
                            for candidate in models
                                .into_iter()
                                .filter(|model| seen.insert(model.clone()))
                            {
                                let owner = owner.clone();
                                let uuid = provider.uuid.clone();
                                let checked = uuid == selected && candidate == model;
                                menu = menu.item(
                                    MenuItem::new(if candidate.is_empty() {
                                        crate::localization::text_for(language, "Default model")
                                            .to_string()
                                    } else {
                                        candidate.clone()
                                    })
                                    .caption(provider.name.clone())
                                    .checked(checked)
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.provider = uuid.clone();
                                                this.model.update(cx, |input, cx| {
                                                    input.set_value(candidate.clone(), window, cx)
                                                });
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                        }
                        let configure = owner.clone();
                        menu.separator().item(
                            MenuItem::localized("rs.ai.action.configureProviders")
                                .icon(IconName::Settings)
                                .on_click(move |_, _, cx| {
                                    let _ = configure.update(cx, |_, cx| cx.emit(Configure));
                                }),
                        )
                    }),
            )
            .into_any_element()
    }
}
