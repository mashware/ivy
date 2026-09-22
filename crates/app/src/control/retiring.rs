//! The one window transition that intentionally ends a producer session.

use eframe::egui;
use panel_control::{Answer, Delivery};

use crate::Window;
use crate::control::PaneControl;
use crate::panel::{PanelState, Released, Retirement};

impl PaneControl {
    pub(super) fn apply_closed(&mut self, state: &mut PanelState, delivery: Delivery) {
        if state.release(delivery.connection) == Released::Given {
            self.did.released += 1;
        }
        self.waiting_on_the_window.push(delivery);
        (self.wake)();
    }

    pub(super) fn defer_retirement(&mut self, delivery: Delivery) {
        self.waiting_on_the_window.push(delivery);
        (self.wake)();
    }
}

impl Window {
    pub(super) fn reap_one_retired(&mut self) {
        let Some(retired) = self.control.next_retired() else {
            return;
        };
        let reaping = std::time::Instant::now();
        drop(retired);
        self.control.reaped(reaping.elapsed());
    }

    pub(super) fn retiring_for_a_connection(
        &mut self,
        origin: usize,
        connection: u64,
        ctx: &egui::Context,
    ) -> Answer {
        let started = std::time::Instant::now();
        let Some(origin_pane) = self.all_panes_mut().find(|pane| pane.name == origin) else {
            return Answer::Refused("the console that asked is gone".to_owned());
        };
        match origin_pane.panel_state.retirement_status(connection) {
            Ok(Some(result)) => {
                return Answer::Retired {
                    closed: result.closed,
                    preserved: result.preserved,
                };
            }
            Ok(None) => {}
            Err(why) => return Answer::Refused(why),
        }
        let Some(dir) = origin_pane.panel.as_ref() else {
            return Answer::Refused("this console has no panel identity".to_owned());
        };
        let baseline = match PanelState::retirement_baseline(dir.path()) {
            Ok(baseline) => baseline,
            Err(failure) => {
                return Answer::Refused(format!(
                    "the panel file generation could not be recorded ({failure})"
                ));
            }
        };

        let owned = self
            .opened_by_agents
            .panes_for_connection(origin, connection);
        let mut closing = Vec::new();
        let mut preserved = 0;
        for pane_name in owned {
            let Some(pane) = self.all_panes().find(|pane| pane.name == pane_name) else {
                return Answer::Refused("a console owned by this connection is gone".to_owned());
            };
            if pane.taken_over.is_some() {
                preserved += 1;
            } else {
                closing.push(pane_name);
            }
        }
        if let Err(why) = self.validate_detaching_panes(&closing) {
            return Answer::Refused(why);
        }

        let result = Retirement {
            closed: closing.len(),
            preserved,
        };
        let origin_pane = self
            .all_panes_mut()
            .find(|pane| pane.name == origin)
            .expect("the validated retirement keeps its origin console");
        origin_pane
            .panel_state
            .retire(connection, baseline, result)
            .expect("retirement authority was verified in the same interface pass");
        let detached = self
            .detach_panes_called(&closing, ctx)
            .expect("every retirement target was validated in the same interface pass");
        self.opened_by_agents
            .forget_the_connection(origin, connection);
        self.control.queue_retired(detached);
        self.control.retired(result, started.elapsed());

        Answer::Retired {
            closed: result.closed,
            preserved: result.preserved,
        }
    }
}
