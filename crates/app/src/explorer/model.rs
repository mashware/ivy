use std::{collections::HashSet, path::PathBuf};

#[derive(Default)]
pub(super) struct Navigation {
    pub(super) root: Option<PathBuf>,
    pub(super) source: Option<PathBuf>,
    pub(super) expanded: HashSet<PathBuf>,
    pub(super) selected: Option<PathBuf>,
    pub(super) scroll: f32,
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
        if self.root.is_none()
            && let Some(path) = self.source.clone()
        {
            self.enter(path);
        }
        true
    }

    pub(super) fn enter(&mut self, path: PathBuf) {
        self.root = Some(path);
        self.expanded.clear();
        self.selected = None;
        self.scroll = 0.0;
        self.refresh();
    }

    pub(super) fn refresh(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cwd_changes_suspend_and_restore_navigation_until_explicit_reset() {
        let mut nav = Navigation::default();
        nav.observe(Some("/work/a".into()));
        nav.expanded.insert("/work/a/src".into());
        nav.selected = Some("/work/a/src/a.md".into());
        nav.scroll = 120.0;
        let epoch = nav.epoch;
        nav.observe(None);
        nav.observe(Some("/work/b".into()));
        assert_eq!(nav.epoch, epoch + 2);
        assert_eq!(nav.root, Some("/work/a".into()));
        assert_eq!(nav.scroll, 120.0);
        assert!(nav.expanded.contains(&PathBuf::from("/work/a/src")));
        nav.enter(nav.source.clone().unwrap());
        assert_eq!(nav.root, Some("/work/b".into()));
        assert!(nav.expanded.is_empty());
        assert!(nav.selected.is_none());
        assert_eq!(nav.scroll, 0.0);
    }
}
