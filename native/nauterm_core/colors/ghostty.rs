impl GhosttyTerminalEngine {
    fn rs_update_palette(&mut self, colors: crate::terminal::TerminalDefaultColors) -> bool {
        let foreground = GhosttyColorRgb::from_u32(colors.foreground_u32());
        let background = GhosttyColorRgb::from_u32(colors.background_u32());
        let cursor = GhosttyColorRgb::from_u32(colors.cursor_u32());
        let palette = colors.palette_u32().map(GhosttyColorRgb::from_u32);
        // Palette allocation happens first. The remaining scalar defaults do
        // not allocate. These options retain explicit application OSC overrides.
        for (option, value) in [
            (
                TERMINAL_OPT_COLOR_PALETTE,
                palette.as_ptr() as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_FOREGROUND,
                &foreground as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_BACKGROUND,
                &background as *const _ as *const c_void,
            ),
            (
                TERMINAL_OPT_COLOR_CURSOR,
                &cursor as *const _ as *const c_void,
            ),
        ] {
            if unsafe { ghostty_terminal_set(self.terminal, option, value) } != GHOSTTY_SUCCESS {
                return false;
            }
        }
        self.options.default_colors = colors;
        self.default_foreground = colors.foreground_u32();
        self.default_background = colors.background_u32();
        self.default_cursor = colors.cursor_u32();
        true
    }
}
