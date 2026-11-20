//! How a header's controls leave once nobody wants them (D99): they stay whole for
//! a while and then fade out, instead of vanishing on the pass that stopped wanting them.

use eframe::egui;

/// Seconds the controls stay whole after they stop being wanted.
const LINGER: f64 = 3.0;
/// Seconds they take to fade out after that.
const FADE: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// The last pass that painted this header wanted its controls.
    Wanted,
    /// The input time of the first pass that found them no longer wanted.
    LeftAt(f64),
}

#[derive(Debug, PartialEq)]
enum Wake {
    Not,
    After(f64),
    Next,
}

/// One pass of the countdown: the state to keep, the opacity to paint with, and when to wake.
///
/// **The countdown starts on the first pass that finds the controls unwanted**, never at the last
/// pass that found them wanted: a window at rest paints nothing, so «last wanted» can be seconds old
/// when the pointer finally leaves, and the controls would vanish on the spot.
fn step(previous: Option<State>, wanted: bool, now: f64, fades: bool) -> (Option<State>, f32, Wake) {
    if wanted {
        return (Some(State::Wanted), 1.0, Wake::Not);
    }
    let left = match previous {
        _ if !fades => return (None, 0.0, Wake::Not),
        None => return (None, 0.0, Wake::Not),
        Some(State::Wanted) => now,
        Some(State::LeftAt(left)) => left,
    };
    let elapsed = (now - left).max(0.0);
    if elapsed < LINGER {
        (Some(State::LeftAt(left)), 1.0, Wake::After(LINGER - elapsed))
    } else if elapsed < LINGER + FADE {
        let opacity = 1.0 - (elapsed - LINGER) / FADE;
        (Some(State::LeftAt(left)), opacity as f32, Wake::Next)
    } else {
        (None, 0.0, Wake::Not)
    }
}

/// How opaque console `name`'s header controls are on this pass, 0 meaning not painted at all.
///
/// Off where fades are off (`pane_view::focus_fades`): a measured run publishes frame cost by
/// subtraction, and the frames a wait or a fade wakes would be published as the framework's (D13).
pub(crate) fn controls_opacity(ctx: &egui::Context, name: usize, wanted: bool) -> f32 {
    let id = egui::Id::new(("integrated-header-linger", name));
    let now = ctx.input(|input| input.time);
    let fades = crate::pane_view::focus_fades(ctx);
    let previous = ctx.data(|data| data.get_temp::<State>(id));
    let (state, opacity, wake) = step(previous, wanted, now, fades);
    if state != previous {
        ctx.data_mut(|data| match state {
            Some(state) => {
                data.insert_temp(id, state);
            }
            None => data.remove::<State>(id),
        });
    }
    match wake {
        Wake::Not => {}
        Wake::After(seconds) => ctx.request_repaint_after(std::time::Duration::from_secs_f64(seconds)),
        Wake::Next => ctx.request_repaint(),
    }
    opacity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_at(seconds: f64) -> (Option<State>, f32, Wake) {
        step(Some(State::LeftAt(0.0)), false, seconds, true)
    }

    #[test]
    fn wanted_controls_are_whole_and_wake_nothing() {
        for previous in [None, Some(State::Wanted), Some(State::LeftAt(0.0))] {
            assert_eq!(
                step(previous, true, 3.25, true),
                (Some(State::Wanted), 1.0, Wake::Not)
            );
        }
    }

    #[test]
    fn the_countdown_starts_on_the_first_unwanted_pass_however_long_the_window_slept() {
        // Wanted since a pass 20 s ago, nothing painted in between: the leave is now, not then.
        assert_eq!(
            step(Some(State::Wanted), false, 20.0, true),
            (Some(State::LeftAt(20.0)), 1.0, Wake::After(LINGER))
        );
    }

    #[test]
    fn they_stay_whole_then_fade_then_go_and_stop_waking() {
        assert_eq!(left_at(2.9).1, 1.0);
        assert!(matches!(left_at(2.9).2, Wake::After(s) if (s - 0.1).abs() < 1e-9));
        let (state, half, wake) = left_at(3.25);
        assert_eq!(state, Some(State::LeftAt(0.0)));
        assert!((half - 0.5).abs() < 1e-6, "{half}");
        assert_eq!(wake, Wake::Next);
        assert_eq!(left_at(3.5), (None, 0.0, Wake::Not));
        assert_eq!(left_at(10.0), (None, 0.0, Wake::Not));
    }

    #[test]
    fn never_shown_or_without_fades_they_are_simply_gone() {
        assert_eq!(step(None, false, 1.0, true), (None, 0.0, Wake::Not));
        assert_eq!(
            step(Some(State::Wanted), false, 1.0, false),
            (None, 0.0, Wake::Not)
        );
    }
}
