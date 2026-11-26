//! The header inside the whole window: what its clicks do to Files and to the focus.
use super::*;

/// A link removed when dropped.
struct Unlink(PathBuf);

impl Drop for Unlink {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One pass of the whole window: the texts and icons it painted, and what it copied.
fn window_pass(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
) -> Painted {
    *time += 0.05;
    let mut frame = eframe::Frame::_new_kittest();
    let input = egui::RawInput {
        time: Some(*time),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 700.0),
        )),
        events,
        ..Default::default()
    };
    eframe::App::logic(window, ctx, &mut frame);
    let output = ctx.run_ui(input, |ui| eframe::App::ui(window, ui, &mut frame));
    let mut texts = Vec::new();
    for clipped in &output.shapes {
        if let egui::Shape::Text(text) = &clipped.shape {
            texts.push((
                text.galley.text().to_owned(),
                text.galley.rect.translate(text.pos.to_vec2()),
            ));
        }
        if let Some((glyph, _, rect)) = crate::ui::icons::painted_icon(&clipped.shape) {
            texts.push((glyph, rect));
        }
    }
    let copied = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    let shapes = painted_shapes(&output.shapes);
    output.drop_without_applying_deltas();
    Painted {
        texts,
        copied,
        shapes,
        widgets: Vec::new(),
    }
}

/// Run window passes until `done` holds, or fail saying `what` never happened.
fn until(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    what: &str,
    mut done: impl FnMut(&crate::Window, &Painted) -> bool,
) -> Painted {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let painted = window_pass(window, ctx, time, vec![]);
        if done(window, &painted) {
            return painted;
        }
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn window_click(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    at: egui::Pos2,
) -> Vec<String> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let mut copied = Vec::new();
    for events in [
        vec![egui::Event::PointerMoved(at)],
        vec![button(true)],
        vec![button(false)],
    ] {
        copied.extend(window_pass(window, ctx, time, events).copied);
    }
    copied
}
crate::windows_unix_omission!(
    a_folder_of_the_path_shows_in_files_and_the_menu_keeps_its_clicks,
    "unix-only: tests filesystem symlink path resolution in the Linux file tree"
);

/// In the whole window: a folder of the path shows that folder in Files, and the menu, which
/// floats over the drawer, keeps its clicks — the item acts and no console is focused by it.
#[cfg(target_os = "linux")]
#[test]
fn a_folder_of_the_path_shows_in_files_and_the_menu_keeps_its_clicks() {
    for zoom in [1.0, 1.25, 1.5] {
        let tree = Tree::new(&["r/src/a.ts"]);
        let root = tree.at("r");
        let script = format!("cd '{}' && exec sleep 30", root.display());
        let mut window = crate::tests::window_of_tabs([[script.as_str(), "sleep 30"]]);
        crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
        let pane = window.active().panes()[0].name;
        let ctx = crate::ui::icons::installed_for_tests(zoom);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut time = 0.0;
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
        until(
            &mut window,
            &ctx,
            &mut time,
            "Files reaching the console's folder",
            |w, _| crate::explorer::root_of(w, pane).as_ref() == Some(&root),
        );
        assert!(crate::viewer::open_document_target(
            &mut window,
            pane,
            tree.at("r/src/a.ts"),
            None
        ));
        let painted = until(&mut window, &ctx, &mut time, "the header", |w, p| {
            w.sidebar.last_dock.is_some_and(|dock| {
                p.texts
                    .iter()
                    .any(|(t, r)| t == "›" && dock.contains(r.center()))
            })
        });
        drop(painted);
        // The drawer at its shortest, where the menu has the least room under the header.
        window.sidebar.set_height(0.0);
        window_pass(&mut window, &ctx, &mut time, vec![]);
        let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
        let dock = window.sidebar.last_dock.expect("the drawer is placed");
        assert!(dock.height() <= 170.0, "the drawer stayed tall: {dock:?}");

        // A menu item over the drawer: it copies, and the focus stays where it was.
        let focused = window.active().focus;
        let more = painted.at_in(egui_phosphor::regular::CARET_DOWN, dock);
        window_click(&mut window, &ctx, &mut time, more);
        let menu = window_pass(&mut window, &ctx, &mut time, vec![]);
        for item in [
            "Open in File Manager",
            "Copy absolute path",
            "Copy relative path",
            "Wrap lines",
        ] {
            let at = menu.at(item);
            assert!(
                dock.contains(at),
                "{item} at {at:?} is outside the drawer {dock:?}"
            );
        }
        let item = menu.at("Copy relative path");
        let copied = window_click(&mut window, &ctx, &mut time, item);
        assert_eq!(copied, ["src/a.ts"]);
        assert_eq!(window.active().focus, focused);

        // With Files closed, a folder of the path opens Files, shows the folder expanded and
        // selected inside the tree it already had, and the file stays in the drawer.
        crate::sidebar::close_files(&mut window);
        window_pass(&mut window, &ctx, &mut time, vec![]);
        let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
        assert!(!crate::sidebar::files_visible(&window));
        let dock = window.sidebar.last_dock.expect("the drawer is placed");
        window_click(&mut window, &ctx, &mut time, painted.at_in("src", dock));
        let painted = until(
            &mut window,
            &ctx,
            &mut time,
            "Files showing the folder",
            |w, _| crate::explorer::selected_of(w, pane) == Some((root.join("src"), true)),
        );
        assert_eq!(
            crate::explorer::root_of(&window, pane),
            Some(root.clone()),
            "the folder's root moved"
        );
        assert!(crate::sidebar::files_visible(&window));
        let dock = window
            .sidebar
            .last_dock
            .expect("the drawer is still placed");
        painted.at_in("a.ts", dock);
    }
}

crate::windows_unix_omission!(
    a_folder_of_the_path_under_a_linked_root_shows_inside_the_tree,
    "unix-only: tests filesystem symlink path resolution in the Linux file tree"
);

/// A Files root reached through a link: the relative path still counts from it, a folder of the
/// path shows inside the tree Files already has, keeping that root, and once Files has moved
/// elsewhere the same folder opens as a new root.
#[cfg(target_os = "linux")]
#[test]
fn a_folder_of_the_path_under_a_linked_root_shows_inside_the_tree() {
    let tree = Tree::new(&["a/src/x.ts", "b/src/x.ts", "a/other/y.ts", "b/other/y.ts"]);
    let link = tree.0.with_file_name(format!(
        "{}-link",
        tree.0.file_name().unwrap().to_string_lossy()
    ));
    std::os::unix::fs::symlink(&tree.0, &link).unwrap();
    let _unlink = Unlink(link.clone());
    let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
    // The probe hands each console a folder under `link` as given, links unresolved.
    crate::project_view::configure(
        &mut window,
        crate::settings::Entry::Normal,
        Some(link.clone()),
    );
    let pane = window.active().panes()[0].name;
    let root = link.join(if pane % 2 == 0 { "a" } else { "b" });
    let ctx = crate::ui::icons::installed_for_tests(1.0);
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    let mut time = 0.0;
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    until(
        &mut window,
        &ctx,
        &mut time,
        "Files at the linked root",
        |w, _| crate::explorer::root_of(w, pane).as_ref() == Some(&root),
    );
    assert!(crate::viewer::open_document_target(
        &mut window,
        pane,
        root.join("src/x.ts"),
        None
    ));
    let painted = until(&mut window, &ctx, &mut time, "the header", |w, p| {
        w.sidebar.last_dock.is_some_and(|dock| {
            p.texts
                .iter()
                .any(|(t, r)| t == "›" && dock.contains(r.center()))
        })
    });
    let dock = window.sidebar.last_dock.expect("the drawer is placed");
    assert!(crate::sidebar::files_visible(&window));

    // The relative path counts from the root, links followed on both sides.
    let more = painted.at_in(egui_phosphor::regular::CARET_DOWN, dock);
    window_click(&mut window, &ctx, &mut time, more);
    let menu = window_pass(&mut window, &ctx, &mut time, vec![]);
    let copied = window_click(&mut window, &ctx, &mut time, menu.at("Copy relative path"));
    assert_eq!(copied, ["src/x.ts"]);

    // Inside the root Files shows: revealed in its tree, the root kept as Files spelled it.
    let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
    let src = painted.at_in("src", dock);
    window_click(&mut window, &ctx, &mut time, src);
    until(&mut window, &ctx, &mut time, "Files showing src", |w, _| {
        crate::explorer::selected_of(w, pane) == Some((root.join("src"), true))
    });
    assert_eq!(crate::explorer::root_of(&window, pane), Some(root.clone()));

    // Files moved to another folder: the same click opens src as Files' new root.
    crate::explorer::open_path(&mut window, pane, root.join("other"));
    until(
        &mut window,
        &ctx,
        &mut time,
        "Files at the other folder",
        |w, _| crate::explorer::root_of(w, pane) == Some(root.join("other")),
    );
    let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
    let dock = window
        .sidebar
        .last_dock
        .expect("the drawer is still placed");
    window_click(&mut window, &ctx, &mut time, painted.at_in("src", dock));
    until(
        &mut window,
        &ctx,
        &mut time,
        "Files rooted at src",
        |w, _| crate::explorer::root_of(w, pane) == Some(root.join("src")),
    );
}
