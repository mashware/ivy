use std::{collections::HashSet, path::PathBuf};

#[derive(Default)]
pub(super) struct Navigation {
    pub(super) root: Option<PathBuf>,
    pub(super) source: Option<PathBuf>,
    pub(super) seeded_source: Option<PathBuf>,
    pub(super) expanded: HashSet<PathBuf>,
    pub(super) selected: Option<PathBuf>,
    pub(super) scroll: f32,
    pub(super) row_limit: usize,
    pub(super) query: String,
    pub(super) reveal_selected: bool,
    pub(super) generation: u64,
    pub(super) epoch: u64,
}

impl Navigation {
    pub(super) fn observe(&mut self, source: Option<PathBuf>) -> bool {
        if self.source == source {
            return false;
        }
        self.source = source;
        self.epoch = self.epoch.wrapping_add(1);
        if self.source.is_some() && (self.root.is_none() || self.source != self.seeded_source) {
            self.query.clear();
            self.use_source();
        }
        true
    }

    pub(super) fn use_source(&mut self) {
        if let Some(path) = self.source.clone() {
            self.seeded_source = Some(path.clone());
            self.enter(path);
        }
    }

    pub(super) fn enter(&mut self, path: PathBuf) {
        self.root = Some(path);
        self.query.clear();
        self.row_limit = super::ROW_LIMIT;
        self.expanded.clear();
        self.selected = None;
        self.reveal_selected = false;
        self.scroll = 0.0;
        self.refresh();
    }

    pub(super) fn reload(&mut self) {
        if self.source.is_some() && self.source != self.seeded_source {
            self.use_source();
        } else {
            self.refresh();
        }
    }

    pub(super) fn refresh(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_changes_follow_the_console_and_remote_sources_suspend_it() {
        let mut nav = Navigation::default();
        nav.observe(Some("/work/a".into()));
        nav.expanded.insert("/work/a/src".into());
        nav.selected = Some("/work/a/src/a.md".into());
        nav.scroll = 120.0;
        let epoch = nav.epoch;
        nav.observe(None);
        nav.observe(Some("/work/b".into()));
        assert_eq!(nav.epoch, epoch + 2);
        assert_eq!(nav.root, Some("/work/b".into()));
        assert!(nav.expanded.is_empty());
        assert!(nav.selected.is_none());
        assert_eq!(nav.scroll, 0.0);
    }
    #[test]
    fn same_source_reload_preserves_expansion_selection_and_scroll() {
        let mut nav = Navigation::default();
        nav.observe(Some("/work/a".into()));
        nav.expanded.insert("/work/a/src".into());
        nav.selected = Some("/work/a/src/a.md".into());
        nav.scroll = 120.0;
        let generation = nav.generation;
        nav.reload();
        assert_eq!(nav.generation, generation + 1);
        assert!(nav.expanded.contains(std::path::Path::new("/work/a/src")));
        assert_eq!(nav.selected, Some("/work/a/src/a.md".into()));
        assert_eq!(nav.scroll, 120.0);
    }

    #[test]
    fn deliberate_browsing_does_not_report_a_console_directory_change() {
        let mut nav = Navigation::default();
        nav.observe(Some("/work/a".into()));
        nav.enter("/work".into());
        assert_eq!(nav.source, nav.seeded_source);
        nav.observe(Some("/work/b".into()));
        assert_eq!(nav.root, Some("/work/b".into()));
        assert_eq!(nav.source, nav.seeded_source);
        assert_eq!(nav.source, nav.seeded_source);
        assert_eq!(nav.root, nav.source);
    }
}
