impl TerminalDefaultColors {
    pub fn from_palette(values: &[u32]) -> Option<Self> {
        if values.len() != 19 || values.iter().any(|value| *value > 0xffffff) {
            return None;
        }
        Some(Self {
            foreground: u32_to_rgb(values[0], DEFAULT_FOREGROUND),
            background: u32_to_rgb(values[1], DEFAULT_BACKGROUND),
            cursor: u32_to_rgb(values[2], DEFAULT_CURSOR),
            rs_ansi: std::array::from_fn(|index| {
                u32_to_rgb(values[index + 3], ANSI_PALETTE[index])
            }),
        })
    }
}

impl TerminalEngine {
    fn rs_resolved_colors(
        &self,
        dynamic: &alacritty_terminal::term::color::Colors,
    ) -> alacritty_terminal::term::color::Colors {
        let mut resolved = *dynamic;
        for index in 0..259 {
            if resolved[index].is_none() {
                resolved[index] = Some(self.options.default_colors.color_for_index(index));
            }
        }
        resolved
    }
    fn rs_finish_color_queries(&self) {
        let requests = self
            .events
            .rs_color_requests
            .lock()
            .map(|mut requests| std::mem::take(&mut *requests))
            .unwrap_or_default();
        let colors = self.term.renderable_content().colors;
        for request in requests {
            if let Event::ColorRequest(index, formatter) = request {
                let color = if index < alacritty_terminal::term::color::COUNT {
                    colors[index]
                } else {
                    None
                }
                .unwrap_or_else(|| self.options.default_colors.color_for_index(index));
                if let Ok(mut writes) = self.events.pty_writes.lock() {
                    writes.push(formatter(color));
                }
            }
        }
    }
}
