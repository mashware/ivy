//! Publishes to a pane over its channel, for the measurement bench.
//!
//! **It exists because batch `S` cannot be seeded by writing a file**, which is how every other
//! batch publishes: what that batch measures is the path that does *not* touch the disk, so
//! something has to hold a connection open and publish down it while the window is being measured.
//!
//! **The document comes in on standard input rather than being written here**, and that is the same
//! rule the script states for its own batch table: a batch is defined in one place. What this adds
//! is the turn number, substituted into the literal `{n}`, so a run's publications are told apart on
//! screen exactly as the file-seeded ones are.
//!
//! **Standard input and not an argument**, because Linux refuses any single argument longer than
//! `MAX_ARG_STRLEN` — 32 pages, 128 KiB with 4 KiB pages — with `E2BIG`, and the large pair
//! publishes a panel near the contract's 256 KiB ceiling, about twice that. Every channel batch
//! takes this one path, so the large pair and its neighbours load their document the same way.
//!
//! **And it reads the document back, or patches it, where the batch asks it to**, which is the other
//! half of the same argument: those batches measure what answering each of those verbs costs, and
//! nothing else in the bench ever asks for one.
//!
//! **The reading and patching batches publish AND then ask, every turn, and that is what makes their
//! control honest.** The first shape had the reader publish once and then only read, which looks
//! tighter and is not: a publication drops the panel's layout cache and changes the text, so the
//! batch it is compared against pays a re-layout and a repaint every second that the reading batch
//! would not — and the subtraction would come out as the cost of a read *minus* all of that, which
//! can be zero or negative and would be published as «answering a read costs nothing». Publishing
//! the same document on the same cadence and then asking makes each of them its control **plus one
//! verb**, which is the only shape whose difference is the thing being measured.
//!
//! **The patching batch's ops come in as an argument**, and for the reason the document is handed in:
//! a load written here is a load the batch table does not declare, and the table is where somebody
//! reads what a run did.
//!
//! ```text
//! publish <directory> <identifier> <source> <how many> <every so many ms>
//!         [publish|read|patch] [the ops, when the verb is patch]   < document
//! ```

use std::time::Duration;

use panel_control::{Answered, Producer};

const USAGE: &str = "publish <directory> <identifier> <source> <how many> <every so many ms> \
                     [publish|read|patch] [the ops, when the verb is patch] < document";

/// What this process does to the pane after it has published, every turn.
enum Then {
    Nothing,
    Read,
    Patch(String),
}

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let ([dir, id, source, how_many, every_ms], verb, ops) = match &asked[..] {
        [dir, id, source, how_many, every_ms] => {
            ([dir, id, source, how_many, every_ms], "publish", None)
        }
        [dir, id, source, how_many, every_ms, verb] => {
            ([dir, id, source, how_many, every_ms], verb.as_str(), None)
        }
        [dir, id, source, how_many, every_ms, verb, ops] => (
            [dir, id, source, how_many, every_ms],
            verb.as_str(),
            Some(ops.clone()),
        ),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    // **A verb nobody serves ends the process instead of falling back to publishing**, because a
    // silent fallback is how a batch measures the load of the batch beside it under its own name.
    // **And a patch whose ops are missing or empty ends it too**, which is the batch's own guard
    // rather than the contract's: the pane refuses an empty list, so every turn would be a refusal,
    // and what the run then publishes as the cost of patching is the cost of being turned down.
    let after = match (verb, ops) {
        ("publish", _) => Then::Nothing,
        ("read", _) => Then::Read,
        ("patch", Some(ops)) if carries_an_op(&ops) => Then::Patch(ops),
        ("patch", _) => {
            eprintln!(
                "publish: the patching verb needs its ops, and at least one of them\n{USAGE}"
            );
            std::process::exit(2);
        }
        (other, _) => {
            eprintln!("publish: the verb is \"publish\", \"read\" or \"patch\", not \"{other}\"");
            std::process::exit(2);
        }
    };
    let mut document = String::new();
    if let Err(failure) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut document) {
        eprintln!("publish: the document could not be read from standard input ({failure})");
        std::process::exit(2);
    }
    // **An empty document ends the process here**, as empty ops do above, and with the usage in the
    // message: a caller that forgot the pipe would otherwise reach the first publication, fail to
    // build it, and end with a line about the request instead of about the missing input.
    if document.trim().is_empty() {
        eprintln!("publish: nothing arrived on standard input to publish\n{USAGE}");
        std::process::exit(2);
    }
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
        let answered = match &after {
            Then::Nothing => published,
            Then::Read => {
                report("publish", published);
                pane.read_the_document()
            }
            Then::Patch(ops) => {
                report("publish", published);
                pane.patch(ops)
            }
        };
        match answered {
            Ok(Answered::Ok) => {}
            // What a read comes back with, **and only the batch that asked for one may see it**: a
            // document answered to a publication is the pane answering the wrong verb, which is the
            // case the arm below exists to report and not one to pass over in silence.
            Ok(Answered::Document(_)) if matches!(after, Then::Read) => {}
            // **Said and not fatal.** A refusal is a real answer — another source holds the pane —
            // and the run's own report is what decides whether the batch is valid, not this process.
            Ok(Answered::Failed(why)) => eprintln!("publish: refused ({why})"),
            // **The three `split_*` verbs are the other half of the channel and this example never
            // asks for them.** A payload here would mean the pane answered the wrong request, which
            // is worth a line and not a panic: this process's job is the load, and the batch's own
            // report is what decides whether the run counts.
            Ok(
                Answered::Retired { .. }
                | Answered::Opened(_)
                | Answered::Text(_)
                | Answered::Document(_),
            ) => {
                eprintln!("publish: the pane answered something else to a publication");
            }
            // **An argument this process got wrong is not the channel dying**, and saying so
            // matters: the run is about to end either way, and the line left on screen is the only
            // thing whoever reads the log has.
            Err(failure) if failure.kind() == std::io::ErrorKind::InvalidInput => {
                eprintln!("publish: what was sent could not be built ({failure})");
                return;
            }
            Err(failure) => {
                eprintln!("publish: the channel is gone ({failure})");
                return;
            }
        }
        std::thread::sleep(every);
    }
}

/// Whether the ops handed in carry at least one op.
///
/// **Read here rather than trusted**, because the shape of this argument is what the batch's whole
/// load is: a list that parses and holds nothing is refused by the pane, and a run of refusals looks
/// from the report exactly like a run of patches that cost less than expected.
fn carries_an_op(ops: &str) -> bool {
    serde_json::from_str::<Vec<serde_json::Value>>(ops).is_ok_and(|ops| !ops.is_empty())
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
