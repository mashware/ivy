//! Resolve the explicitly configured family before the first frame.
use crate::measure;
use eframe::egui;
use std::time::Instant;

/// Hands the text engine the family the settings ask for, and says what the report has to publish.
///
/// **Nothing is asked of the machine when they ask for nothing, and that is the whole of D24.**
/// Resolving a name forces enumerating every font installed —between 17.3 and 23.2 ms— which is the
/// cost D24 decided never to pay at start-up. The `None` branch here is what keeps that decision
/// standing, and it is the branch every start-up without a settings file takes.
///
/// **It is a function of its own and not four lines inside the window's constructor**, because
/// inside that closure no test can reach it: the laziness would rest on reading the code.
pub(crate) fn choose_the_font(
    family: Option<&str>,
    discoverer: &mut super::Discoverer,
    ctx: &egui::Context,
) -> measure::ChoosingTheFont {
    let Some(asked_for) = family else {
        return measure::ChoosingTheFont::NotAsked;
    };
    let started = Instant::now();
    let chosen = discoverer.use_the_family(asked_for, ctx);
    let how_long = started.elapsed();
    let asked_for = asked_for.to_owned();
    match chosen {
        super::Chosen::Registered(family) => measure::ChoosingTheFont::Found { family, how_long },
        // **Each one is said with its own cause, and that is not politeness.** From outside the three
        // look identical —the window opens exactly as always— and each sends whoever reads it
        // somewhere different: to fix what they typed, to fix what is installed, or nowhere at all.
        super::Chosen::NotInstalled => {
            eprintln!(
                "agent-terminal: the font '{asked_for}' is not installed on this machine. Painting \
                 with the built-in ones."
            );
            measure::ChoosingTheFont::Missing {
                asked_for,
                how_long,
            }
        }
        super::Chosen::Unusable => {
            eprintln!(
                "agent-terminal: the font '{asked_for}' is installed and none of its files can be \
                 read. Painting with the built-in ones."
            );
            measure::ChoosingTheFont::Unusable {
                asked_for,
                how_long,
            }
        }
        // **Nothing is said here**: whoever found out there are no system fonts already said so, and
        // a second line blaming the name would send the reader to fix a name that is fine.
        super::Chosen::NoFontsOnTheMachine => measure::ChoosingTheFont::Missing {
            asked_for,
            how_long,
        },
    }
}
