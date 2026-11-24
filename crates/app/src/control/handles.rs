//! What a connection does with the consoles it opened: reading them and closing them, by handle.
//!
//! Free functions over the window rather than methods: `impl Window` is a D28 ratchet.

use eframe::egui;
use panel_control::Answer;

use crate::Window;

/// The console a handle names, **if this connection opened it and the person has not taken it
/// over**, with the refusal as the caller is to read it.
///
/// **One place resolves a handle**, so reading and closing cannot drift into answering
/// differently about the same console: a revoked handle answers with the reason and not with a
/// bare error (**AC15**), which is what lets an agent tell «the person took it» from «you never
/// had it».
fn a_console_this_connection_owns(
    window: &mut Window,
    origin: usize,
    connection: u64,
    handle: &str,
) -> Result<usize, String> {
    let pane_name = window
        .opened_by_agents
        .reachable(origin, connection, handle)?;
    let Some(pane) = window.all_panes().find(|pane| pane.name == pane_name) else {
        return Err("that console is gone".to_owned());
    };
    match &pane.taken_over {
        Some(why) => Err(why.clone()),
        None => Ok(pane_name),
    }
}

/// What a console this connection opened is showing.
pub(super) fn reading_for_a_connection(
    window: &mut Window,
    origin: usize,
    connection: u64,
    handle: &str,
    scrollback: bool,
) -> Answer {
    let pane_name = match a_console_this_connection_owns(window, origin, connection, handle) {
        Ok(name) => name,
        Err(why) => return Answer::Refused(why),
    };
    let Some(pane) = window.all_panes().find(|pane| pane.name == pane_name) else {
        return Answer::Refused("that console is gone".to_owned());
    };
    // **The visible screen unless the scrollback was asked for**, which is the contract's default
    // and not a policy: the small read is what an agent usually wants, and the whole history is a
    // lot of bytes to hand over by accident.
    let text = if scrollback {
        pane.screen.scrollback_text()
    } else {
        pane.screen.to_text()
    };
    Answer::Text(text)
}

/// Closes a console this connection opened.
pub(super) fn closing_for_a_connection(
    window: &mut Window,
    origin: usize,
    connection: u64,
    handle: &str,
    ctx: &egui::Context,
) -> Answer {
    let pane_name = match a_console_this_connection_owns(window, origin, connection, handle) {
        Ok(name) => name,
        Err(why) => return Answer::Refused(why),
    };
    match window.close_a_pane_called(pane_name, ctx) {
        Ok(()) => {
            window.opened_by_agents.forget(pane_name);
            Answer::Taken
        }
        Err(why) => Answer::Refused(why),
    }
}
