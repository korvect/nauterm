//! Variable-height message materialization with stable pixel-scroll anchoring.
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Rows {
    rows: BTreeMap<i64, Row>,
    anchor: Option<i64>,
}
struct Row {
    revision: u64,
    height: f32,
    width: f32,
}
impl Rows {
    pub fn estimate(&self, key: i64, estimate: impl FnOnce() -> f32) -> f32 {
        self.rows
            .get(&key)
            .map(|row| row.height)
            .unwrap_or_else(estimate)
    }
    pub fn retain(&mut self, alive: &BTreeSet<i64>) {
        self.rows.retain(|key, _| alive.contains(key));
        self.anchor = None;
    }
    pub fn slot(
        &mut self,
        key: i64,
        revision: u64,
        estimate: f32,
        top: f32,
        viewport: std::ops::Range<f32>,
        full: bool,
    ) -> (f32, bool) {
        let (start, end) = (viewport.start, viewport.end);
        let row = self.rows.entry(key).or_insert(Row {
            revision,
            height: estimate.max(32.),
            width: 0.,
        });
        if row.revision != revision {
            row.revision = revision;
            row.width = 0.;
        }
        let bottom = top + row.height;
        if self.anchor.is_none() && bottom >= start {
            self.anchor = Some(key);
        }
        (
            row.height,
            full || (bottom >= start - 600. && top <= end + 600.),
        )
    }
    pub fn measure(&mut self, key: i64, revision: u64, width: f32, height: f32) -> Option<f32> {
        let row = self
            .rows
            .get_mut(&key)
            .filter(|row| row.revision == revision)?;
        if !height.is_finite() || height <= 0. || !width.is_finite() || width <= 0. {
            return None;
        }
        let delta = height - row.height;
        let changed = delta.abs() >= 0.5 || (row.width - width).abs() >= 0.5;
        row.height = height;
        row.width = width;
        changed.then(|| {
            if self.anchor.is_some_and(|anchor| key < anchor) {
                delta
            } else {
                0.
            }
        })
    }
}
pub(super) fn estimate(text: &str, width: f32, attachments: usize) -> f32 {
    let columns = (width.max(120.) / 7.).max(12.) as usize;
    let lines: usize = text
        .lines()
        .map(|line| line.chars().count().max(1).div_ceil(columns))
        .sum();
    (lines.max(1) as f32 * 19. + 30. + if attachments > 0 { 28. } else { 0. }).max(49.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_histories_materialize_only_the_viewport_and_overdraw() {
        let mut rows = Rows::default();
        let mut visible = 0;
        for i in 0..10_000 {
            visible += usize::from(
                rows.slot(i, 1, 100., i as f32 * 114., 570_000. ..570_600., false)
                    .1,
            );
        }
        assert!(visible < 20);
        assert_eq!(rows.measure(0, 1, 300., 140.), Some(40.));
        assert_eq!(rows.measure(0, 1, 300., 140.), None);
    }
    #[test]
    fn selection_materializes_every_row_and_stale_measurements_cannot_replace_edits() {
        let mut rows = Rows::default();
        assert!(rows.slot(1, 1, 80., 0., 10_000. ..10_500., true).1);
        rows.slot(1, 2, 80., 0., 0. ..500., false);
        assert_eq!(rows.measure(1, 1, 300., 200.), None);
        assert_eq!(rows.measure(1, 2, 300., 80.), Some(0.));
        rows.retain(&BTreeSet::new());
        assert!(rows.rows.is_empty());
    }
}
