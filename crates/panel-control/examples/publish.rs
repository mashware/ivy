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
//! ```text
//! publish <directory> <identifier> <source> <how many> <every so many ms> <document>
//! ```

use std::time::Duration;

use panel_control::{Answered, Producer};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let [dir, id, source, how_many, every_ms, document] = &asked[..] else {
        eprintln!(
            "publish <directory> <identifier> <source> <how many> <every so many ms> <document>"
        );
        std::process::exit(2);
    };
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
        match pane.publish(source, &document) {
            Ok(Answered::Ok) => {}
            // **Said and not fatal.** A refusal is a real answer — another source holds the pane —
            // and the run's own report is what decides whether the batch is valid, not this process.
            Ok(Answered::Failed(why)) => eprintln!("publish: refused ({why})"),
            // **The three `split_*` verbs are the other half of the channel and this example never
            // asks for them.** A payload here would mean the pane answered the wrong request, which
            // is worth a line and not a panic: this process's job is the load, and the batch's own
            // report is what decides whether the run counts.
            Ok(Answered::Opened(_) | Answered::Text(_)) => {
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
