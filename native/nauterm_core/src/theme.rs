use alacritty_terminal::vte::ansi::Rgb;

pub const DEFAULT_BACKGROUND: Rgb = Rgb {
    r: 0xfb,
    g: 0xfb,
    b: 0xf8,
};

pub const DEFAULT_FOREGROUND: Rgb = Rgb {
    r: 0x34,
    g: 0x38,
    b: 0x42,
};

pub const DEFAULT_CURSOR: Rgb = Rgb {
    r: 0x9e,
    g: 0xc6,
    b: 0xee,
};

pub const ANSI_PALETTE: [Rgb; 16] = [
    Rgb {
        r: 0x34,
        g: 0x38,
        b: 0x42,
    },
    Rgb {
        r: 0xd9,
        g: 0x5f,
        b: 0x56,
    },
    Rgb {
        r: 0x43,
        g: 0xa4,
        b: 0x6f,
    },
    Rgb {
        r: 0xb8,
        g: 0x84,
        b: 0x16,
    },
    Rgb {
        r: 0x1f,
        g: 0x73,
        b: 0xd8,
    },
    Rgb {
        r: 0x9d,
        g: 0x52,
        b: 0xa8,
    },
    Rgb {
        r: 0x12,
        g: 0x98,
        b: 0xaa,
    },
    Rgb {
        r: 0x68,
        g: 0x70,
        b: 0x7d,
    },
    Rgb {
        r: 0x55,
        g: 0x5b,
        b: 0x68,
    },
    Rgb {
        r: 0xe4,
        g: 0x6f,
        b: 0x67,
    },
    Rgb {
        r: 0x63,
        g: 0xb9,
        b: 0x87,
    },
    Rgb {
        r: 0xd6,
        g: 0xa7,
        b: 0x3d,
    },
    Rgb {
        r: 0x4b,
        g: 0x9c,
        b: 0xf0,
    },
    Rgb {
        r: 0xb8,
        g: 0x6a,
        b: 0xc6,
    },
    Rgb {
        r: 0x47,
        g: 0xb7,
        b: 0xc6,
    },
    Rgb {
        r: 0x85,
        g: 0x8d,
        b: 0x99,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_white_sentinels_do_not_collide_with_default_background() {
        assert_ne!(ANSI_PALETTE[7], DEFAULT_BACKGROUND);
        assert_ne!(ANSI_PALETTE[15], DEFAULT_BACKGROUND);
    }
}
