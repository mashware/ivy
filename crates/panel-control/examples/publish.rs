//! Publishes to a pane over its channel, for the measurement bench.
//!
//! **It exists because batch `S` cannot be seeded by writing a file**, which is how every other
//! batch publishes: what that batch measures is the path that does *not* touch the disk, so
//! something has to hold a connection open and publish down it while the window is being measured.
//!
//! **The document comes in as an argument rather than being written here**, and that is the same
//! rule the script states for its own batch table: a batch is defined in one place. What this adds
//! is the turn number, substituted into the literal `{n}`, so a run's publications are told apart on
//! screen exactly as the file-seeded ones are.
//!
//! **And it reads the document back where the batch asks it to**, which is the other half of the same
//! argument: the reading batch measures what answering a read costs, and nothing else in the bench
//! ever asks for one.
//!
//! **The reading batch publishes AND reads every turn, and that is what makes its control honest.**
//! The first shape had it publish once and then only read, which looks tighter and is not: a
//! publication drops the panel's layout cache and changes the text, so the batch it is compared
//! against pays a re-layout and a repaint every second that the reading batch would not — and the
//! subtraction would come out as the cost of a read *minus* all of that, which can be zero or
//! negative and would be published as «answering a read costs nothing». Publishing the same document
//! on the same cadence and asking for it back makes the reading batch its control **plus one read**,
//! which is the only shape whose difference is the thing being measured.
//!
//! ```text
//! publish <directory> <identifier> <source> <how many> <every so many ms> <document> [publish|read]
//! ```

use std::time::Duration;

use panel_control::{Answered, Producer};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let ([dir, id, source, how_many, every_ms, document], verb) = match &asked[..] {
        [dir, id, source, how_many, every_ms, document] => (
            [dir, id, source, how_many, every_ms, document],
            "publish".to_owned(),
        ),
        [dir, id, source, how_many, every_ms, document, verb] => (
            [dir, id, source, how_many, every_ms, document],
            verb.clone(),
        ),
        _ => {
            eprintln!(
                "publish <directory> <identifier> <source> <how many> <every so many ms> <document> [publish|read]"
            );
            std::process::exit(2);
        }
    };
    // **A verb nobody serves ends the process instead of falling back to publishing**, because a
    // silent fallback is how a batch measures the load of the batch beside it under its own name.
    if verb != "publish" && verb != "read" {
        eprintln!("publish: the verb is \"publish\" or \"read\", not \"{verb}\"");
        std::process::exit(2);
    }
    let reading = verb == "read";
    let how_many: usize = how_many.parse().expect("how many is a number");
    let every = Duration::from_millis(every_ms.parse().expect("the interval is a number"));

    // **One connection for the whole run, and that is the point of the batch.** A connection per
    // publication would claim and release the pane every turn, which measures the holder's churn
    // rather than the ordinary path an agent takes.
    let mut pane = match Producer::of(std::path::Path::new(dir), id) {
        Ok(pane) => pane,
        Err(failure) => {
            eprintln!("publish: {dir} is not listening ({failure})");
            std::process::exit(1);
        }
    };

    for turn in 1..=how_many {
        let document = document.replace("{n}", &turn.to_string());
        // **The publication happens in both batches**, so the one that reads is the other one plus a
        // read rather than a different load wearing its name.
        let published = pane.publish(source, &document);
        let answered = if reading {
            report("publish", published);
            pane.read_the_document()
        } else {
            published
        };
        match answered {
            Ok(Answered::Ok) => {}
            // What a read comes back with, **and only the batch that asked for one may see it**: a
            // document answered to a publication is the pane answering the wrong verb, which is the
            // case the arm below exists to report and not one to pass over in silence.
            Ok(Answered::Document(_)) if reading => {}
            // **Said and not fatal.** A refusal is a real answer — another source holds the pane —
            // and the run's own report is what decides whether the batch is valid, not this process.
            Ok(Answered::Failed(why)) => eprintln!("publish: refused ({why})"),
            // **The three `split_*` verbs are the other half of the channel and this example never
            // asks for them.** A payload here would mean the pane answered the wrong request, which
            // is worth a line and not a panic: this process's job is the load, and the batch's own
            // report is what decides whether the run counts.
            Ok(Answered::Opened(_) | Answered::Text(_) | Answered::Document(_)) => {
                eprintln!("publish: the pane answered something else to a publication");
            }
            Err(failure) => {
                eprintln!("publish: the channel is gone ({failure})");
                return;
            }
        }
        std::thread::sleep(every);
    }
}

/// Says what an answer was, when nothing else is going to look at it.
///
/// **Its `Err` arm is the reason it exists.** The publication the reading batch makes every turn is
/// not the one whose answer that batch inspects, and dropping it silently is how a run passes its own
/// guard having never published over the channel at all: the file seeding writes `panel.json` too, so
/// the reads succeed on a document that arrived the other way and the batch reports a channel it
/// never used.
fn report(verb: &str, answered: std::io::Result<Answered>) {
    match answered {
        Ok(Answered::Ok) => {}
        Ok(Answered::Failed(why)) => eprintln!("{verb}: refused ({why})"),
        Ok(_) => eprintln!("{verb}: the pane answered something else"),
        Err(failure) => eprintln!("{verb}: the channel is gone ({failure})"),
    }
}
