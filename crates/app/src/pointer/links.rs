//! Which presses are link gestures, and what taking one does. **D51**.
//!
//! The panel owns a bare click on a link label and the terminal owns `Ctrl`+click. A press taken
//! here leaves the queue before the buttons are routed to consoles, panels and programs — only a
//! press that answers a standing question goes first — and the gesture keeps the pane until its
//! release so a small movement is not read as a selection, a pane move or a program's mouse report.

use eframe::egui;

use super::ButtonPress;
use crate::Window;
use crate::injected::InjectedGesture;
use crate::links::{Origin, WebTarget, opens_a_console_link};
use crate::painter::CellMetrics;

/// A link press that owns its pane's primary button until the frame after its release.
///
/// **It remembers the frame it was opened in**, because a frame's queue can carry the release of an
/// earlier click before this press: a release is only this gesture's when it comes after the press,
/// and for the frame it was opened in the queue's order already said whether it did.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LinkGesture {
    pane: usize,
    opened_in: u64,
    released_in: Option<u64>,
}

/// Whether the queue carries a primary-button event going the given way.
pub(super) fn has_primary(queue: &[ButtonPress], pressed: bool) -> bool {
    queue
        .iter()
        .any(|button| button.pressed == pressed && button.button == egui::PointerButton::Primary)
}

fn end_link_gesture(gesture: &mut Option<LinkGesture>, frame: u64) {
    if gesture.is_some_and(|gesture| gesture.released_in.is_some_and(|at| at != frame)) {
        *gesture = None;
    }
}

fn open_local_press(
    window: &mut Window,
    metrics: &CellMetrics,
    frame: u64,
    queue: &mut Vec<ButtonPress>,
    which: usize,
) -> bool {
    let button = queue[which];
    let local = window.active().panes().into_iter().find_map(|pane| {
        let painted = pane.painted_console.as_ref()?;
        painted
            .local_at(button.at, metrics, frame)
            .map(|target| (pane.name, target))
    });
    let Some((pane, target)) = local else {
        return false;
    };
    crate::viewer::open_markdown_target(window, pane, target.path, target.line);
    let released = has_primary(&queue[which + 1..], false);
    queue.remove(which);
    window.link_press_in_progress = Some(LinkGesture {
        pane,
        opened_in: frame,
        released_in: released.then_some(frame),
    });
    window.the_press_went_to_the_program = false;
    true
}

impl Window {
    /// Whether the bench is running the link gesture, which suppresses the desktop hand-off.
    pub(crate) fn measuring_links(&self) -> bool {
        self.injected_drags
            .as_ref()
            .is_some_and(|injected| injected.which == InjectedGesture::ActivateLinks)
    }

    /// Whether a link gesture owns this pane's primary button, which keeps focus, selection and
    /// dragging from reading it too.
    pub(crate) fn a_link_gesture_holds(&self, pane: usize) -> bool {
        self.link_press_in_progress
            .is_some_and(|gesture| gesture.pane == pane)
    }

    /// Lets go of a link gesture whose release belonged to an earlier frame.
    pub(super) fn a_link_gesture_ended(&mut self, frame: u64) {
        end_link_gesture(&mut self.link_press_in_progress, frame);
    }

    /// Marks the link gesture in progress as released in this frame, unless it was opened in it:
    /// then [`Self::open_link_presses`] already read whether the release came after its press.
    pub(super) fn a_link_gesture_was_released(&mut self, released: bool, frame: u64) {
        if let Some(gesture) = self.link_press_in_progress.as_mut()
            && released
            && gesture.opened_in != frame
        {
            gesture.released_in = Some(frame);
        }
    }

    /// Opens every supported link press in this frame and removes only those presses from routing.
    ///
    /// Both surfaces resolve against retained paint state: no hover is needed, and output or a
    /// publication arriving after paint cannot change the destination underneath an already
    /// visible label.
    pub(super) fn open_link_presses(
        &mut self,
        metrics: &CellMetrics,
        frame: u64,
        mut queue: Vec<ButtonPress>,
    ) -> Vec<ButtonPress> {
        if has_primary(&queue, true) {
            self.link_press_in_progress = None;
        }
        let mut which = 0;
        while which < queue.len() {
            let button = queue[which];
            if !button.pressed || button.button != egui::PointerButton::Primary {
                which += 1;
                continue;
            }

            if !button.chord.is_none() && !opens_a_console_link(button.chord) {
                which += 1;
                continue;
            }

            if opens_a_console_link(button.chord)
                && open_local_press(self, metrics, frame, &mut queue, which)
            {
                continue;
            }

            // **The console's snapshot gates the panel's click too**: it is what says this pane —
            // its panel included — was on screen in the frame the press answers, which a panel's
            // own `settled` does not, since a pane that stopped being painted keeps its last one.
            let found = self.active().panes().into_iter().find_map(|pane| {
                let painted = pane
                    .painted_console
                    .as_ref()
                    .filter(|painted| painted.is_current(frame))?;
                let (target, origin) = if button.chord.is_none() {
                    pane.panel_state
                        .link_at(button.at)
                        .map(|target| (target.to_owned(), Origin::Panel))
                } else {
                    painted
                        .link_at(button.at, metrics, frame)
                        .map(|link| (link.target, link.source.into()))
                }?;
                Some((pane.name, target, origin))
            });
            let Some((pane, target, origin)) = found else {
                which += 1;
                continue;
            };
            let Some(target) = WebTarget::parse(&target) else {
                // An invalid producer value is not a link gesture. It keeps following the same
                // routing rules the visible text or panel area had before this feature existed.
                which += 1;
                continue;
            };

            match origin {
                Origin::Plain => self.links.plain += 1,
                Origin::Osc8 => self.links.osc8 += 1,
                Origin::Panel => self.links.panel += 1,
            }
            if !self.measuring_links() {
                self.desktop.open(&target);
            } else if let Some(injected) = self.injected_drags.as_mut() {
                injected.one_more_with_effect();
            }
            let released = has_primary(&queue[which + 1..], false);
            queue.remove(which);
            self.link_press_in_progress = Some(LinkGesture {
                pane,
                opened_in: frame,
                released_in: released.then_some(frame),
            });
            self.the_press_went_to_the_program = false;
        }
        queue
    }
}
