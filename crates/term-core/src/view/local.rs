use super::{CellWidth, View};

impl View {
    /// Candidate under a painted cell, limited to complete visible wrapped lines.
    pub fn local_path_at(&self, row: u16, col: u16) -> Option<String> {
        if row >= self.size.rows || col >= self.size.cols {
            return None;
        }
        let cols = self.size.cols as usize;
        let clicked = row as usize * cols + col as usize;
        if let Some(span) = self
            .osc8
            .iter()
            .find(|s| s.start <= clicked && clicked < s.end)
        {
            return Some(span.target.clone());
        }
        let mut first = row as usize;
        while first > 0 && self.row_wraps[first - 1] {
            first -= 1;
        }
        let mut last = row as usize;
        while last + 1 < self.size.rows as usize && self.row_wraps[last] {
            last += 1;
        }
        let mut glyphs = Vec::new();
        for n in first * cols..(last + 1) * cols {
            let cell = &self.cells[n];
            if matches!(cell.width, CellWidth::WideTail | CellWidth::WrapFiller) {
                continue;
            }
            glyphs.push((n, cell.c));
        }
        let pos = glyphs.iter().rposition(|(n, _)| *n <= clicked)?;
        let boundary = |c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | '`' | ','
                )
        };
        if boundary(glyphs[pos].1) {
            return None;
        }
        let mut start = pos;
        while start > 0 && !boundary(glyphs[start - 1].1) {
            start -= 1;
        }
        let mut end = pos + 1;
        while end < glyphs.len() && !boundary(glyphs[end].1) {
            end += 1;
        }
        if start == 0 && first == 0 && self.continued_from_above {
            return None;
        }
        if end == glyphs.len() && last + 1 == self.size.rows as usize && self.row_wraps[last] {
            return None;
        }
        Some(glyphs[start..end].iter().map(|(_, c)| c).collect())
    }
}
