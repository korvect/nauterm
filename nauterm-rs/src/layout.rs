//! Geometry shared by rendering and pure model checks.
pub const SIDEBAR_WIDTH: f32 = 190.;
pub const COLLAPSED_SIDEBAR_WIDTH: f32 = 56.;
pub const DRAWER_WIDTH: f32 = 360.;
pub const CARD_GAP: f32 = 18.;
pub const CARD_MIN_WIDTH: f32 = 260.;
pub const CARD_MAX_WIDTH: f32 = 320.;

/// A connection can occupy a full tab, a drawer, or a short overview tile.
/// Bound the body from the actual container, keeping the footer out of its
/// scroll region. The normal-size reference spacing remains unchanged.
pub struct ConnectionMetrics {
    pub compact: bool,
    pub outer: f32,
    pub horizontal: f32,
    pub inner: f32,
    pub progress_top: f32,
    pub progress_bottom: f32,
    pub footer_gap: f32,
    pub body_max: f32,
}
impl ConnectionMetrics {
    pub fn with_footer_height(mut self, height: f32) -> Self {
        let baseline = if self.compact { 24. } else { 36. };
        self.body_max = (self.body_max - (height - baseline).max(0.)).max(0.);
        self
    }
    pub fn new(height: f32, drawer: bool) -> Self {
        let short = height < 480.;
        let compact = drawer || short;
        let outer = if compact { 16. } else { 32. };
        let inner = if compact { 0. } else { 22. };
        let progress_top = if short { 12. } else { 22. };
        let progress_bottom = if short { 12. } else { 28. };
        let footer_gap = if short { 12. } else { 26. };
        let fixed = 40.
            + 24.
            + progress_top
            + progress_bottom
            + footer_gap
            + if compact { 24. } else { 36. };
        Self {
            compact,
            outer,
            horizontal: if compact { 16. } else { 24. },
            inner,
            progress_top,
            progress_bottom,
            footer_gap,
            body_max: (height - outer * 2. - fixed).max(0.),
        }
    }
}

/// Available width after reserving right drawers, before laying out the
/// sidebar. Keeping the decision independent of its current animated width
/// prevents collapse/expand feedback at the breakpoint.
pub fn sidebar_width(available: f32) -> f32 {
    if available < 760. {
        COLLAPSED_SIDEBAR_WIDTH
    } else {
        SIDEBAR_WIDTH
    }
}

#[derive(Clone, Copy)]
pub struct CardGrid {
    pub columns: u16,
    pub max_width: Option<f32>,
}

impl CardGrid {
    pub fn new(content_width: f32, item_count: usize, list: bool) -> Self {
        if list {
            return Self {
                columns: 1,
                max_width: None,
            };
        }
        let columns = (((content_width.max(1.) + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor()
            as usize)
            .clamp(1, 5)
            .min(item_count.max(1));
        Self {
            columns: columns as u16,
            max_width: Some(CARD_MAX_WIDTH * columns as f32 + CARD_GAP * (columns - 1) as f32),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn short_connection_tiles_reserve_space_for_logs_without_covering_the_footer() {
        let normal = super::ConnectionMetrics::new(716., false);
        assert!(!normal.compact);
        assert_eq!(
            (normal.outer, normal.horizontal, normal.inner),
            (32., 24., 22.)
        );
        assert_eq!(
            (
                normal.progress_top,
                normal.progress_bottom,
                normal.footer_gap
            ),
            (22., 28., 26.)
        );
        for height in [280., 306., 396., 439.] {
            let tile = super::ConnectionMetrics::new(height, false);
            assert!(tile.compact);
            // One 18px error line and its gap leave a readable inner log area.
            assert!(tile.body_max - 18. - 16. >= 80.);
            let used = tile.outer * 2.
                + 40.
                + 24.
                + tile.progress_top
                + tile.progress_bottom
                + tile.body_max
                + tile.footer_gap
                + 24.;
            assert_eq!(used, height);
        }
        assert!(super::ConnectionMetrics::new(600., true).compact);
    }
    #[test]
    fn wrapped_connection_footer_reserves_an_extra_row_without_widening_the_drawer() {
        let normal = super::ConnectionMetrics::new(600., false);
        let wrapped = super::ConnectionMetrics::new(600., false).with_footer_height(80.);
        assert_eq!(wrapped.body_max, normal.body_max - 44.);
        let compact = super::ConnectionMetrics::new(320., true);
        assert_eq!(compact.horizontal, 16.);
        assert_eq!(
            super::ConnectionMetrics::new(320., true)
                .with_footer_height(56.)
                .body_max,
            compact.body_max - 32.
        );
        assert_eq!(
            super::ConnectionMetrics::new(320., true)
                .with_footer_height(1000.)
                .body_max,
            0.
        );
    }
    #[test]
    fn cards_reflow_with_drawer_and_never_grow_beyond_reference_maximum() {
        use super::CardGrid;
        let wide = CardGrid::new(1000., 8, false);
        assert_eq!((wide.columns, wide.max_width), (3, Some(996.)));
        assert_eq!(CardGrid::new(578., 8, false).columns, 2);
        assert_eq!(CardGrid::new(200., 8, false).columns, 1);
        assert_eq!(CardGrid::new(578., 8, true).max_width, None);
        assert_eq!(CardGrid::new(1600., 20, false).columns, 5);
        assert_eq!(CardGrid::new(4000., 20, false).max_width, Some(1672.));
        assert_eq!(CardGrid::new(1600., 2, false).max_width, Some(658.));
        // Fractional window sizes must not let Flex wrap the third card onto
        // a new row: tracks, not a rounded per-card width, own placement.
        for step in 0..2780 {
            let width = 816. + step as f32 / 10.;
            assert_eq!(CardGrid::new(width, 8, false).columns, 3);
        }
        for width in [260., 537.9, 538., 815.9, 816., 1093.9, 1094., 1372., 4000.] {
            let grid = CardGrid::new(width, 30, false);
            let used = width.min(grid.max_width.unwrap());
            let card_width =
                (used - super::CARD_GAP * f32::from(grid.columns - 1)) / f32::from(grid.columns);
            assert!((super::CARD_MIN_WIDTH..=super::CARD_MAX_WIDTH).contains(&card_width));
        }
        assert_eq!(super::sidebar_width(759.), 56.);
        assert_eq!(super::sidebar_width(760.), 190.);
    }
}
