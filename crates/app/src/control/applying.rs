//! Apply pane-scoped deliveries without changing panel ownership for agent metadata.
use super::*;
impl PaneControl {
    /// Applies everything waiting, and answers whoever is waiting for an answer.
    ///
    /// **The queue is bounded at [`MAX_PENDING`], and the earlier shape that left it unbounded was
    /// wrong for a reason review found.** The argument for leaving it open was that a pane serves a
    /// fixed, small number of connections and each one blocks on its own reply, so what is ordinarily
    /// waiting is one request per connection. What that missed is a caller that *gave up*: its thread
    /// goes back to reading with its delivery still queued, and a window that is minimised runs no
    /// pass to drain it — so a producer parked against it piled up one document per connection per
    /// timeout, unbounded. With `sync_channel` the send applies backpressure (or the listener refuses,
    /// see its `publish`), and memory stays capped.
    ///
    /// **A caller that gave up still gets its document applied when a pass runs**, and the wording on
    /// the other side is what makes that honest: the reply a timed-out caller reads says the window
    /// did not answer in time, never that nothing was published.
    pub fn apply(
        &mut self,
        state: &mut PanelState,
        signals: &mut crate::agent_signals::Signals,
        now: SystemTime,
    ) {
        while let Ok(delivery) = self.inbox.try_recv() {
            match delivery.what {
                What::Overlay {
                    panel,
                    holder,
                    incarnation,
                    authority,
                } => {
                    if state.accepts_overlay(holder, incarnation)
                        && authority.current()
                        && state.apply_overlay(panel, now).is_ok()
                    {
                        self.did.overlaid += 1;
                    } else {
                        self.did.overlays_refused += 1;
                        crate::panel::overlay::refused_at_apply(state);
                    }
                }

                What::PublishAttested {
                    source,
                    panel,
                    peer,
                } => {
                    let answer = match state.deliver_observed(
                        delivery.connection,
                        &source,
                        panel,
                        now,
                        delivery.origin,
                    ) {
                        Ok(()) => {
                            if let Some(authority) = &self.authority {
                                authority.capture(state, delivery.connection, &peer);
                            }
                            self.did.applied += 1;
                            Answer::Taken
                        }
                        Err(why) => {
                            self.did.refused += 1;
                            Answer::Refused(why)
                        }
                    };
                    let _ = delivery.answer.send(answer);
                }

                What::AgentSignal(snapshot) => {
                    let answer = signals
                        .receive(delivery.connection, snapshot, std::time::Instant::now())
                        .map_or_else(Answer::Refused, |_| Answer::Taken);
                    let _ = delivery.answer.send(answer);
                }
                What::Publish { source, panel } => {
                    let answer = match state.deliver_observed(
                        delivery.connection,
                        &source,
                        panel,
                        now,
                        delivery.origin,
                    ) {
                        Ok(()) => {
                            self.did.applied += 1;
                            Answer::Taken
                        }
                        Err(why) => {
                            self.did.refused += 1;
                            Answer::Refused(why)
                        }
                    };
                    // The send cannot block — the answer channel takes one value — and it fails only
                    // when the caller is already gone, which changes nothing about what is on screen.
                    let _ = delivery.answer.send(answer);
                }
                What::Get => {
                    let answer = match state.document() {
                        Ok(document) => {
                            self.reads.succeeded(document.len());
                            Answer::Document(document)
                        }
                        Err(why) => {
                            self.reads.refused();
                            Answer::Refused(why)
                        }
                    };
                    let _ = delivery.answer.send(answer);
                }
                // **The patch lands beside the publication and not beside the reads**, because it
                // is a write: it needs the hold, it stamps the date, and what it leaves behind is
                // what the next painting draws.
                What::Patch { ops } => {
                    let answer = match state.patch(delivery.connection, &ops, now) {
                        Ok(written) => {
                            self.did.patched += 1;
                            // **The size of what the patch left behind, not of the ops that
                            // arrived**, and it comes back from the ceiling check rather than from a
                            // second write: the document a following read would hand over is the one
                            // whose size explains this path's cost.
                            self.did.largest_serialized = self.did.largest_serialized.max(written);
                            Answer::Taken
                        }
                        Err(why) => {
                            self.did.patches_refused += 1;
                            Answer::Refused(why)
                        }
                    };
                    let _ = delivery.answer.send(answer);
                }
                What::Closed => {
                    signals.closed(delivery.connection, std::time::Instant::now());
                    self.apply_closed(state, delivery);
                }
                // **The three `split_*` verbs are the window's, and a pane has no tree.** They are
                // held here until the window's drain takes them, which is where the handles, the
                // focus and the revocation live — and the window is woken for it, because the drain
                // of this very pass has already run: without that, a request arriving while the
                // window is at rest would be answered by no pass at all.
                What::Retire => {
                    self.defer_retirement(delivery);
                    break;
                }
                What::Open { .. }
                | What::Read { .. }
                | What::Close { .. }
                | What::PlanSet { .. }
                | What::PlanGet
                | What::PlanRetire => {
                    self.waiting_on_the_window.push(delivery);
                    (self.wake)();
                }
            }
        }
    }
}
