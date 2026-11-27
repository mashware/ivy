//! Console keyboard routing, separate from window rendering.
use crate::{Window, WindowRequest, header, keyboard};
use eframe::egui;
use std::time::Instant;
mod navigation;

fn event_action(
    event: &egui::Event,
    scan: &mut egui::Modifiers,
    keymap: &keyboard::Keymap,
) -> Option<keyboard::Action> {
    match event {
        egui::Event::ModifiersChanged(value) => *scan = *value,
        egui::Event::WindowFocused(false) => *scan = egui::Modifiers::NONE,
        egui::Event::Key {
            key, pressed: true, ..
        } => {
            return keymap.action_for(keyboard::Chord {
                ctrl: scan.ctrl,
                shift: scan.shift,
                alt: scan.alt,
                key: *key,
            });
        }
        _ => {}
    }
    None
}

fn is_navigation(action: Option<keyboard::Action>) -> bool {
    matches!(
        action,
        Some(
            keyboard::Action::FocusLeft
                | keyboard::Action::FocusRight
                | keyboard::Action::FocusUp
                | keyboard::Action::FocusDown
                | keyboard::Action::MaximizePane
        )
    )
}

fn may_emit_text(event: &egui::Event, modifiers: egui::Modifiers) -> bool {
    use egui::Key;
    let egui::Event::Key { key, .. } = event else {
        return false;
    };
    if modifiers.ctrl || modifiers.command || modifiers.mac_cmd {
        return false;
    }
    match key {
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => false,
        Key::Quote | Key::Space | Key::Minus => true,
        key => key.symbol_or_name().len() == 1,
    }
}

struct Routing<'a> {
    keymap: &'a keyboard::Keymap,
    clipboard: keyboard::Clipboard,
    started: Instant,
}

/// Route each part of a keyboard batch to the console holding focus at that point.
///
/// It lives apart from the painting for the same reason as [`Window::handle_the_mouse`]: sending
/// it to both would write the same key into two programs, which is exactly what «without
/// stepping on each other» forbids, and inside the interface pass that decision cannot be
/// checked without opening a window.
///
/// **On a pass somebody typed in, the pass is timed whole and subtracted from the frame
/// (D13).** It used to be a declared hole — «in the batches nobody types» — and the
/// scrollback's search is the batch that types: a press of `Enter` walks the history right
/// here, so leaving it untimed would publish that walk as the interface framework's cost. The
/// press is also published on its own line of the report (D61); only this whole pass is
/// subtracted, so it is never subtracted twice.
///
/// **A pass nobody typed in declares nothing, and that is the hole that is left, said rather
/// than hidden.** It clones the keymap and walks an event list with no press, text or paste in
/// it — key releases and modifier changes, at most — and declaring it on every frame would hand
/// every other declaring path's test a non-zero it did not earn. «Typed» is the blinking cursor's
/// question (`keyboard::somebody_typed`): a press, a text or a paste. A copy on its own is not
/// in it, and stays in the hole. The cursor's anchor is read before the clock starts.
pub(crate) fn send_keys(window: &mut Window, ctx: &egui::Context) -> Vec<WindowRequest> {
    if header::menu::keys(window, ctx) {
        return Vec::new();
    }
    // **The phase goes back to the start whenever a key is pressed**, so the cursor is lit the
    // instant you type instead of at whatever point of the cycle you caught it — and typing
    // continuously never leaves it half-drawn. **It is asked BEFORE the keys are handed over**:
    // sending them consumes the frame's events, so read afterwards this always sees an empty
    // list and the anchor never moves. Measured — the test for it failed exactly that way.
    let typed = ctx.input(keyboard::somebody_typed);
    if typed {
        window.blink.saw_a_key();
    }
    let before = Instant::now();
    let started = window.frame_started;
    // The seed is taken out and put back because the pane and the field are two borrows of the
    // same owner, and the compiler does not let both be held at once.
    let mut modifiers = window.modifiers;
    // **The table is cloned, and it is cheap: at most one pair per `keyboard::Action`.**
    // The borrow is the same problem as the modifiers above —the pane and the field are two
    // borrows of the same owner— and here it is solved the same way, without leaving the table
    // somewhere nobody would look for it.
    let keymap = window.keymap.clone();
    let clipboard = keyboard::Clipboard::native(window.copy_enabled);
    let routing = Routing {
        keymap: &keymap,
        clipboard,
        started,
    };
    let has_navigation = ctx.input(|input| {
        let mut scan = modifiers;
        input
            .events
            .iter()
            .any(|event| is_navigation(event_action(event, &mut scan, &keymap)))
    });
    if !has_navigation {
        let tabs_open = window.tabs.len();
        let tab = window.active_mut();
        let requests = tab
            .where_the_focus_is()
            .map(|at| {
                tab.panes_mut()[at].send_keys(
                    ctx,
                    started,
                    &mut modifiers,
                    tabs_open,
                    &keymap,
                    clipboard,
                )
            })
            .unwrap_or_default();
        window.timed_separately += before.elapsed() * u32::from(typed);
        window.modifiers = modifiers;
        return requests;
    }
    let events = ctx.input_mut(|input| std::mem::take(&mut input.events));
    // Requests applied before the next keystroke can charge their own work (D13).
    // The enclosing keyboard interval already includes that time.
    let timed_before_navigation = window.timed_separately;
    let mut pending = Vec::new();
    let mut retained = Vec::new();
    let mut requests = Vec::new();
    let mut scan = modifiers;
    let mut drop_text = false;
    let mut drop_request = false;
    for event in events {
        if (drop_text && matches!(event, egui::Event::Text(_)))
            || (drop_request && matches!(event, egui::Event::Copy | egui::Event::Cut))
        {
            drop_text = false;
            drop_request = false;
            continue;
        }
        drop_text = false;
        drop_request = false;
        let action = event_action(&event, &mut scan, &keymap);
        let navigation = is_navigation(action);
        let tab_switch = matches!(
            action,
            Some(keyboard::Action::PreviousTab | keyboard::Action::NextTab)
        );
        if !navigation && !tab_switch {
            pending.push(event);
            continue;
        }
        let prior = send_chunk(
            window,
            ctx,
            &mut modifiers,
            &routing,
            &mut pending,
            &mut retained,
        );
        if !prior.is_empty() {
            let metrics = crate::painter::CellMetrics::measure(
                ctx,
                egui::FontId::monospace(window.font_size),
            );
            for request in prior {
                window.apply(request, ctx, ctx.content_rect(), &metrics);
            }
        }
        if tab_switch {
            if window.tabs.len() > 1 {
                drop_request =
                    clipboard.insert_chords && keyboard::request_behind(&event).is_some();
                drop_text = may_emit_text(&event, scan);
                pending.push(event);
                let changes = send_chunk(
                    window,
                    ctx,
                    &mut modifiers,
                    &routing,
                    &mut pending,
                    &mut retained,
                );
                let metrics = crate::painter::CellMetrics::measure(
                    ctx,
                    egui::FontId::monospace(window.font_size),
                );
                for request in changes {
                    window.apply(request, ctx, ctx.content_rect(), &metrics);
                }
            } else {
                pending.push(event);
            }
            continue;
        }
        drop_text = may_emit_text(&event, scan);
        drop_request = clipboard.insert_chords && keyboard::request_behind(&event).is_some();
        let repeat = matches!(event, egui::Event::Key { repeat: true, .. });
        if action == Some(keyboard::Action::MaximizePane) {
            if !repeat {
                let tab = window.active();
                let (name, focus, count) = (tab.name, tab.focus, tab.panes().len());
                window.pane_view.toggle(name, focus, count);
                ctx.request_repaint();
            }
        } else {
            let direction = match action {
                Some(keyboard::Action::FocusLeft) => navigation::Direction::Left,
                Some(keyboard::Action::FocusRight) => navigation::Direction::Right,
                Some(keyboard::Action::FocusUp) => navigation::Direction::Up,
                Some(keyboard::Action::FocusDown) => navigation::Direction::Down,
                _ => unreachable!(),
            };
            let tab = window.active();
            if window.pane_view.zoomed(tab.name).is_none() {
                let metrics = crate::painter::CellMetrics::measure(
                    ctx,
                    egui::FontId::monospace(window.font_size),
                );
                let area = crate::tab_bar::geometry(window, ctx.content_rect(), &metrics).consoles;
                let tab = window.active();
                let panes = tab.panes();
                let names: Vec<_> = panes.iter().map(|pane| pane.name).collect();
                if let Some(current) = names.iter().position(|name| *name == tab.focus) {
                    let rects = tab.root.place(area, metrics.width()).parts;
                    if let Some(next) = navigation::neighbor(&rects, current, direction) {
                        window.active_mut().focus = names[next];
                        ctx.request_repaint();
                    }
                }
            }
        }
    }
    requests.extend(send_chunk(
        window,
        ctx,
        &mut modifiers,
        &routing,
        &mut pending,
        &mut retained,
    ));
    ctx.input_mut(|input| input.events = retained);
    window.timed_separately = timed_before_navigation + before.elapsed();
    window.modifiers = modifiers;
    requests
}

fn send_chunk(
    window: &mut Window,
    ctx: &egui::Context,
    modifiers: &mut egui::Modifiers,
    routing: &Routing<'_>,
    pending: &mut Vec<egui::Event>,
    retained: &mut Vec<egui::Event>,
) -> Vec<WindowRequest> {
    ctx.input_mut(|input| input.events = std::mem::take(pending));
    let tabs_open = window.tabs.len();
    let tab = window.active_mut();
    // **The focused one is looked up by its name**, and if it does not turn up nobody is typed
    // to: writing into «the first one there is» would send the keys to a program the user is
    // not looking at, which is worse than losing them.
    let at = tab.where_the_focus_is();
    let requests = at
        .map(|at| {
            tab.panes_mut()[at].send_keys(
                ctx,
                routing.started,
                modifiers,
                tabs_open,
                routing.keymap,
                routing.clipboard,
            )
        })
        .unwrap_or_default();
    retained.extend(ctx.input_mut(|input| std::mem::take(&mut input.events)));
    requests
}

/// What the window does to a batch before egui reads it: the injectors of the measured runs, and
/// on Windows the clipboard request behind Ctrl+Insert and Shift+Delete (**D115**,
/// [`keyboard::add_requests_behind`]). The platform is asked first, so a batch elsewhere is not
/// even walked.
pub(crate) fn before_egui(
    window: &mut Window,
    raw_input: &mut egui::RawInput,
    ctx: &egui::Context,
) {
    if cfg!(target_os = "windows") {
        keyboard::add_requests_behind(&mut raw_input.events);
    }
    crate::settings::editor::inject(window, raw_input);
    crate::board::probe::inject(window, raw_input, ctx);
    crate::sidebar::probe::inject(window, raw_input, ctx);
}

impl Window {
    pub(crate) fn send_keys_to_the_focused(&mut self, ctx: &egui::Context) -> Vec<WindowRequest> {
        send_keys(self, ctx)
    }
}
