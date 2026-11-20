//! How a header's controls leave once nobody wants them (D99): they stay whole for
//! a while and then fade out, instead of vanishing on the pass that stopped wanting them.

use eframe::egui;

/// Seconds the controls stay whole after they stop being wanted.
const LINGER: f64 = 3.0;
/// Seconds they take to fade out after that.
const FADE: f64 = 0.5;
/// Below this opacity a fading control is painted but no longer answers: an icon a quarter
/// opaque or less is one nobody aims at, and D99 keeps presses for controls that were seen.
pub(crate) const PRESSABLE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// The pass with this frame number painted the header and wanted its controls.
    Wanted(u64),
    /// The input time of the first pass that found them no longer wanted.
    LeftAt(f64),
}

#[derive(Debug, PartialEq)]
enum Wake {
    Not,
    After(f64),
    Next,
}

/// One pass of the countdown.
#[derive(Debug, PartialEq)]
struct Step {
    keep: Option<State>,
    opacity: f32,
    wake: Wake,
}

const GONE: Step = Step {
    keep: None,
    opacity: 0.0,
    wake: Wake::Not,
};

/// **The countdown starts on the first pass that finds the controls unwanted**, never at the last
/// pass that found them wanted: a window at rest paints nothing, so «last wanted» can be seconds old
/// when the pointer finally leaves, and the controls would vanish on the spot. A `Wanted` older
/// than the previous pass means the header went unpainted in between (a tab in the background, a
/// sibling behind a maximized console): nobody saw it leave, so nothing lingers.
fn step(previous: Option<State>, wanted: bool, now: f64, frame: u64, fades: bool) -> Step {
    if wanted {
        return Step {
            keep: Some(State::Wanted(frame)),
            opacity: 1.0,
            wake: Wake::Not,
        };
    }
    if !fades {
        return GONE;
    }
    let left = match previous {
        Some(State::Wanted(painted)) if painted + 1 == frame => now,
        Some(State::LeftAt(left)) => left,
        _ => return GONE,
    };
    let elapsed = now - left;
    if elapsed < LINGER {
        Step {
            keep: Some(State::LeftAt(left)),
            opacity: 1.0,
            wake: Wake::After(LINGER - elapsed),
        }
    } else if elapsed < LINGER + FADE {
        Step {
            keep: Some(State::LeftAt(left)),
            opacity: (1.0 - (elapsed - LINGER) / FADE) as f32,
            wake: Wake::Next,
        }
    } else {
        GONE
    }
}

/// How opaque console `name`'s header controls are on this pass, 0 meaning not painted at all —
/// and, while they wait or fade, the repaint that carries them to their next step.
///
/// Off where fades are off (`pane_view::focus_fades`, the switch every timed animation of the
/// chrome shares): a measured run publishes frame cost by subtraction, and the frames a wait or a
/// fade wakes would be published as the framework's (D13).
pub(crate) fn controls_opacity(ctx: &egui::Context, name: usize, wanted: bool) -> f32 {
    let id = egui::Id::new(("integrated-header-linger", name));
    let now = ctx.input(|input| input.time);
    let frame = ctx.cumulative_frame_nr();
    let fades = crate::pane_view::focus_fades(ctx);
    let step = ctx.data_mut(|data| {
        let step = step(data.get_temp::<State>(id), wanted, now, frame, fades);
        match step.keep {
            Some(state) => {
                data.insert_temp(id, state);
            }
            None => data.remove::<State>(id),
        }
        step
    });
    match step.wake {
        Wake::Not => {}
        Wake::After(seconds) => {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(seconds))
        }
        Wake::Next => ctx.request_repaint(),
    }
    step.opacity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_at(seconds: f64) -> Step {
        step(Some(State::LeftAt(0.0)), false, seconds, 9, true)
    }

    #[test]
    fn wanted_controls_are_whole_and_wake_nothing() {
        for previous in [None, Some(State::Wanted(3)), Some(State::LeftAt(0.0))] {
            assert_eq!(
                step(previous, true, 3.25, 9, true),
                Step {
                    keep: Some(State::Wanted(9)),
                    opacity: 1.0,
                    wake: Wake::Not
                }
            );
        }
    }

    #[test]
    fn the_countdown_starts_on_the_first_unwanted_pass_however_long_the_window_slept() {
        // Wanted on the previous pass, 20 s ago by the clock: the leave is now, not then.
        assert_eq!(
            step(Some(State::Wanted(8)), false, 20.0, 9, true),
            Step {
                keep: Some(State::LeftAt(20.0)),
                opacity: 1.0,
                wake: Wake::After(LINGER)
            }
        );
    }

    #[test]
    fn a_header_that_went_unpainted_does_not_linger_when_it_comes_back() {
        assert_eq!(step(Some(State::Wanted(5)), false, 20.0, 9, true), GONE);
    }

    #[test]
    fn they_stay_whole_then_fade_then_go_and_stop_waking() {
        let late = left_at(2.9);
        assert_eq!(late.opacity, 1.0);
        assert!(matches!(late.wake, Wake::After(s) if (s - 0.1).abs() < 1e-9));
        let half = left_at(3.25);
        assert_eq!(half.keep, Some(State::LeftAt(0.0)));
        assert!((half.opacity - 0.5).abs() < 1e-6, "{}", half.opacity);
        assert_eq!(half.wake, Wake::Next);
        assert_eq!(left_at(3.5), GONE);
        assert_eq!(left_at(10.0), GONE);
    }

    #[test]
    fn never_shown_or_without_fades_they_are_simply_gone() {
        assert_eq!(step(None, false, 1.0, 9, true), GONE);
        assert_eq!(step(Some(State::Wanted(8)), false, 1.0, 9, false), GONE);
    }
}
