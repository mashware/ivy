use super::*;
use crate::tests::{one_frame_in, window_of_tabs};

fn sources() -> Vec<(usize, Option<PathBuf>)> {
    vec![
        (1, Some("/repo/main".into())),
        (2, Some("/repo/worktree".into())),
        (3, Some("/repo/main".into())),
    ]
}

#[test]
fn panes_and_worktrees_restore_independent_navigation_and_restart_hidden() {
    let mut state = Explorer::default();
    state.reconcile(&sources());
    let nav = state.navigation.get_mut(&1).unwrap();
    nav.expanded.insert("/repo/main/src".into());
    nav.selected = Some("/repo/main/src/a.md".into());
    nav.scroll = 120.0;
    state.visible = true;
    state.pinned = Some(1);
    assert_eq!(state.target(Some(2)), Some(1));
    state.hide();
    state.visible = true;
    assert_eq!(state.navigation[&1].scroll, 120.0);
    assert_eq!(state.navigation[&2].root, Some("/repo/worktree".into()));
    assert_eq!(state.navigation[&3].root, Some("/repo/main".into()));
    assert!(state.navigation[&2].expanded.is_empty());
    assert!(state.navigation[&3].selected.is_none());
    state.pinned = None;
    assert_eq!(state.target(Some(2)), Some(2));
    assert_eq!(state.target(Some(1)), Some(1));
    let restarted = Explorer::default();
    assert!(!restarted.visible && restarted.pinned.is_none() && restarted.navigation.is_empty());
}

#[test]
fn files_pin_expansion_and_scroll_survive_shared_sidebar_tool_switches() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ids: Vec<_> = window.all_panes().map(|pane| pane.name).collect();
    window.explorer.reconcile(&[
        (ids[0], Some("/repo/a".into())),
        (ids[1], Some("/repo/b".into())),
    ]);
    let nav = window.explorer.navigation.get_mut(&ids[0]).unwrap();
    nav.expanded.insert("/repo/a/src".into());
    nav.selected = Some("/repo/a/src/lib.rs".into());
    nav.scroll = 189.0;
    window.explorer.pinned = Some(ids[0]);

    for tool in [
        crate::sidebar::Tool::Files,
        crate::sidebar::Tool::Markdown,
        crate::sidebar::Tool::Board,
        crate::sidebar::Tool::Files,
    ] {
        crate::sidebar::select(&mut window, tool);
    }
    let nav = &window.explorer.navigation[&ids[0]];
    assert!(nav.expanded.contains(Path::new("/repo/a/src")));
    assert_eq!(
        nav.selected.as_deref(),
        Some(Path::new("/repo/a/src/lib.rs"))
    );
    assert_eq!(nav.scroll, 189.0);
    assert_eq!(window.explorer.pinned, Some(ids[0]));
}

#[test]
fn source_epochs_and_pane_liveness_reject_stale_results() {
    let mut state = Explorer::default();
    state.reconcile(&sources());
    let request = state.request(1, "/repo/main".into());
    assert!(state.current(&request));
    state.reconcile(&[(1, None)]);
    assert!(!state.current(&request));
    state.reconcile(&sources());
    assert!(!state.current(&request));
    state.pinned = Some(1);
    state.reconcile(&[(2, Some("/repo/worktree".into()))]);
    assert_eq!(state.pinned, None);
    assert!(state.message.contains("Pinned console closed"));
    assert!(!state.navigation.contains_key(&1));
    assert!(!state.current(&request));
}

#[test]
fn cache_eviction_never_discards_navigation() {
    let mut state = Explorer::default();
    state.reconcile(&sources());
    state.navigation.get_mut(&1).unwrap().scroll = 120.0;
    for index in 0..33 {
        let request = state.request(1, PathBuf::from(format!("/repo/main/{index}")));
        state.cache.insert(
            request,
            Cached {
                result: Ok(Listing {
                    entries: vec![],
                    bytes: 0,
                }),
                used: index,
            },
        );
    }
    state.evict();
    assert_eq!(state.cache.len(), CACHE_COUNT);
    assert_eq!(state.navigation[&1].scroll, 120.0);
}

#[test]
fn explorer_uses_the_exact_host_allocation() {
    let state = Explorer::default();
    for width in [700.0, 320.0, 120.0] {
        let area = egui::Rect::from_min_size(egui::pos2(800.0, 40.0), egui::vec2(width, 600.0));
        assert_eq!(state.rect(area), area);
    }
}

#[test]
fn file_display_escapes_controls_without_changing_the_original_path() {
    let path = PathBuf::from("/tmp/a\n\u{202e}.md");
    assert_eq!(display(&path), "/tmp/a\\u{a}\\u{202e}.md");
    assert_eq!(path.to_str().unwrap(), "/tmp/a\n\u{202e}.md");
}

#[test]
fn explorer_escape_and_keyboard_never_resize_or_close_consoles() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    one_frame_in(&mut window, &ctx, area, vec![]);
    let before: Vec<_> = window.all_panes().map(|p| (p.name, p.measured)).collect();
    toggle(&mut window, &ctx);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert!(window.explorer.visible);
    one_frame_in(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!window.explorer.visible);
    assert_eq!(
        window
            .all_panes()
            .map(|p| (p.name, p.measured))
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn cache_pressure_keeps_the_current_tree_root_instead_of_reloading_it() {
    let mut state = Explorer::default();
    state.reconcile(&sources());
    let root = state.request(1, "/repo/main".into());
    state.cache.insert(
        root.clone(),
        Cached {
            result: Ok(Listing {
                entries: vec![],
                bytes: 0,
            }),
            used: 0,
        },
    );
    for index in 1..=32 {
        let child = state.request(1, PathBuf::from(format!("/repo/main/{index}")));
        state.cache.insert(
            child,
            Cached {
                result: Ok(Listing {
                    entries: vec![],
                    bytes: 0,
                }),
                used: index,
            },
        );
    }
    state.evict();
    assert_eq!(state.cache.len(), CACHE_COUNT);
    assert!(
        state.cache.contains_key(&root),
        "evicting root makes the next frame reload the whole tree"
    );
}
