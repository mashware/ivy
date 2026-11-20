//! Where each line of the file starts, top to bottom: the code view lays out only what is on
//! screen, so the rest is placed by these heights.
use super::super::highlight::Line;

/// Unwrapped, every line is one row. Wrapped, a line is as tall as its layout: estimated from
/// its length until it is painted, then measured.
pub(super) struct Heights {
    /// What these were built for: the wrap width in whole points, if wrapped, and how many lines.
    key: Option<(Option<u32>, usize)>,
    heights: Vec<f32>,
    /// `tops[i]` is where line `i` starts; the last entry is where the file ends.
    tops: Vec<f32>,
    stale: bool,
}

/// No lines yet: the file ends where it starts, so `top` and `line_at` answer before `prepare`.
impl Default for Heights {
    fn default() -> Self {
        Self {
            key: None,
            heights: Vec::new(),
            tops: vec![0.0],
            stale: false,
        }
    }
}

impl Heights {
    /// Estimate again when the width, the wrapping or the number of lines changed.
    pub(super) fn prepare(&mut self, lines: &[Line], wrap: Option<f32>, row: f32, glyph: f32) {
        let key = (wrap.map(|width| width.round() as u32), lines.len());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let columns = wrap.map(|width| ((width / glyph).floor() as usize).max(1));
        self.heights = lines
            .iter()
            .map(|line| match columns {
                Some(columns) => {
                    let chars = line.text[..line.shown].chars().count().max(1);
                    chars.div_ceil(columns) as f32 * row
                }
                None => row,
            })
            .collect();
        self.rebuild();
    }

    pub(super) fn top(&self, line: usize) -> f32 {
        self.tops[line.min(self.tops.len() - 1)]
    }

    pub(super) fn total(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    /// The line at height `y`, counted from the top of the file.
    pub(super) fn line_at(&self, y: f32) -> usize {
        self.tops[1..]
            .partition_point(|top| *top <= y)
            .min(self.heights.len().saturating_sub(1))
    }

    /// What painting measured for `line`; the places below it move on the next frame.
    pub(super) fn measured(&mut self, line: usize, height: f32) {
        if let Some(known) = self.heights.get_mut(line)
            && (*known - height).abs() > 0.01
        {
            *known = height;
            self.stale = true;
        }
    }

    /// Bring the places in line with what was measured; whether anything moved.
    pub(super) fn settle(&mut self) -> bool {
        let moved = std::mem::take(&mut self.stale);
        if moved {
            self.rebuild();
        }
        moved
    }

    fn rebuild(&mut self) {
        self.tops.clear();
        self.tops.reserve(self.heights.len() + 1);
        self.tops.push(0.0);
        for height in &self.heights {
            self.tops
                .push(self.tops.last().copied().unwrap_or(0.0) + height);
        }
    }
}
