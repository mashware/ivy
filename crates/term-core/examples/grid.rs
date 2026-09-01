//! Spawns a program, feeds its output into the grid and dumps it as text.
//!
//! It is the by-hand check of the milestone: you see what the terminal *understands*, not the
//! bytes it receives. With `vim` or `htop` inside, the grid has to read as their interface.
//!
//! ```text
//! cargo run --example grid -p term-core            # zsh -l
//! cargo run --example grid -p term-core -- vim     # vim
//! cargo run --example grid -p term-core -- htop
//! ```
//!
//! At 2 seconds it resizes to 30×100 and dumps again, which is how the reflow is checked.

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use term_core::{Pty, Screen, Size};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // With no arguments, a login shell. With arguments, whatever is asked as-is: `-l` means
    // different things in each program (in `vim` it is lisp mode), so it is not slipped in behind.
    let mut cli = std::env::args().skip(1);
    let program = cli.next().unwrap_or_else(|| "zsh".to_string());
    let args: Vec<String> = match cli.collect::<Vec<_>>() {
        v if v.is_empty() && program == "zsh" => vec!["-l".to_string()],
        v => v,
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let initial = Size { rows: 24, cols: 80 };

    let (mut pty, pty_output) = Pty::spawn(&program, &args, initial)?;
    let mut screen = Screen::new(initial);

    pump(&pty_output, &mut screen, Duration::from_secs(2));
    println!("── grid {}×{} ──", initial.rows, initial.cols);
    println!("{}", screen.to_text());

    // The two halves of the resize: the grid reflows, and the program finds out and repaints.
    // The grid goes first on purpose: as soon as the program knows the new size it starts to send
    // bytes for it, and if the grid were still the old one they would be interpreted against the
    // wrong dimensions.
    let resized = Size {
        rows: 30,
        cols: 100,
    };
    screen.resize(resized);
    pty.resize(resized)?;

    pump(&pty_output, &mut screen, Duration::from_secs(2));
    println!(
        "\n── grid {}×{} after resizing ──",
        resized.rows, resized.cols
    );
    println!("{}", screen.to_text());

    Ok(())
}

/// Passes whatever is in the channel into the grid for a while.
///
/// `advance` is called from here and not from a thread: the consumer decides how much it processes
/// per pass. Careful, this is **not** promotable as-is to the paint loop: `recv_timeout` blocks and
/// holds the thread until the deadline runs out, and a frame is a call that has to return control.
/// What is inherited is the shape, not the code.
fn pump(pty_output: &Receiver<Vec<u8>>, screen: &mut Screen, for_how_long: Duration) {
    let until = Instant::now() + for_how_long;
    while Instant::now() < until {
        match pty_output.recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => screen.advance(&bytes),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        }
    }
}
