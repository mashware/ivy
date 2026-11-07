//! What whoever uses the terminal can change about it, and where it is read from.
//!
//! ## This file is written by a person, and that decides almost everything here
//!
//! The window's layout is written by the application, so a half-baked document can only come
//! from a fault of our own and **D16** decided not to open it half-baked: guessing half a layout is worse than not
//! restoring it. Here it is the other way around. **A person who makes mistakes types it**, and for each line there is an
//! exact replacement —the factory value—, so **it is tolerated per line**: the one that is not understood is
//! ignored, which one it is gets said, and the rest is applied.
//!
//! It is not an exception to D16: it is the side D16 already contemplates when it contrasts the panel contract,
//! *«which a third party writes by hand»*. And it coincides with what the four terminals this
//! project competes against do —WezTerm, Kitty, Alacritty and Ghostty warn and carry on with the factory settings—, but that
//! coincidence is a confirmation, not the reason.
//!
//! ## And that is why the format is plain text and not JSON
//!
//! Per-line tolerance is the requirement, and **in JSON a forgotten comma prevents building the whole
//! document**. The panel contract is JSON and tolerates per field, but it can afford to because **a program
//! generates it**; here the forgotten comma is the most common error there is. In plain text that global failure
//! cannot happen: the worst case is one line. And it admits comments, without which a configuration
//! file does not explain itself.

pub(crate) mod document;
pub(crate) mod editor;
pub(crate) mod store;

use std::path::{Path, PathBuf};

use eframe::egui::Color32;

/// The file's name, beside the layout and not inside it.
pub const FILE: &str = "config";

/// The value that unbinds a shortcut, **and the only one the terminal publishes as a word**.
///
/// **It lives once because it was written three times**: the two branches that recognize it and the warning that tells
/// whoever makes a mistake what to write. Changing the token in the parser and forgetting the warning leaves the terminal
/// recommending a word it rejects itself, and the test that covers that warning asserts over another part
/// of the sentence, so it would not see it.
const OFF: &str = "none";

/// The file's ceiling, applied **before** parsing.
///
/// **It is literally the layout's, not a copy with the same number.** A comment saying «the
/// same as that one» next to a constant of its own is a promise nothing forces to keep: the day one
/// of the two changes, the comment lies and no one notices.
use crate::session::MAX_BYTES;

/// How many bad lines get said before going silent.
///
/// **The warning here IS the product, not a marginal note.** The layout warns once per class of
/// problem because the application writes it and the warning only informs; here, with two badly written colors,
/// hearing only one makes half of it get fixed and reopened. But a file pasted from the wrong
/// place must not fill the error output either, so there is a cap **and how many were left is said**.
const MAX_WARNINGS: usize = 20;

/// The format version this reader understands.
///
/// **It is optional, unlike in the layout, and the reason is the one D16 gives for requiring it there.**
/// There the file **freezes the shape of a tree**, so an old reader would misinterpret a new
/// file. Here the format is flat: a key a reader does not know **is ignored and said**, and one that
/// is missing takes its factory value. There is no shape to freeze, so requiring a ritual line from whoever
/// writes the file would protect from nothing.
///
/// What is needed is the half that does protect: **a version GREATER than this one is not applied**, in case one
/// day the format changes incompatibly. With no file in the way there is nothing to decide.
const VERSION: u64 = 1;

/// What happens to the **whole** file, when there is nothing to apply.
///
/// A bad line is not here: that is a [`Warning`], and the rest of the file is applied all the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigProblem {
    /// There is no file. **It is the normal case** and it is not warned about.
    Absent,
    /// There is a link with that name and it points to nothing.
    BrokenLink,
    /// There is something with that name and it is not a file.
    NotAFile,
    /// It could not be read.
    Unreadable(String),
    /// It goes over the ceiling.
    TooBig,
    /// It is not text.
    NotText,
    /// It claims to be of a version this terminal does not know.
    Version(u64),
}

impl ConfigProblem {
    /// Whether it is worth saying through the error output.
    ///
    /// **That there is no file is the normal case, not a problem**, and saying it on every startup of someone who
    /// configures nothing would be pure noise. The rule lives **in the type** and not in the caller, just as in the
    /// sibling module: it is the only way two places that use it do not stop coinciding.
    pub fn warrants_warning(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

impl std::fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(f, "there is no settings file"),
            Self::BrokenLink => write!(f, "the settings link points nowhere"),
            Self::NotAFile => write!(f, "that is not a file"),
            Self::Unreadable(why) => write!(f, "it could not be read: {why}"),
            Self::TooBig => {
                write!(f, "it is over the {} KiB that get read", MAX_BYTES / 1024)
            }
            Self::NotText => write!(f, "it is not text"),
            Self::Version(which) => write!(
                f,
                "it claims to be version {which} and this terminal understands up to {VERSION}"
            ),
        }
    }
}

/// What happens to **one line**, with where it is.
///
/// **It carries the line number because without it the warning is useless**: «a badly written color» in a file of
/// forty lines forces going over all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub line: usize,
    pub missing: Missing,
}

/// What a line is missing to be able to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// It is not of the form `key = value`.
    NoEquals,
    /// It has the equals but the key or the value is missing.
    Incomplete,
    /// The key is none of the ones this terminal understands.
    UnknownKey(String),
    /// The value is not valid for that key.
    InvalidValue { key: String, value: String },
    /// The theme name is not in the built-in catalog.
    UnknownTheme(String),
    /// That key was already set above; this one wins.
    Repeated(String),
    /// That action's chord is decided by the desktop: it can only be turned off.
    OnlyTurnsOff(String),
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.missing {
            Missing::NoEquals => write!(f, "it is not of the form 'key = value'"),
            Missing::Incomplete => write!(f, "the key or the value is missing"),
            Missing::UnknownKey(key) => write!(f, "'{key}' is not a setting this terminal knows"),
            Missing::InvalidValue { key, value } => {
                write!(f, "'{}' is not valid for '{key}'", trimmed(value))
            }
            Missing::UnknownTheme(value) => write!(
                f,
                "'{}' is not a known theme; choose 'tokyonight' or 'gruvbox-dark'",
                trimmed(value)
            ),
            Missing::Repeated(key) => write!(f, "'{key}' was already set above; this one wins"),
            Missing::OnlyTurnsOff(key) => write!(
                f,
                "the chord for '{key}' is the desktop's and cannot be changed; \
                 the only thing you can do is turn it off with '{OFF}'"
            ),
        }
    }
}

/// How much of a value fits in a warning.
///
/// **The value comes out of the file as is, and a file can bring a line of a hundred thousand characters or some
/// escape codes.** It is not attack surface —the file belongs to whoever uses the terminal— but this project's
/// hard rules contemplate the file **of foreign origin**, which here is a `config` copied from
/// anywhere. Trimming costs one line and makes the warning legible.
fn trimmed(value: &str) -> String {
    const FITS: usize = 60;
    if value.chars().count() <= FITS {
        return value.escape_debug().to_string();
    }
    let short: String = value.chars().take(FITS).collect();
    format!("{}…", short.escape_debug())
}

/// The colors the file requests, each one **only if it requests it**.
///
/// There are **twenty-two**: the sixteen of the palette and the six of our own.
///
/// **Each field is optional on purpose**: what the file does not say stays as it is by default, which is
/// what makes a file with a single line work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Colors {
    /// The sixteen of the palette that the program inside requests.
    pub palette: [Option<Color32>; 16],
    pub background: Option<Color32>,
    pub foreground: Option<Color32>,
    pub cursor: Option<Color32>,
    /// The line that separates two consoles.
    pub divider: Option<Color32>,
    /// What marks the console the keyboard goes to.
    pub focus: Option<Color32>,
    /// The status panel's background.
    pub panel: Option<Color32>,
}

/// What the file requests, already understood.
// **`Copy` falls away here**: the shortcuts are a list, and a list does not copy itself. The type is passed by
// reference, which is how its only two consumers already asked for it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub colors: Colors,
    pub theme: Option<crate::theme::NamedTheme>,
    /// The family the file asks to paint with. `None` if it asks for none.
    ///
    /// **The name arrives as it was written, and it is not laziness.** Whether that family exists is
    /// something only the machine can answer, and answering it costs between 17.3 and 23.2 ms
    /// (**D24**) — the very cost this project decided never to pay at start-up. Checking it here
    /// would move that cost into reading the file, where nobody asked for it, and would pay it for
    /// **any** value, including the ones nothing will ever be painted with.
    pub font: Option<String>,
    /// The font body, in points. `None` if the file does not request it.
    ///
    /// **It is named the same as in the window and different from in the file, on purpose.** `size` is the key
    /// people type and is published contract; `font_size` is what the rest of the code calls this
    /// number. The translation happens **once**, where the key is parsed, and not on every jump.
    pub font_size: Option<f32>,
    /// The shortcuts the file changes: the action and its new chord, or `None` to unbind it.
    ///
    /// **A list and not an already resolved table**, because order matters: «the last one wins» can only be
    /// decided by applying them in the order they were written.
    pub shortcuts: Vec<(crate::keyboard::Action, Option<crate::keyboard::Chord>)>,
    /// Whether the file settles the blink, over the program's head. `None` leaves it to the program.
    ///
    /// **It is a ceiling in both directions and not a default**: «on» blinks however steady a shape
    /// the program asks for, «off» never blinks however loudly it asks. A ceiling a program can climb
    /// over is not a ceiling, and the reason this is absolute — with the softer rules that were
    /// discarded — is **D37**.
    pub blink: Option<bool>,
    /// Whether the file turned off the copy shortcut.
    ///
    /// **Apart from the list, because copy has no chord to move.** Its chord is resolved by the
    /// desktop before it arrives here —and without saying whether there was Shift—, so the only thing that
    /// can be done with it is to stop attending it, which is what gives `Ctrl+Shift+C` back to `vim`.
    pub copy: Option<bool>,
}

/// The least and the most admitted as a font body.
///
/// **It is not a fussiness: outside this range the window stops being a terminal.** At two points nothing
/// is legible and at two hundred three columns fit, and the panes' tiling has minimums that would stop being
/// possible to meet. The extremes are generous on purpose —some work at 8 and some present at 40—:
/// what is rejected is what can only be a typo.
const MIN_FONT_SIZE: f32 = 4.0;
const MAX_FONT_SIZE: f32 = 96.0;

/// The lines that could not be applied: the ones kept **and how many there were**.
///
/// **Both numbers, and not just the list, because the cap lies if it keeps quiet.** Keeping twenty and not saying
/// there were forty would make one believe the file is fine from the twentieth on.
///
/// **It does not print: it decides.** Who says it through the error output is the caller's business, just as in the
/// sibling module — printing cannot be observed from inside the process, so the only thing a
/// test can tie down is the decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Warnings {
    kept: Vec<Warning>,
    total: usize,
}

impl Warnings {
    fn push(&mut self, warning: Warning) {
        self.total += 1;
        if self.kept.len() < MAX_WARNINGS {
            self.kept.push(warning);
        }
    }

    /// The ones that were kept, which are the first [`MAX_WARNINGS`].
    pub fn kept(&self) -> &[Warning] {
        &self.kept
    }

    /// How many lines gave a problem in total, counting the ones that were not kept.
    pub fn total(&self) -> usize {
        self.total
    }

    /// How many were left unlisted by the cap.
    pub fn unlisted(&self) -> usize {
        self.total - self.kept.len()
    }
}

/// Where the file lives, reading the environment.
///
/// A one-line wrapper, with the work in [`path_from`], which is the sibling module's pattern and for its
/// same reason: setting these variables from a test is undefined behavior in a process with
/// threads, so the decision has to be testable without touching them.
pub fn path() -> Option<PathBuf> {
    path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// Where the file lives, with the two given values.
pub fn path_from(
    config: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    crate::session::directory_from(config, home).map(|directory| directory.join(FILE))
}

/// Which door this session comes in through.
///
/// **An enum and not a boolean**, because in the call a loose `false` does not say what it is that
/// it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entry {
    /// The normal one: whoever opens the terminal to use it.
    Normal,
    /// A batch of the measurement bench.
    Measuring,
}

/// The settings that have to be used: the file's, **or none if this is a measurement batch**.
///
/// **The veto holds up more than it seems.** The measurement script already launches with the
/// configuration directory isolated, but **whoever launches the binary by hand with the measurement
/// variables** —which is how the documentation says to open a particular layout— would read theirs.
/// Their font size would change the grid, and **every** figure published by this project depends on
/// the grid.
///
/// And there is a second consequence: the script **invalidates a run** if a batch that does not
/// dump in Japanese looks for fonts. With a configured font read under measurement, every batch
/// would enumerate and **every run would come out invalid**.
///
/// **The whole file is vetoed, not «the settings that affect the measurement».** That list would be
/// a list that forgets to grow; this one is read at a glance.
pub(crate) fn to_use(
    entry: Entry,
    path: Option<&Path>,
) -> (Settings, crate::measure::ReadingSettings) {
    if entry == Entry::Measuring {
        // **The one setting a measured run does get, and it is switched off rather than read.**
        // The cursor blinks by default (**D37**), so a measured pane — focused, with a cursor, and
        // nobody having asked for anything — would wake the window 1.7 times a second for the first
        // fifteen seconds of every batch. The file cannot be the way through: it is vetoed a few
        // lines above, for reasons that are not reopened here. So the ceiling comes from the
        // measurement gate itself, which is the same regime that already vetoes the file and forbids
        // saving, and the report declares it on its own line.
        return (
            Settings {
                blink: Some(false),
                ..Settings::default()
            },
            crate::measure::ReadingSettings::Vetoed,
        );
    }
    let Some(path) = path else {
        // **With nowhere to trust it is not read either, and it is said.** Whoever has the
        // environment without an absolute path would write a file that is never applied; the
        // sibling module already has that same verb.
        eprintln!(
            "agent-terminal: there is nowhere to look for the settings (neither $XDG_CONFIG_HOME nor $HOME \
             gives an absolute path). Opening with the factory ones."
        );
        return (
            Settings::default(),
            crate::measure::ReadingSettings::Missing,
        );
    };
    let started = std::time::Instant::now();
    match read(path) {
        Ok((settings, warnings)) => {
            // **Every one that was kept is said, and how many were left out.** Keeping quiet about
            // the extra ones without saying how many they were would make you believe that beyond
            // the ceiling the file is fine.
            for warning in warnings.kept() {
                eprintln!("agent-terminal: {warning}");
            }
            if warnings.unlisted() > 0 {
                eprintln!(
                    "agent-terminal: and {} more line(s) with problems, which are not listed",
                    warnings.unlisted()
                );
            }
            (
                settings,
                crate::measure::ReadingSettings::Applied {
                    how_long: started.elapsed(),
                    warnings: warnings.total(),
                },
            )
        }
        Err(problem) => {
            // **That there is no file is the normal case and is not said**; the type itself decides
            // it, so that this branch and any other that reads the file do not stop agreeing.
            if problem.warrants_warning() {
                eprintln!(
                    "agent-terminal: {} could not be used: {problem}. Opening with the factory ones.",
                    path.display()
                );
            }
            // **It is declared that it WAS looked at, even though nothing was applied.** «Factory
            // because the file was broken» and «factory because a batch vetoed it» are two
            // different things, and the report only tells batches apart if this line does not
            // confuse them.
            (
                Settings::default(),
                crate::measure::ReadingSettings::Missing,
            )
        }
    }
}

/// Reads the file at that path. Returns what was understood **and** what was not.
///
/// **The order is the layout's mold**: ask for the metadata, check that it is a file, **open and
/// read with the cap in place**, apply the ceiling over the read bytes, and only then look at whether it is text.
pub fn read(path: &Path) -> Result<(Settings, Warnings), ConfigProblem> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        // **A broken link is NOT «there is no file», and the difference shows.** Whoever created their configuration and
        // moved it deserves to be told; silence is for whoever has written nothing.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(match std::fs::symlink_metadata(path) {
                Ok(_) => ConfigProblem::BrokenLink,
                Err(_) => ConfigProblem::Absent,
            });
        }
        Err(error) => return Err(ConfigProblem::Unreadable(error.to_string())),
    };
    if !metadata.is_file() {
        return Err(ConfigProblem::NotAFile);
    }
    let file = std::fs::File::open(path).map_err(|f| ConfigProblem::Unreadable(f.to_string()))?;
    let raw = crate::session::read_capped(file, metadata.len())
        .map_err(|error| ConfigProblem::Unreadable(error.to_string()))?;
    // **The ceiling goes over the READ bytes, not over the ones the file's metadata declares.** They are not the
    // same: a file in `/proc` declares zero and delivers a hundred and seventy-five bytes, checked
    // by running it. An earlier version read the whole thing with `fs::read` and compared afterwards, with a comment
    // that said «open» where nothing was opened — the ceiling existed and **did not bound the reservation**, which is
    // exactly what its own reason says to avoid. A review caught it.
    if raw.len() > MAX_BYTES {
        return Err(ConfigProblem::TooBig);
    }
    let text = String::from_utf8(raw).map_err(|_| ConfigProblem::NotText)?;
    interpret(&text)
}

/// What a line turns out to be.
///
/// **It is classified ONCE.** The first version returned «nothing» both for an empty line and for a badly
/// written one, and the caller had to look at it again to know which of the two it was: the
/// same decision taken twice, in two places that could stop coinciding.
enum ConfigLine<'a> {
    /// Empty, spaces, or a comment.
    Blank,
    /// A key and its value, already without spaces.
    Pair(&'a str, &'a str),
    /// It says something, but not in the shape that is needed.
    Bad(Missing),
}

/// Parses the text line by line.
pub fn interpret(text: &str) -> Result<(Settings, Warnings), ConfigProblem> {
    let mut settings = Settings::default();
    let mut warnings = Warnings::default();
    let mut already_set: Vec<String> = Vec::new();

    // **The byte order mark is removed, and it is the cruelest bug this file could have.** It is put in
    // by Windows editors and some pages when copying; it stays stuck to the first key, and the
    // resulting warning points at something that **on screen looks exactly the same** as the correct key:
    // ««color.1» is not a known setting». Checked by running it.

    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    for (index, line_text) in text.lines().enumerate() {
        let line = index + 1;
        let (key, value) = match classify(line_text) {
            ConfigLine::Blank => continue,
            ConfigLine::Bad(missing) => {
                warnings.push(Warning { line, missing });
                continue;
            }
            ConfigLine::Pair(key, value) => (key, value),
        };

        // **The version is looked at here and not earlier**, because it is one more key: it can go on any line, and
        // that it is missing is the normal thing.
        // **The version does not enter the repeated accounting, and it is not an oversight.** «This one wins» means
        // that this line stepped over another's value; the version **stores no value**, it is a veto that is
        // checked and forgotten. Saying that something wins would be lying twice when its value is also not
        // understood, which is what the first version of this fix did. A review caught it.
        if key == "version" {
            match value.parse::<u64>() {
                Ok(which) if which > VERSION => return Err(ConfigProblem::Version(which)),
                Ok(_) => {}
                Err(_) => warnings.push(Warning {
                    line,
                    missing: Missing::InvalidValue {
                        key: key.to_owned(),
                        value: value.to_owned(),
                    },
                }),
            }
            continue;
        }
        if let Err(missing) = apply(&mut settings, key, value) {
            warnings.push(Warning { line, missing });
            // **A line that does not apply does not count as set**, so it is not warned as repeated for
            // it: saying «this one wins» of a line that wins nothing is a warning that lies, and on top of that
            // contradicts the one next to it. Checked by running it before fixing it.
            continue;
        }

        // **The repeated warning goes AFTER applying, and that is why it tells the truth.** It is emitted only when
        // this line really has kept the value.
        if already_set.iter().any(|already| already == key) {
            warnings.push(Warning {
                line,
                missing: Missing::Repeated(key.to_owned()),
            });
        } else {
            already_set.push(key.to_owned());
        }
    }

    Ok((settings, warnings))
}

impl Settings {
    /// Puts on the window whatever whoever uses it configured, **except the font**.
    ///
    /// The family named in the file is applied by [`crate::choose_the_font`] instead, and it is not tidiness:
    /// everything here puts a value on the window, while that one talks to the text engine, needs the
    /// context and can cost tens of milliseconds. It runs beside this, at start-up.
    ///
    /// **It goes here and not in the constructor**, just like the injected drags and the restoring
    /// span: taking it inside would force passing it through three constructors with dozens of
    /// calls, and none of them configures anything.
    pub(crate) fn apply_to(&self, window: &mut crate::Window) {
        window.theme = self.resolved_theme();
        // **What the file does not ask for stays as it is**, which is what makes it possible to
        // configure just one color without inheriting along the way a font size nobody wrote.
        window.font_size = self.font_size.unwrap_or(window.font_size);
        window.copy_enabled = self.copy.unwrap_or(window.copy_enabled);
        window.blink.ceiling = self.blink.or(window.blink.ceiling);
        // **They are applied in the order they were written, and that is why they arrive as a
        // list.** «The last one wins» can only be decided that way, and it is what makes pasting
        // one set of shortcuts on top of another do what is expected.
        for (action, chord) in &self.shortcuts {
            if let Some(displaced) = window.keymap.bind(*action, *chord) {
                // **The clash is said out loud, not resolved in silence.** Without this, whoever
                // reassigns a chord that already had an owner loses the other shortcut without
                // noticing, and finds out the day they need it.
                eprintln!(
                    "agent-terminal: '{}' is left without a shortcut: its chord goes to '{}', which comes later",
                    displaced.as_written(),
                    action.as_written()
                );
            }
        }
    }
}

impl Settings {
    fn resolved_theme(&self) -> crate::theme::Theme {
        self.theme
            .map(crate::theme::NamedTheme::theme)
            .unwrap_or_default()
            .with_colors(&self.colors)
    }
}

/// Puts the value where it belongs, if the key and the value are valid.
fn apply(settings: &mut Settings, key: &str, value: &str) -> Result<(), Missing> {
    if let Some(name) = key.strip_prefix("key.") {
        // **Copy still has its key, and it only accepts `none`.** It is the key this terminal already
        // published, and whoever wrote it to give the chord back to `vim` has no reason to find out
        // that inside it stopped being a chord. What cannot be done is reassigning it, and that is said.
        if name.eq_ignore_ascii_case("copy") {
            if !value.eq_ignore_ascii_case(OFF) {
                return Err(Missing::OnlyTurnsOff(key.to_owned()));
            }
            settings.copy = Some(false);
            return Ok(());
        }
        let action = crate::keyboard::Action::from_written(name)
            .ok_or_else(|| Missing::UnknownKey(key.to_owned()))?;
        // **`none` unbinds the action, and it is the half that makes all this useful**: it is what gives a
        // chord back to the program inside when `vim` or `tmux` want it for themselves.
        let chord = if value.eq_ignore_ascii_case(OFF) {
            None
        } else {
            Some(crate::keyboard::Chord::from_written(value).ok_or_else(|| {
                Missing::InvalidValue {
                    key: key.to_owned(),
                    value: value.to_owned(),
                }
            })?)
        };
        settings.shortcuts.push((action, chord));
        return Ok(());
    }
    // **Before the colour table on purpose**: `cursor` on its own is the colour, and `cursor.blink`
    // would otherwise fall through to `strip_prefix("color.")` and come back as an unknown key.
    if key == "cursor.blink" {
        // **«on» and «off», and not the «none» the shortcuts use.** There it means «hand the chord
        // back to the program»; here the program is exactly who is being overruled, so the same word
        // would mean the opposite thing in two lines of the same file.
        settings.blink = Some(match value {
            _ if value.eq_ignore_ascii_case("on") => true,
            _ if value.eq_ignore_ascii_case("off") => false,
            _ => {
                return Err(Missing::InvalidValue {
                    key: key.to_owned(),
                    value: value.to_owned(),
                });
            }
        });
        return Ok(());
    }
    if key == "font" {
        settings.font = Some(value.to_owned());
        return Ok(());
    }
    if key == "theme" {
        settings.theme = Some(
            crate::theme::NamedTheme::from_written(value)
                .ok_or_else(|| Missing::UnknownTheme(value.to_owned()))?,
        );
        return Ok(());
    }
    if key == "size" {
        let amount = value
            .parse::<f32>()
            .ok()
            .filter(|c| c.is_finite() && (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(c))
            .ok_or_else(|| Missing::InvalidValue {
                key: key.to_owned(),
                value: value.to_owned(),
            })?;
        settings.font_size = Some(amount);
        return Ok(());
    }
    let colors = &mut settings.colors;
    let slot: &mut Option<Color32> = match key {
        "background" => &mut colors.background,
        "foreground" => &mut colors.foreground,
        "cursor" => &mut colors.cursor,
        "divider" => &mut colors.divider,
        "focus" => &mut colors.focus,
        "panel" => &mut colors.panel,
        // **The index is digits and nothing else.** Leaving it to the number parser admitted
        // `color.007` and `color.+1` —both applied a color— while `color.-1` was rejected: three
        // ways of writing the same thing with two different results, none documented. A
        // review caught it by running them.
        _ => match key.strip_prefix("color.") {
            Some(index)
                if index.chars().all(|c| c.is_ascii_digit())
                    && index
                        .parse::<usize>()
                        .is_ok_and(|n| n < colors.palette.len()) =>
            {
                // **An index outside the palette is an unknown key, not a bad value**: what
                // is badly written is the setting's name, and saying «the value is not valid» would send one to look at
                // the color.
                &mut colors.palette[index.parse::<usize>().unwrap_or_default()]
            }
            _ => return Err(Missing::UnknownKey(key.to_owned())),
        },
    };
    *slot = Some(color_from(value).ok_or_else(|| Missing::InvalidValue {
        key: key.to_owned(),
        value: value.to_owned(),
    })?);
    Ok(())
}

/// A color written as `#rrggbb`, in uppercase or lowercase.
fn color_from(value: &str) -> Option<Color32> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |from: usize| u8::from_str_radix(&digits[from..from + 2], 16).ok();
    Some(Color32::from_rgb(channel(0)?, channel(2)?, channel(4)?))
}

/// What this line is.
///
/// **The comment is whole-line, and cannot go at the end of a value.** It is not a simplification:
/// the hash is **at the same time** the comment character and a color's prefix, so a
/// comment at the end of the value splits `color.1 = #ff0000` in half and leaves the key without a value.
/// Checked by writing it: the first version of this parser ate all the colors.
///
/// It is exactly what Kitty and Ghostty do, which use this same format, and for the same reason.
fn classify(line: &str) -> ConfigLine<'_> {
    let trimmed_line = line.trim();
    if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
        return ConfigLine::Blank;
    }
    let Some((key, value)) = trimmed_line.split_once('=') else {
        return ConfigLine::Bad(Missing::NoEquals);
    };
    let (key, value) = (key.trim(), value.trim());
    if key.is_empty() || value.is_empty() {
        // **It is distinguished from «it has no equals» on purpose**: whoever wrote `background =` did not forget the
        // equals, they forgot the color, and telling them the former sends them to look where it is not.
        return ConfigLine::Bad(Missing::Incomplete);
    }
    ConfigLine::Pair(key, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::markdown;

    /// What parsing a text that is understood in full returns.
    fn parsed(text: &str) -> (Settings, Warnings) {
        interpret(text).expect("the test text should not bring down the whole file")
    }

    #[test]
    fn a_bad_line_does_not_take_the_good_ones_down_with_it() {
        // **It is the format's reason for being, and what separates this file from the layout's.** That one is
        // written by the application and does not open half-baked; this one is typed by a person, and taking down the twenty-two
        // colors because one is badly written would force them to find the error blind.
        let (settings, _) = parsed("color.1 = #ff0000\ncolor.2 = sky blue\ncolor.3 = #0000ff");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "the GOOD line before the bad one was not applied"
        );
        assert_eq!(
            settings.colors.palette[3],
            Some(Color32::from_rgb(0, 0, 0xFF)),
            "the good line AFTER the bad one was not applied: the parser stopped at the error"
        );
        assert_eq!(
            settings.colors.palette[2], None,
            "the badly written color was applied all the same"
        );
    }

    #[test]
    fn the_bad_line_is_said_with_its_number() {
        // **Without the number, the warning is useless**: «a badly written color» in a file of forty lines
        // forces going over all of them. And with no warning at all, whoever wrote the line will believe the terminal
        // ignores what they ask of it.
        let (_, warnings) = parsed("# a comment\n\ncolor.1 = purple\n");

        assert_eq!(
            warnings.kept().len(),
            1,
            "the bad line was not warned about"
        );
        assert_eq!(
            warnings.kept()[0].line,
            3,
            "the warning points at the wrong line: the empty ones and the comments are counted too, \
             because the number has to match what is seen in the editor"
        );
        assert!(
            warnings.kept()[0].to_string().contains("purple"),
            "the warning does not say which value was not understood: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn a_file_with_nothing_configured_leaves_the_factory_theme() {
        // **AC5 by its cheapest side**, and the one that upholds that this milestone breaks no one: what the
        // file does not say stays as it is.
        let (settings, warnings) = parsed("# only comments\n\n   \n");

        assert_eq!(
            settings,
            Settings::default(),
            "a file with no settings changed something"
        );
        assert_eq!(
            warnings.total(),
            0,
            "an empty or comment line was counted as a problem"
        );
        assert_eq!(
            crate::theme::Theme::from_colors(&settings.colors)
                .color(term_core::TermColor::Indexed(1)),
            crate::theme::Theme::default().color(term_core::TermColor::Indexed(1)),
            "with nothing configured the theme stopped being the factory one"
        );
    }

    #[test]
    fn a_color_is_not_mistaken_for_a_comment() {
        // **The hash is at the same time the comment and a color's prefix**, and the first version of
        // this parser admitted comments at the end of the line: it ate ALL the colors, because
        // it cut `color.4 = #123456` right before the value. This suite uncovered it on the first try.
        //
        // That is why the comment is whole-line, as in Kitty and in Ghostty.
        let (settings, warnings) = parsed("# the usual blue\ncolor.4 = #123456");

        assert_eq!(
            settings.colors.palette[4],
            Some(Color32::from_rgb(0x12, 0x34, 0x56)),
            "a color's value was lost for starting with a hash"
        );
        assert_eq!(
            warnings.total(),
            0,
            "the comment line or the color's line were counted as a problem"
        );
    }

    #[test]
    fn a_repeated_key_lets_the_last_one_win_and_it_is_said() {
        // **The two halves.** That the last one wins is what makes pasting one theme on top of another work
        // as expected; saying it is what prevents someone from spending half an hour changing a line that another
        // further down is stepping on.
        let (settings, warnings) = parsed("color.1 = #ff0000\ncolor.1 = #00ff00");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0, 0xFF, 0)),
            "the first one won: pasting one theme on top of another would not do what is expected"
        );
        assert_eq!(warnings.kept().len(), 1, "the repeated key was not said");
        assert!(
            warnings.kept()[0].to_string().contains("color.1"),
            "the warning does not say which key was repeated: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn the_blink_is_settled_by_the_file_or_left_to_the_program() {
        // The three states of the key, and the third is the one that matters: **absent is not
        // «off»**. Reading `None` as a `false` anywhere downstream turns a ceiling into a default
        // and takes the blink away from every program that asks for it.
        assert_eq!(parsed("cursor.blink = on").0.blink, Some(true));
        assert_eq!(parsed("cursor.blink = off").0.blink, Some(false));
        assert_eq!(parsed("size = 14").0.blink, None, "absent is not off");
        assert_eq!(
            parsed("cursor.blink = ON").0.blink,
            Some(true),
            "the rest of this file ignores case and this key would be the exception"
        );
    }

    #[test]
    fn the_blink_key_does_not_eat_the_cursor_colour() {
        // `cursor` on its own is a colour and `cursor.blink` is not one, and what matters is the
        // **order** of the two in the same dispatch, not the distance: the blink is matched before
        // the colour table. Mutated: matching it after sends it to `strip_prefix("color.")` and it
        // comes back as an unknown key.
        let (settings, warnings) = parsed("cursor = #ff0000\ncursor.blink = off");

        assert_eq!(settings.colors.cursor, Some(Color32::from_rgb(0xFF, 0, 0)));
        assert_eq!(settings.blink, Some(false));
        assert_eq!(
            warnings.total(),
            0,
            "one of the two keys was not understood"
        );
    }

    #[test]
    fn a_blink_nobody_can_read_is_said_with_its_line_and_the_rest_applies() {
        let (settings, warnings) = parsed("size = 14\ncursor.blink = maybe\nfont = Iosevka");

        assert_eq!(
            settings.blink, None,
            "a value nobody understands was applied"
        );
        assert_eq!(
            settings.font_size,
            Some(14.0),
            "the line before it was dropped"
        );
        assert_eq!(
            settings.font.as_deref(),
            Some("Iosevka"),
            "the line after it was dropped: one bad line is not a bad file"
        );
        assert_eq!(warnings.kept().len(), 1);
        let said = warnings.kept()[0].to_string();
        assert!(
            said.contains("cursor.blink") && said.contains("maybe") && said.contains('2'),
            "the warning has to carry the key, the value and the line number: «{said}»"
        );
    }

    #[test]
    fn a_palette_color_out_of_range_is_neither_swallowed_nor_overflows() {
        // The palette has sixteen slots and the index is written by a person. A `color.99` has to
        // warn, not apply somewhere else nor go out of the vector.
        let (settings, warnings) =
            parsed("color.16 = #ffffff\ncolor.999 = #ffffff\ncolor.-1 = #ffffff");

        assert_eq!(
            settings,
            Settings::default(),
            "an index outside the palette changed something"
        );
        assert_eq!(
            warnings.total(),
            3,
            "the three impossible indices were not warned about"
        );
    }

    #[test]
    fn a_badly_written_color_does_not_slip_through_by_looking_similar() {
        // The four errors that are really made when writing a color by hand.
        for value in ["ff0000", "#ff00", "#ffgg00", "#ff00000"] {
            let (settings, warnings) = parsed(&format!("background = {value}"));
            assert_eq!(
                settings.colors.background, None,
                "«{value}» was accepted as a color and it is not one"
            );
            assert_eq!(warnings.total(), 1, "«{value}» gave no warning");
        }
    }

    #[test]
    fn a_newer_version_is_not_applied_halfway() {
        // **It is the only half of the version that protects from anything**, and that is why it is the only one that exists: a
        // file of a future format could want to mean something different with a known key, so
        // applying the ones that are understood would be worse than applying nothing.
        assert_eq!(
            interpret("version = 99\ncolor.1 = #ff0000"),
            Err(ConfigProblem::Version(99)),
            "a file of a future version was applied all the same"
        );
        let (settings, warnings) = parsed("version = 1\ncolor.1 = #ff0000");
        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "the known version prevented applying the rest"
        );
        assert_eq!(
            warnings.total(),
            0,
            "declaring the known version gave a warning"
        );
    }

    #[test]
    fn without_declaring_a_version_the_file_is_read_all_the_same() {
        // **It is what separates this format from the layout's**: there the version is mandatory because the
        // file freezes the shape of a tree. Here it is flat, and requiring a ritual line from whoever
        // writes it would protect from nothing. Every other case in this suite takes it for granted; this one ties it down.
        let (settings, warnings) = parsed("color.1 = #ff0000");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            warnings.total(),
            0,
            "not declaring the version was counted as a problem"
        );
    }

    #[test]
    fn the_warnings_have_a_cap_and_how_many_were_left_out_is_said() {
        // **A file pasted from the wrong place must not fill the error output**, but keeping quiet about the
        // extra ones without saying how many they were would make one believe that from the twentieth on the file is fine.
        let text: String = (0..MAX_WARNINGS + 7)
            .map(|n| format!("unknown{n} = 1\n"))
            .collect();
        let (_, warnings) = parsed(&text);

        assert_eq!(
            warnings.kept().len(),
            MAX_WARNINGS,
            "more warnings were kept than the cap"
        );
        assert_eq!(
            warnings.total(),
            MAX_WARNINGS + 7,
            "the count stayed at the kept ones, so the rest disappear without a trace"
        );
        assert_eq!(
            warnings.unlisted(),
            7,
            "it is not known how many were left unlisted"
        );
    }

    #[test]
    fn a_line_with_content_and_without_an_equals_warns_and_an_empty_one_does_not() {
        let (_, warnings) = parsed("this is not a setting\n\n   \n# nor this");

        assert_eq!(
            warnings.total(),
            1,
            "either the line without «=» was not warned about, or the empty ones and the comments were"
        );
    }

    #[test]
    fn it_is_not_said_that_a_line_that_wins_nothing_wins() {
        // **The warning was lying, and a review uncovered it by running it.** With the key repeated and the second
        // value badly written, TWO contradictory warnings came out on the same line: «this one wins» and, right
        // below, that that value is not valid. Which means it does not win: the first one stays.
        let (settings, warnings) = parsed("color.1 = #ff0000\ncolor.1 = purple");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "a line with the value badly written erased the good value from before"
        );
        assert_eq!(
            warnings.total(),
            1,
            "the same line was warned about twice, and one of the two says the opposite of the other: {:?}",
            warnings.kept()
        );
        assert!(
            warnings.kept()[0].to_string().contains("purple"),
            "the warning that remained is not the one that explains what happens: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn of_the_version_it_is_not_said_to_win_because_it_wins_nothing() {
        // **«This one wins» means that this line stepped over another's value, and the version stores no
        // value**: it is a veto that is checked and forgotten. An earlier fix put it into that
        // accounting and produced the most absurd warning possible — «this one wins» over a line whose value
        // was not understood either, meaning it neither won nor applied. A review caught it by running it.
        let (_, warnings) = parsed("version = 1\nversion = 1");
        assert_eq!(
            warnings.total(),
            0,
            "it was said that a repeated version «wins», and it wins nothing: {:?}",
            warnings.kept()
        );

        let (_, warnings) = parsed("version = one\nversion = one");
        assert_eq!(
            warnings.total(),
            2,
            "each illegible version has to warn once and only once: {:?}",
            warnings.kept()
        );
        assert!(
            warnings
                .kept()
                .iter()
                .all(|a| !matches!(a.missing, Missing::Repeated(_))),
            "«this one wins» was said of a line whose value was not even understood: {:?}",
            warnings.kept()
        );
    }

    #[test]
    fn the_font_size_is_read_and_the_old_key_is_no_longer_valid() {
        let (settings, warnings) = parsed("size = 18");
        assert_eq!(settings.font_size, Some(18.0), "«size» was not understood");
        assert_eq!(warnings.total(), 0, "«size» gave a warning");

        let (settings, _) = parsed("size = 12.5");
        assert_eq!(
            settings.font_size,
            Some(12.5),
            "a size with decimals was not understood"
        );

        // **There is no alias table, and this is the only thing that ties it down.** D27 decides that the keys move to
        // English with no compatibility because no one has the terminal installed; without this assertion, letting
        // a branch slip through that keeps accepting the old key goes unnoticed, and the debt the decision
        // saved comes back through the back door.
        for old in ["tamaño", "tamano"] {
            let (settings, warnings) = parsed(&format!("{old} = 18"));
            assert_eq!(
                settings.font_size, None,
                "«{old}» is still accepted: there is an alias table where D27 says there is none"
            );
            assert_eq!(warnings.total(), 1, "«{old}» gave no unknown-key warning");
        }
    }

    #[test]
    fn an_impossible_size_does_not_leave_the_window_without_a_grid() {
        // **Out of range the window stops being a terminal**: at two points nothing is legible, at
        // two hundred three columns fit, and the panes' tiling has minimums that would stop being
        // possible to meet. A zero or a negative are worse: the cell would measure zero and the grid would come
        // from dividing by it.
        for value in ["0", "-14", "1", "500", "nan", "inf", "big"] {
            let (settings, warnings) = parsed(&format!("size = {value}"));
            assert_eq!(
                settings.font_size, None,
                "«{value}» was accepted as a font body"
            );
            assert_eq!(warnings.total(), 1, "«{value}» gave no warning");
        }
        // And the extremes that are valid, so the range does not close too much: some work at 8 and
        // some present at 40.
        for value in ["4", "8", "40", "96"] {
            let (settings, _) = parsed(&format!("size = {value}"));
            assert!(
                settings.font_size.is_some(),
                "«{value}» was rejected and it is a working size"
            );
        }
    }

    #[test]
    fn the_palette_index_is_digits_and_nothing_else() {
        // **The asymmetry there was could not be explained**: `color.+1` applied a color and `color.-1` was
        // rejected, both by leaving the decision to the number parser. A review caught it
        // by running them.
        for key in ["color.+1", "color.-1", "color. 1", "color.one", "color."] {
            let (settings, warnings) = parsed(&format!("{key} = #ff0000"));
            assert_eq!(
                settings,
                Settings::default(),
                "«{key}» applied a color, and it is not a form anyone has documented"
            );
            assert_eq!(warnings.total(), 1, "«{key}» gave no warning");
        }

        // **Leading zeros ARE valid, and that is a decision, not an oversight**: whoever writes
        // `color.07` wants the seven and there is no other possible reading. Rejecting it would be hostile for the sake of it.
        let (settings, warnings) = parsed("color.07 = #ff0000");
        assert_eq!(
            settings.colors.palette[7],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "«color.07» did not apply color seven"
        );
        assert_eq!(warnings.total(), 0);
    }

    #[test]
    fn a_very_long_value_is_not_dumped_whole_into_the_warning() {
        // **The value comes out of the file as is**, and a `config` copied from anywhere can bring a
        // line of a hundred thousand characters or some escape codes that talk to the terminal that launched the
        // application. A review caught it.
        let long = "x".repeat(100_000);
        let (_, warnings) = parsed(&format!("background = {long}"));
        let said = warnings.kept()[0].to_string();
        assert!(
            said.chars().count() < 200,
            "the warning came out with {} characters: the whole value is being dumped",
            said.chars().count()
        );

        let (_, warnings) = parsed("background = \u{1b}[31mRED\u{1b}[0m");
        assert!(
            !warnings.kept()[0].to_string().contains('\u{1b}'),
            "the warning lets escape codes through to the terminal that launched the application"
        );
    }

    #[test]
    fn whoever_forgets_the_value_is_told_that_and_not_something_else() {
        // **Telling «it is not of the form key = value» to whoever wrote `background =` sends them to look where it is
        // not**: the equals is in place, what is missing is the color.
        let (_, warnings) = parsed("background =");
        assert_eq!(
            warnings.total(),
            1,
            "the half-baked line was not warned about"
        );
        let said = warnings.kept()[0].to_string();
        assert!(
            said.contains("the key or the value is missing"),
            "the warning describes the fault badly: «{said}»"
        );

        let (_, warnings) = parsed("this has no equals");
        assert!(
            warnings.kept()[0].to_string().contains("'key = value'"),
            "a line without an equals has to say that, and it said: «{}»",
            warnings.kept()[0]
        );
    }

    #[test]
    fn the_byte_order_mark_does_not_eat_the_first_line() {
        // **It is the cruelest bug this file could have**, and testing instead of reading uncovered it: the
        // mark is put in by Windows editors and some pages when copying, it stays stuck to the first
        // key, and the warning points at something that **on screen looks identical** to the correct key. Whoever
        // suffers it has no way to see the difference.
        let (settings, warnings) = parsed("\u{FEFF}color.1 = #ff0000\nbackground = #010203");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "the byte order mark ate the first line"
        );
        assert_eq!(
            warnings.total(),
            0,
            "a line that is correctly written was warned about"
        );
    }

    #[test]
    fn a_windows_file_is_read_all_the_same() {
        // **Windows line breaks arrive on their own**: a theme copied from a web page, a file
        // edited on another machine. If the carriage return stayed stuck to the value, no color
        // would work and the warning would talk about a value that on screen looks correctly written.
        //
        // **The one that resolves it is `str::lines`, not this module, and it is said because it changes what the test
        // ties down**: mutating this file's space trimming does **not** take it down. What it ties down is the contract
        // with that function — the day someone splits the text another way, this falls.
        let (settings, warnings) =
            parsed("# a theme\r\ncolor.1 = #ff0000\r\nbackground = #010203\r\n");

        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0)),
            "the carriage return stayed stuck to the value"
        );
        assert_eq!(settings.colors.background, Some(Color32::from_rgb(1, 2, 3)));
        assert_eq!(
            warnings.total(),
            0,
            "a file with Windows breaks gave warnings"
        );
    }

    #[test]
    fn the_color_is_written_in_uppercase_or_lowercase() {
        // They are copied from pages that write them both ways, so rejecting one would be a fault
        // that whoever suffers it cannot even see.
        let (settings, warnings) = parsed("color.1 = #AABBCC\ncolor.2 = #aabbcc");

        assert_eq!(settings.colors.palette[1], settings.colors.palette[2]);
        assert_eq!(warnings.total(), 0, "one of the two forms was rejected");
    }

    #[test]
    fn a_value_with_an_equals_inside_is_not_split_on_the_second() {
        // A family name can carry an equals — `Iosevka Term = Extended` is written that way on the
        // page it is copied from. Splitting on the **first** equals is what keeps the name whole.
        let (settings, warnings) = parsed("font = Iosevka Term = Extended");

        assert_eq!(warnings.total(), 0, "the line gave a warning");
        // **The whole tail, not just that it starts with the name.** Splitting on the last equals
        // leaves the value as «Extended», and splitting on the first but trimming the tail leaves
        // «Iosevka Term»: both are wrong and only the whole comparison tells them apart.
        assert_eq!(
            settings.font.as_deref(),
            Some("Iosevka Term = Extended"),
            "the line was split on an equals that is not the first"
        );
    }

    #[test]
    fn the_font_is_read_by_its_name_and_is_not_interpreted() {
        // **It arrives as written, spaces inside and all.** A family is named JetBrains Mono, with
        // its space, and whoever strips it here ends up asking the machine for a family nobody has
        // installed.
        let (settings, warnings) = parsed("  font   =   JetBrains Mono  ");

        assert_eq!(warnings.total(), 0, "«font» gave a warning");
        assert_eq!(
            settings.font.as_deref(),
            Some("JetBrains Mono"),
            "the name did not arrive as it was written"
        );
    }

    #[test]
    fn without_a_font_line_nothing_is_asked_of_the_machine() {
        // **This is the half that keeps D24 standing**: enumerating the system's fonts costs between
        // 17.3 and 23.2 ms, and the decision is that it is only paid when somebody asks for it. What
        // is asserted here is the datum whoever searches reads: no line, no name.
        let (settings, warnings) = parsed("size = 14\ncolor.1 = #ff0000");

        assert_eq!(warnings.total(), 0, "a good line gave a warning");
        assert_eq!(
            settings.font, None,
            "a file that names no font left a family to look for anyway"
        );
    }

    #[test]
    fn the_shortcuts_are_read_from_the_file_in_the_order_they_are_written() {
        // **Order matters and that is why they arrive as a list**: «the last one wins» can only be decided
        // by applying them in the order they are written.
        let (settings, warnings) = parsed(
            "key.split-right = ctrl+shift+e\n             key.close-pane = none\n             key.new-tab = ctrl+alt+t",
        );
        assert_eq!(
            warnings.total(),
            0,
            "one of the three lines was not understood"
        );
        assert_eq!(
            settings.shortcuts.len(),
            3,
            "the three shortcuts did not arrive"
        );
        assert_eq!(
            settings.shortcuts[0].0,
            crate::keyboard::Action::SplitRight,
            "the read order is not preserved, and «the last one wins» can no longer be decided"
        );
        assert_eq!(
            settings.shortcuts[1].1, None,
            "«none» did not unbind the action"
        );
    }

    #[test]
    fn the_copy_shortcut_can_only_be_turned_off() {
        // **Its chord is decided by the desktop**, which keeps it before it arrives here and does not even
        // say whether there was Shift. Accepting `key.copy = ctrl+shift+e` would be promising a chord that
        // would never fire and that whoever wrote it would have no way to debug.
        let (settings, warnings) = parsed("key.copy = none");
        assert_eq!(warnings.total(), 0, "«none» was not understood");
        assert_eq!(settings.copy, Some(false), "«none» did not turn off copy");
        assert!(
            settings.shortcuts.is_empty(),
            "copy slipped into the chord list, where it has none to move"
        );

        let (settings, warnings) = parsed("key.copy = ctrl+shift+e");
        assert_eq!(warnings.total(), 1, "reassigning it was not warned about");
        assert_eq!(settings.copy, None, "reassigning it touched the switch");
        assert!(
            format!("{}", warnings.kept()[0]).contains("turn it off"),
            "the warning does not say the only thing that can be done with that chord: {}",
            warnings.kept()[0]
        );

        // And **it is not turned off without asking**: a file that does not name it leaves it as it was.
        let (settings, _) = parsed("background = #101010");
        assert_eq!(
            settings.copy, None,
            "a file that does not talk about copy left it stated all the same"
        );
    }

    #[test]
    fn a_badly_written_shortcut_warns_and_does_not_take_the_others_with_it() {
        // The same rule as the rest of the file, on the surface most typed by hand.
        for (line, what) in [
            (
                "key.made-up = ctrl+shift+e",
                "an action that does not exist",
            ),
            (
                "key.close-pane = ctrl+shift+doesnotexist",
                "a key that does not exist",
            ),
            ("key.close-pane = e", "a chord without a modifier key"),
            ("key.close-pane = ", "without a value"),
        ] {
            let (settings, warnings) =
                parsed(&format!("color.1 = #ff0000\n{line}\ncolor.2 = #00ff00"));
            assert!(
                settings.shortcuts.is_empty(),
                "{what} was applied all the same: «{line}»"
            );
            assert_eq!(warnings.total(), 1, "{what} gave no warning: «{line}»");
            assert!(
                settings.colors.palette[1].is_some() && settings.colors.palette[2].is_some(),
                "{what} took the good lines next to it down with it"
            );
        }
    }

    #[test]
    fn the_twenty_two_colors_reach_the_theme() {
        // **It asserts over the color the theme returns, with the literal value written in the test.**
        // Comparing it against `Theme::from_colors(...)` again would measure that the code is consistent with itself:
        // crossing two colors would leave both sides of the equality changed and the test green. This
        // project already has that disguise on record, with 525 tests surviving a crossing of colors.
        let text = "color.0 = #010101\ncolor.15 = #0f0f0f\nbackground = #020202\nforeground = #030303\n\
                     cursor = #040404\ndivider = #050505\nfocus = #060606\npanel = #070707";
        let (settings, warnings) = parsed(text);
        assert_eq!(
            warnings.total(),
            0,
            "one of the eight lines was not understood"
        );

        let theme = crate::theme::Theme::from_colors(&settings.colors);

        assert_eq!(
            theme.color(term_core::TermColor::Indexed(0)),
            Color32::from_rgb(1, 1, 1),
            "color 0 of the palette did not reach the theme"
        );
        assert_eq!(
            theme.color(term_core::TermColor::Indexed(15)),
            Color32::from_rgb(0x0F, 0x0F, 0x0F),
            "color 15 did not reach: the palette is being copied halfway"
        );
        assert_eq!(
            theme.background(),
            Color32::from_rgb(2, 2, 2),
            "the background did not reach"
        );
        assert_eq!(
            theme.foreground(),
            Color32::from_rgb(3, 3, 3),
            "the text did not reach"
        );
        assert_eq!(
            theme.divider(),
            Color32::from_rgb(5, 5, 5),
            "the divider did not reach"
        );
        assert_eq!(
            theme.focus(),
            Color32::from_rgb(6, 6, 6),
            "the focus did not reach"
        );
        assert_eq!(
            theme.panel_background(),
            Color32::from_rgb(7, 7, 7),
            "the panel's background did not reach"
        );
        // **The cursor is asserted through where the painter reads it, and it was the only one of the six that no one
        // checked.** Measured: changing its assignment to the focus's left the 718 tests of `app`
        // green, so the defect and the fix gave the same result. And it goes through `TermColor`
        // because it is the door the painter asks for that color through, not through a field: asserting over the
        // field would check that the value is noted down, not that it is used.
        assert_eq!(
            theme.color(term_core::TermColor::Cursor),
            Color32::from_rgb(4, 4, 4),
            "the cursor's color did not reach"
        );
    }

    #[test]
    fn requesting_a_single_color_does_not_erase_the_other_fifteen() {
        // **It is what allows changing one color without writing the twenty-two**, and the failure mode is that the
        // palette is replaced whole instead of slot by slot: fifteen blacks would remain.
        let (settings, _) = parsed("color.1 = #ff0000");
        let theme = crate::theme::Theme::from_colors(&settings.colors);

        assert_eq!(
            theme.color(term_core::TermColor::Indexed(1)),
            Color32::from_rgb(0xFF, 0, 0),
            "the requested color was not applied"
        );
        assert_eq!(
            theme.color(term_core::TermColor::Indexed(2)),
            crate::theme::Theme::default().color(term_core::TermColor::Indexed(2)),
            "requesting one color changed another: the palette is being replaced whole"
        );
    }

    #[test]
    fn explicit_colors_override_the_named_theme_in_either_line_order() {
        for text in [
            "theme = tokyonight\ncolor.4 = #010203",
            "color.4 = #010203\ntheme = tokyonight",
        ] {
            let (settings, warnings) = parsed(text);
            assert_eq!(warnings.total(), 0);
            let theme = settings.resolved_theme();
            assert_eq!(
                theme.color(term_core::TermColor::Indexed(4)),
                Color32::from_rgb(1, 2, 3),
                "explicit blue was lost for {text:?}"
            );
            assert_eq!(
                theme.color(term_core::TermColor::Indexed(1)),
                Color32::from_rgb(0xF7, 0x76, 0x8E),
                "another slot stopped coming from Tokyo Night for {text:?}"
            );
        }
    }

    #[test]
    fn overriding_named_blue_preserves_every_other_named_color() {
        let base = crate::theme::NamedTheme::TokyoNight.theme();
        for text in [
            "theme = tokyonight\ncolor.4 = #010203",
            "color.4 = #010203\ntheme = tokyonight",
        ] {
            let (settings, warnings) = parsed(text);
            assert_eq!(warnings.total(), 0);
            let actual = settings.resolved_theme();
            assert_eq!(
                actual.color(term_core::TermColor::Indexed(4)),
                Color32::from_rgb(1, 2, 3),
                "explicit blue was lost for {text:?}"
            );
            for index in (0..16).filter(|index| *index != 4) {
                assert_eq!(
                    actual.color(term_core::TermColor::Indexed(index)),
                    base.color(term_core::TermColor::Indexed(index)),
                    "ANSI color {index} changed for {text:?}"
                );
            }
            for (slot, actual, expected) in [
                ("background", actual.background(), base.background()),
                ("foreground", actual.foreground(), base.foreground()),
                (
                    "cursor",
                    actual.color(term_core::TermColor::Cursor),
                    base.color(term_core::TermColor::Cursor),
                ),
                ("divider", actual.divider(), base.divider()),
                ("focus", actual.focus(), base.focus()),
                ("panel", actual.panel_background(), base.panel_background()),
            ] {
                assert_eq!(actual, expected, "{slot} changed for {text:?}");
            }
        }
    }

    #[test]
    fn an_unknown_theme_warns_without_erasing_valid_settings() {
        let (settings, warnings) =
            parsed("theme = tokyonight\ntheme = unknown\nforeground = #abcdef");
        assert_eq!(settings.theme, Some(crate::theme::NamedTheme::TokyoNight));
        assert_eq!(warnings.total(), 1);
        assert_eq!(
            settings.resolved_theme().background(),
            Color32::from_rgb(0x1A, 0x1B, 0x26)
        );
        assert_eq!(
            settings.resolved_theme().foreground(),
            Color32::from_rgb(0xAB, 0xCD, 0xEF)
        );

        let (settings, warnings) = parsed(
            "theme = tokyonight\ntheme = unknown\nforeground = #abcdef\ntheme = gruvbox-dark",
        );
        assert_eq!(settings.theme, Some(crate::theme::NamedTheme::GruvboxDark));
        assert_eq!(warnings.total(), 2);
        assert_eq!(
            warnings.kept()[0].to_string(),
            "line 2: 'unknown' is not a known theme; choose 'tokyonight' or 'gruvbox-dark'"
        );
        assert_eq!(
            warnings.kept()[1].to_string(),
            "line 4: 'theme' was already set above; this one wins"
        );
        let theme = settings.resolved_theme();
        assert_eq!(theme.foreground(), Color32::from_rgb(0xAB, 0xCD, 0xEF));
        assert_eq!(theme.background(), Color32::from_rgb(0x28, 0x28, 0x28));
    }

    #[test]
    fn theme_names_ignore_ascii_case_and_no_name_keeps_the_factory() {
        let (settings, warnings) = parsed("theme = TokyoNight");
        assert_eq!(warnings.total(), 0);
        assert_eq!(settings.theme, Some(crate::theme::NamedTheme::TokyoNight));
        assert_eq!(
            settings.resolved_theme().background(),
            Color32::from_rgb(0x1A, 0x1B, 0x26)
        );

        let (settings, _) = parsed("color.4 = #010203");
        assert_eq!(settings.theme, None);
        assert_eq!(
            settings.resolved_theme().background(),
            crate::theme::Theme::default().background()
        );
    }

    /// A directory of its own per test, with the process number.
    ///
    /// **The suite runs in parallel and the file's name is fixed**, so two tests that
    /// shared a directory would read each other's configuration and would fail only sometimes. It is
    /// the same mold the sibling module uses, and for the same reason.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "agent-terminal-settings-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create the test directory");
            Self(path)
        }

        fn with(&self, contents: &[u8]) -> PathBuf {
            let path = self.0.join(FILE);
            std::fs::write(&path, contents).expect("write the test file");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_that_goes_over_the_ceiling_is_not_read() {
        // **The ceiling exists because the file has arbitrary size**, and reading it without a cap is reserving without
        // a cap at startup. Without this test, the ceiling could be lowered to one byte with the whole suite
        // green — checked by a review with seven live mutations on this path.
        let dir = TestDir::new("ceiling");
        let exact = dir.with(&vec![b'#'; MAX_BYTES]);
        assert!(
            read(&exact).is_ok(),
            "a file of exactly the ceiling has to be read"
        );

        let over = dir.with(&vec![b'#'; MAX_BYTES + 1]);
        assert_eq!(
            read(&over),
            Err(ConfigProblem::TooBig),
            "a file one byte over the ceiling was read all the same"
        );
    }

    #[test]
    fn the_ceiling_and_the_warning_cap_are_the_ones_the_document_says() {
        // **Both constants are published in `docs/08-configuration.md`**, and the tests that use them
        // are parameterized **with themselves**: moving the number took nothing down while the document
        // kept promising the old one. It is the disguise this project has on record —choosing a number with
        // which the defect and the fix give the same— and they need to be written literal once.
        assert_eq!(MAX_BYTES, 64 * 1024, "the document promises 64 KiB");
        assert_eq!(
            MAX_WARNINGS, 20,
            "the document promises up to twenty warnings"
        );
    }

    /// **The color and size keys live twice**: in the `match` of [`apply`] and in the document's
    /// table, written by hand in both places and with nothing to confront them. The shortcuts already
    /// had their two guards; these eight had none, so renaming them on one side left
    /// a configuration file published that the application rejects — which is exactly the failure that
    /// the contract's document describes for the shortcuts, in the list next to it.
    ///
    /// **The document is read, the table is not copied.** A test that repeated the keys fixes
    /// itself when someone translates them in the code and in the test and forgets the document, which is the
    /// case that has to be caught. The opposite direction —a key the code accepts and the document
    /// keeps quiet about— cannot be derived: the `match` cannot be iterated, and copying it here would be having
    /// the list twice again.
    #[test]
    fn the_document_publishes_keys_the_application_accepts() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuration.md");

        let table = markdown::table(DOCUMENT, &["Key", "What it is"]);
        let published: Vec<&str> = table
            .column("Key")
            .into_iter()
            .filter_map(|cell| cell.split('`').nth(1))
            .collect();
        // **Without a floor this test passes by being empty**, which is how a guard that reads another
        // file dies: someone changes the table's header, the walk does not hook, and nothing is
        // left to check. Eleven is what the table publishes today.
        assert_eq!(
            published.len(),
            11,
            "the document's key table gave {} rows: either the document changed shape, or this \
             walk stopped finding it and the test is not looking at anything — {published:?}",
            published.len()
        );

        for key in published {
            // One of these values must really apply; merely avoiding UnknownKey would admit a
            // published key whose documented value is rejected.
            let known = ["#ff0000", "14", "on", "tokyonight"]
                .iter()
                .any(|value| apply(&mut Settings::default(), key, value).is_ok());
            assert!(
                known,
                "the document publishes «{key}» and the application rejects it as unknown: whoever follows the \
                 document would write a line that does nothing"
            );
        }
    }

    /// **And the examples the document gives to copy are really interpreted**, not just their keys.
    ///
    /// The guard next to it reads **the table**, so it leaves out what is most copied: the example
    /// blocks. Measured before writing this: changing the document's `key.` prefix to another, or the
    /// value `none` to whatever, left the 873 tests green — and they are the two halves this
    /// delivery just renamed. Here each `key = value` line of each block is passed **to
    /// the real parser**, which is what the terminal will do with the file someone pastes.
    ///
    /// **It goes by example blocks and not by backticks, and that avoids a list of exceptions.**
    /// The prose names `key = value` as a form, which is no key; an example block, on the
    /// other hand, is literally what someone copies and pastes, so there are no placeholders
    /// to exempt.
    #[test]
    fn the_document_examples_are_interpreted_without_a_single_warning() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuration.md");

        let examples: Vec<&str> = markdown::fenced(DOCUMENT)
            .code
            .into_iter()
            // The lines of the blocks that are not `key = value` —the on-screen messages— are not
            // configuration examples and are not passed to the parser.
            .filter(|line| line.contains(" = ") && !line.starts_with("agent-terminal:"))
            .collect();
        // Same floor and for the same reason as its sibling guard: if the walk stops hooking the
        // blocks, this test passes by being empty.
        assert_eq!(
            examples.len(),
            12,
            "the document's example blocks gave {} configuration lines: either the document \
             changed shape, or this walk stopped finding them — {examples:?}",
            examples.len()
        );

        for example in examples {
            let (settings, warnings) =
                interpret(example).expect("a single line cannot bring down the whole file");
            assert_eq!(
                warnings.total(),
                0,
                "the document gives «{example}» as an example and the terminal rejects it: {}",
                warnings.kept()[0]
            );
            // **And it asserts that it did something**, not just that it did not complain: an unknown key that stopped
            // warning would pass this test without applying anything, which is how a guard stays green
            // without checking anything. The exception is `version`, which **stores no value** on purpose:
            // it is a veto that is checked and forgotten, and that is why its example cannot change anything.
            if !example.starts_with("version") {
                assert_ne!(
                    settings,
                    Settings::default(),
                    "the document gives «{example}» as an example and it changes no setting"
                );
            }
        }
    }

    /// Spanish function words, to judge a sentence that appears on screen.
    ///
    /// **It is the same criterion the glossary guard picks its own with**, and for its same
    /// reason: they are short, the list is closed, and none of them is also an English word. That is why
    /// «no» and «sin» are left out, which are. They are compared as a whole word, so `none` is not «no».
    /// **The second half is content words, and it was added because the first half was shown to be
    /// evadable.** «no lleva ninguna pestana», «no tiene texto UTF-8 valido» and «no cabe» carry not one
    /// function word from the list: a negation plus content words walks straight through, and «no» and
    /// «sin» cannot join because they are English words too. The criterion is unchanged — closed, short,
    /// and no entry is also an English word — only widened past function words.
    const SPANISH_FUNCTION_WORDS: &[&str] = &[
        "de",
        "del",
        "la",
        "el",
        "los",
        "las",
        "lo",
        "que",
        "con",
        "por",
        "para",
        "una",
        "uno",
        "al",
        "mas",
        "ya",
        "se",
        "es",
        "hay",
        "ninguna",
        "ninguno",
        "lleva",
        "tiene",
        "cabe",
        "texto",
        "valido",
        "pestana",
        "fichero",
        "disposicion",
        "guardar",
        "pudo",
        "puede",
        "son",
        "esta",
        "estan",
        "sido",
        "textos",
        "invalido",
        "pestanas",
        "ficheros",
        "disposiciones",
    ];

    /// The first Spanish word in that text, if there is one.
    fn spanish_word_in(text: &str) -> Option<&str> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .find(|word| SPANISH_FUNCTION_WORDS.contains(&word.to_ascii_lowercase().as_str()))
    }

    /// **The document publishes three on-screen messages, word for word, and nothing confronted them
    /// with the code.** They are what someone pastes into a search engine when their file does not do what
    /// they expected, so translating them in the code and not in the document —or the other way around— leaves published a
    /// sentence the terminal never says. It is the same coupling as the key table
    /// above, in the half that one leaves out **on purpose**: its walk discards the lines
    /// that start with `agent-terminal:`, and until here no one picked them up.
    ///
    /// **The two line warnings are reproduced by the real parser**, writing on the line
    /// the document points to what the document tells happened there. The third is printed rather
    /// than returned, and printing cannot be observed from inside the process, so the only thing
    /// that can be confronted without copying it again is **its template in the source**. That
    /// source is this file: putting the settings onto the window left `lib.rs` with the delivery
    /// that made the cursor blink, and the sentence went with it.
    #[test]
    fn the_messages_the_document_publishes_are_the_ones_the_terminal_writes() {
        const DOCUMENT: &str = include_str!("../../../docs/08-configuration.md");
        const WINDOW: &str = include_str!("settings.rs");

        let published: Vec<&str> = markdown::fenced(DOCUMENT)
            .code
            .into_iter()
            .filter(|line| line.starts_with("agent-terminal: "))
            .collect();
        // Same floor and for the same reason as the two sibling guards above: without it, a document
        // that stopped publishing messages leaves this test passing by looking at nothing.
        assert_eq!(
            published.len(),
            3,
            "the document publishes {} on-screen messages: either it changed shape, or this walk stopped \
             finding them and the test is not looking at anything — {published:?}",
            published.len()
        );

        // **And the three have to be in English, here and not only in the code.** Without this, returning the
        // message AND its line in the document to Spanish passes both guards: the sentences would keep
        // coinciding with each other, which is the only thing the rest of this test checks.
        for line in &published {
            assert!(
                line.is_ascii(),
                "the document publishes «{line}» with a character that is not ASCII: the angle \
                 quotes and the accents only come from the Spanish version of that message"
            );
            assert_eq!(
                spanish_word_in(line),
                None,
                "the document publishes «{line}», which is still in Spanish"
            );
        }

        // A line warning is reproduced by putting its line where the document says it was: the
        // number is part of the message, so seeding it on another line no longer reproduces it.
        let written_by_the_terminal = |at: usize, text: &str| {
            let file = format!("{}{text}", "\n".repeat(at - 1));
            let (_, warnings) = parsed(&file);
            assert_eq!(
                warnings.kept().len(),
                1,
                "«{text}» had to give exactly one warning and gave {}",
                warnings.kept().len()
            );
            format!("agent-terminal: {}", warnings.kept()[0])
        };
        let ours = [
            written_by_the_terminal(3, "key.copy = ctrl+shift+e"),
            written_by_the_terminal(7, "color.3 = purple"),
        ];

        for said in &ours {
            assert!(
                published.contains(&said.as_str()),
                "the terminal writes «{said}» and the document does not publish that sentence: whoever searches for it \
                 just as it appears on screen will not find the document — {published:?}"
            );
        }
        // **And the opposite direction**, which is the one that catches a new message published without a guard: every
        // line warning the document shows has to be one of the ones just reproduced.
        for line in published.iter().filter(|line| line.contains(": line ")) {
            assert!(
                ours.iter().any(|said| said == line),
                "the document publishes «{line}» and no terminal warning comes out like that"
            );
        }

        let clash = published
            .iter()
            .find(|line| !line.contains(": line "))
            .expect("the document publishes the warning about the two shortcuts that clash");
        // The two halves the warning interpolates are action names, and they have to really be so.
        for name in clash.split('\'').skip(1).step_by(2) {
            assert!(
                crate::keyboard::Action::from_written(name).is_some(),
                "the document shows «{name}» in that warning and it is no action that can be bound"
            );
        }
        // Back to the template: what is quoted is what the window interpolates, and the rest is the
        // fixed sentence that has to be written the same in both places.
        let template = clash
            .split('\'')
            .enumerate()
            .map(|(nth, piece)| {
                if nth % 2 == 1 {
                    "{}".to_owned()
                } else {
                    piece.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("'");
        // **A string split in the source carries `\` at the end of the line**, which swallows the break and the
        // indentation of the next one. Undoing it here is what makes this guard survive
        // `cargo fmt` readjusting that line, instead of falling because of it.
        let mut source = String::with_capacity(WINDOW.len());
        let mut rest = WINDOW;
        while let Some(cut) = rest.find("\\\n") {
            source.push_str(&rest[..cut]);
            rest = rest[cut + 2..].trim_start();
        }
        source.push_str(rest);
        assert!(
            source.contains(&template),
            "the document publishes «{clash}» and the window does not write that sentence: the template \
             «{template}» was searched for"
        );
    }

    /// **The fixed sentence of each message is the half of AC6 that no test looked at.** The guards of
    /// this suite assert over what the warning **interpolates** —the key, the value, the line
    /// number—, and where they quote the sentence they quote a fragment. Returning the thirteen messages of the
    /// two types to Spanish left the whole suite green: checked before writing this.
    ///
    /// **Function words and not a wide lexicon**, which is the same criterion the glossary
    /// guard picks its own with: they are short, the list is closed and none of them is also an
    /// English word. That is why «no» and «sin» are left out, which are, even though the thirteen Spanish sentences
    /// carried them.
    ///
    /// The non-ASCII oracle goes alongside because the angle quotes and the accents are free to
    /// check, not because it suffices: four of the Spanish messages were pure ASCII.
    #[test]
    fn no_settings_message_was_left_in_spanish() {
        let problems = [
            ConfigProblem::Absent,
            ConfigProblem::BrokenLink,
            ConfigProblem::NotAFile,
            ConfigProblem::Unreadable("permission denied".to_owned()),
            ConfigProblem::TooBig,
            ConfigProblem::NotText,
            ConfigProblem::Version(9),
        ];
        let faults = [
            Missing::NoEquals,
            Missing::Incomplete,
            Missing::UnknownKey("size".to_owned()),
            Missing::InvalidValue {
                key: "size".to_owned(),
                value: "abc".to_owned(),
            },
            Missing::UnknownTheme("unknown".to_owned()),
            Missing::Repeated("size".to_owned()),
            Missing::OnlyTurnsOff("key.copy".to_owned()),
        ];
        // **The two `match` are exhaustive and without a wildcard, and they are not redundant.** They are the only thing that forces
        // a new variant to pass through this guard: the two lists above are literals, so without
        // them adding a message would slip through silently and the test would stay green.
        for problem in &problems {
            match problem {
                ConfigProblem::Absent
                | ConfigProblem::BrokenLink
                | ConfigProblem::NotAFile
                | ConfigProblem::Unreadable(_)
                | ConfigProblem::TooBig
                | ConfigProblem::NotText
                | ConfigProblem::Version(_) => {}
            }
        }
        for fault in &faults {
            match fault {
                Missing::NoEquals
                | Missing::Incomplete
                | Missing::UnknownKey(_)
                | Missing::InvalidValue { .. }
                | Missing::UnknownTheme(_)
                | Missing::Repeated(_)
                | Missing::OnlyTurnsOff(_) => {}
            }
        }

        let said: Vec<String> = problems
            .iter()
            .map(ToString::to_string)
            .chain(
                faults
                    .into_iter()
                    .map(|missing| Warning { line: 7, missing }.to_string()),
            )
            .collect();
        assert_eq!(
            said.len(),
            14,
            "the fourteen on-screen messages were not walked: {said:?}"
        );

        for message in said {
            assert!(
                message.is_ascii(),
                "«{message}» carries a character that is not ASCII: the angle quotes and the accents \
                 only come from the Spanish version of this message"
            );
            assert_eq!(
                spanish_word_in(&message),
                None,
                "the message «{message}» is still in Spanish, and it is what appears on screen"
            );
        }
    }

    /// **The sibling above walks the thirteen settings messages, and until here nothing walked the rest.**
    /// Measured: replacing every on-screen sentence of `session` and `watcher` with a placeholder left the
    /// whole suite green — they are what someone reads when their layout file is corrupt, the watcher
    /// cannot arm, or the window will not split, and no test asserted one word of them.
    ///
    /// **Two halves, because one alone does not cover it.**
    ///
    /// The first walks the two enums and **the `match` produces the index**, which is the half a
    /// wildcard-free `match` looks like it gives and does not: arms enumerated beside a hand-written array
    /// are satisfied by adding an arm and leaving the variant out of the array. Checked by doing exactly
    /// that — a duplicate variant in the array kept the count at its number and the suite green. Producing
    /// the index and demanding the set be whole is what makes the array's omission fail.
    ///
    /// The second walks **every sentence-shaped literal of the three files, from the source**, and not
    /// the arguments of `eprintln!`. Checked: scraping that one macro missed `go_blind`'s template, its
    /// two reasons and the five prefixes of `session`, and reverting any of them to Spanish left the
    /// test green. Reading the literals instead cannot be evaded by moving a sentence to `writeln!` or
    /// `format!`.
    ///
    /// **`lib.rs` joined the walk with the delivery that translated it**, which closes the hole this
    /// doc used to name: `SaveFailure::WontOverwrite` renders
    /// `"what is saved is not overwritten: {reason}"` and the template was checked here, but the
    /// `reason` is a `&'static str` handed in by `lib.rs` (`forbid_overwrite`), so the sentence the
    /// user reads was half checked and half not. Both halves are walked now — and with them the other
    /// 45 sentences that file prints, which nothing had ever looked at.
    ///
    /// **What the oracle cannot do, with the sentences that prove it.** It was attacked deliberately and
    /// three ASCII-folded Spanish sentences walked through: «cero pestanas abiertas» — a **plural** of a
    /// listed word, since the comparison is whole-word, and that class is closed now — plus «formato
    /// desconocido» and «ruta inalcanzable», which carry **no word from the list at all**. Those two
    /// still pass, and no closed list catches them: one long enough would stop being closed and would
    /// start colliding with English.
    ///
    /// So what a green here means, exactly: no accented or `ñ`-carrying Spanish, and none built with the
    /// common function words. What it does **not** mean is that the sentence is English. Real Spanish
    /// carries accents —«pestañas», «válido»— and the non-ASCII half catches those on its own; folding
    /// them to ASCII on purpose to slip past a test is not an accident anybody has. «no» and «sin»
    /// cannot join the list, being English words too.
    #[test]
    fn no_layout_or_watcher_message_was_left_in_spanish() {
        use crate::session::{LayoutProblem, SaveFailure};
        use std::collections::BTreeSet;

        let io = || std::io::Error::other("io");
        let problems = [
            LayoutProblem::Absent,
            LayoutProblem::NotAFile,
            LayoutProblem::Unreadable("io".to_owned()),
            LayoutProblem::NotText,
            LayoutProblem::Unintelligible("json".to_owned()),
            LayoutProblem::Version { found: 9 },
            LayoutProblem::NoTabs,
            LayoutProblem::SingleChildBranch { tab: 0 },
            LayoutProblem::NestedBranches { tab: 0 },
            LayoutProblem::InvalidWeight { tab: 0 },
            LayoutProblem::NonFiniteSum { tab: 0 },
            LayoutProblem::TooManyLeaves { count: 999 },
            LayoutProblem::TooLarge { bytes: 1 },
        ];
        let failures = [
            SaveFailure::NoPlace,
            SaveFailure::Vetoed { found: 9 },
            SaveFailure::NotSerializable,
            SaveFailure::Directory(io()),
            SaveFailure::TempFile(io()),
            SaveFailure::Writing(io()),
            SaveFailure::Renaming(io()),
            SaveFailure::WontOverwrite("name"),
            SaveFailure::DoesntFit,
        ];

        // **The index comes out of the `match`, and that is the whole point.** Numbering by hand here
        // would let the array and the arms drift apart in silence; taking the number from the variant
        // means an array that skips one leaves a hole in the set below.
        let problem_index = |problem: &LayoutProblem| match problem {
            LayoutProblem::Absent => 0,
            LayoutProblem::NotAFile => 1,
            LayoutProblem::Unreadable(_) => 2,
            LayoutProblem::NotText => 3,
            LayoutProblem::Unintelligible(_) => 4,
            LayoutProblem::Version { .. } => 5,
            LayoutProblem::NoTabs => 6,
            LayoutProblem::SingleChildBranch { .. } => 7,
            LayoutProblem::NestedBranches { .. } => 8,
            LayoutProblem::InvalidWeight { .. } => 9,
            LayoutProblem::NonFiniteSum { .. } => 10,
            LayoutProblem::TooManyLeaves { .. } => 11,
            LayoutProblem::TooLarge { .. } => 12,
        };
        let failure_index = |failure: &SaveFailure| match failure {
            SaveFailure::NoPlace => 0,
            SaveFailure::Vetoed { .. } => 1,
            SaveFailure::NotSerializable => 2,
            SaveFailure::Directory(_) => 3,
            SaveFailure::TempFile(_) => 4,
            SaveFailure::Writing(_) => 5,
            SaveFailure::Renaming(_) => 6,
            SaveFailure::WontOverwrite(_) => 7,
            SaveFailure::DoesntFit => 8,
        };

        let seen: BTreeSet<usize> = problems.iter().map(problem_index).collect();
        assert_eq!(
            seen,
            (0..13).collect::<BTreeSet<usize>>(),
            "a layout problem is missing from the list above, so its sentence is not being looked at"
        );
        let seen: BTreeSet<usize> = failures.iter().map(failure_index).collect();
        assert_eq!(
            seen,
            (0..9).collect::<BTreeSet<usize>>(),
            "a save failure is missing from the list above, so its sentence is not being looked at"
        );

        // **Every sentence-shaped literal of the two files, taken from the source.** A sentence is a
        // literal with a space in it: that is what a person reads and what a translation forgets.
        //
        // **It stops at `mod tests`, and that boundary is the oracle's and not laziness.** A test
        // message is prose but it is not on screen, and it uses the project's angle quotes for
        // emphasis — which the non-ASCII half would report as Spanish. Checked: without the cut, the
        // walk failed on an English assertion message quoting «not measured».
        /// The one character this crate writes in English and is not ASCII: the rule that opens
        /// each pane's block in the measurement report. Flattening it would change a published
        /// artifact to satisfy a test, which is the tail wagging the dog.
        const DRAWN_RULE: char = '\u{2500}';

        const SOURCES: [(&str, &str); 21] = [
            ("settings/document.rs", include_str!("settings/document.rs")),
            ("settings/store.rs", include_str!("settings/store.rs")),
            ("settings/editor.rs", include_str!("settings/editor.rs")),
            (
                "settings/editor/view.rs",
                include_str!("settings/editor/view.rs"),
            ),
            (
                "settings/editor/timing.rs",
                include_str!("settings/editor/timing.rs"),
            ),
            ("fonts/startup.rs", include_str!("fonts/startup.rs")),
            ("session.rs", include_str!("session.rs")),
            ("watcher.rs", include_str!("watcher.rs")),
            ("lib.rs", include_str!("lib.rs")),
            ("header/splitting.rs", include_str!("header/splitting.rs")),
            ("header/controls.rs", include_str!("header/controls.rs")),
            ("header/menu.rs", include_str!("header/menu.rs")),
            // **They arrived because this walk noticed them leaving**, twice now. D28 took the
            // bench's injected gestures out of `lib.rs` and six sentence-shaped literals went with
            // them; the tiling and the tree left next and took thirteen more, dropping the walk to
            // 63 against a floor of 74. A module split silently narrows every list that names
            // files, which is why the guard counts and does not just judge.
            ("injected.rs", include_str!("injected.rs")),
            // **And a third time, with the milestone that gives the panel buttons**: the keys' walk
            // and what a pane does with the wheel left `lib.rs` for these two, taking three
            // sentence-shaped literals with them. The corpus fell from 76 to 73 and the floor of 72
            // swallowed it in silence — the exact failure this comment had already described twice
            // and the list still did not defend against, because a list only defends the files it
            // names.
            ("keyboard.rs", include_str!("keyboard.rs")),
            ("mouse.rs", include_str!("mouse.rs")),
            ("pointer.rs", include_str!("pointer.rs")),
            ("report.rs", include_str!("report.rs")),
            // The scrollback's search speaks on its own row over a console (D61), and brought eight
            // sentence-shaped literals with it.
            ("search.rs", include_str!("search.rs")),
            ("tab_bar.rs", include_str!("tab_bar.rs")),
            ("tiling.rs", include_str!("tiling.rs")),
            ("tree.rs", include_str!("tree.rs")),
        ];
        let mut sentences: Vec<(&str, String)> = Vec::new();
        for (name, source) in SOURCES {
            // **The continuations are rejoined before walking, and that is not tidiness.** A literal
            // that rustfmt wrapped with a trailing `\\` leaves its second half between an odd pair of
            // quotes, where a per-line walk reads it as code and skips it. `session.rs` carries three
            // today —the two long cut messages and the version veto— and only their first halves were
            // being looked at.
            let rejoined = source.replace("\\\n", "");
            for line in rejoined.lines() {
                let code = line.trim_start();
                if code.starts_with("mod tests") || code.starts_with("pub mod tests") {
                    break;
                }
                if code.starts_with("//") || code.starts_with("*") {
                    continue;
                }
                for piece in line.split('"').skip(1).step_by(2) {
                    if piece.contains(' ') && piece.len() >= 12 {
                        sentences.push((name, piece.to_owned()));
                    }
                }
            }
        }
        // Same floor and for the same reason as its siblings: without it a walk that stopped finding
        // literals leaves this test passing by looking at nothing. The twelve files hold **88**
        // today — 27 in `session.rs`, 10 each in `lib.rs` and `tree.rs`, 9 in `injected.rs`, 8 in
        // `search.rs`, 5 each in `watcher.rs`, `keyboard.rs` and `report.rs`, 3 each in
        // `pointer.rs` and `tiling.rs`, 2 in `mouse.rs` and 1 in `tab_bar.rs` — and the floor sits
        // five below, tight enough that a broken walk cannot hide behind it and loose enough that
        // deleting a message is not a failure. (It sat at 74 against a corpus that had grown to 80
        // when the search's delivery measured it again; the floor moves with the corpus.)
        //
        // **It was left at 74 when the corpus dropped from 78 to 76 and the claim above stopped
        // being true**: at two lines of slack, deleting a pair of messages *is* a failure. The floor
        // moves with the corpus or it stops meaning what it says.
        //
        // **The cut at `mod tests` rests on there being nothing after it, and for one of the files
        // walked here that is FALSE** — it read «there is not» until somebody checked. Six of this
        // crate's modules declare no test module at all; of the ones that do, `pty_output.rs` keeps
        // `DeafConsole` below its own and is not walked here, while **`keyboard.rs` now is**, and it
        // keeps `Action`, `Chord` and `Keymap` below its own — about a hundred and sixty production
        // lines this walk therefore does not read. **`shape.rs` cannot see those lines either**,
        // which is the same invisibility D28 records as costing one delivery fifty lines, and it is
        // left rather than fixed; the one guard that could not live with it — the one counting the
        // doors to the control-character gate — removes the test blocks by counting braces instead
        // of truncating, and its own comment says why. Whenever a top-level item lands below a test
        // module its sentences leave this walk in silence, which is why the floor is stated as a
        // number and not as «some».
        assert!(
            sentences.len() >= 83,
            "only {} sentence-shaped literals were found before `mod tests` in the listed files: the \
             walk stopped working",
            sentences.len()
        );
        // **Two floors and not one**, the lesson `COMMENT_LINES_AT_LEAST` in `tests/prose.rs`
        // already paid for and this walk had only half of. A total floor answers «the walk still
        // works»; it cannot answer «this file is still being read», because the slack above it is
        // wider than any one file. Measured as the search's delivery leaves the tree: **seven of
        // the twelve files sit inside the total floor's five lines of slack** — `watcher.rs`,
        // `keyboard.rs` and `report.rs` with five, `pointer.rs` and `tiling.rs` with three,
        // `mouse.rs` with two and `tab_bar.rs` with one — and are defended by nothing else at all.
        //
        // **That is the failure this list has now met three times, and the total floor swallowed it
        // every time**: a split moves a file's messages into a module nobody adds here, the corpus
        // drops by less than the slack, and the guard stays green over sentences no oracle reads
        // any more. Checked, when the floor was 74, by doing exactly that — dropping `mouse.rs` from
        // the list above left it green at 77.
        //
        // One and not more: `tab_bar.rs` carries a single sentence, so any higher number would be a
        // table rather than a floor — the same reason its sibling gives.
        for (name, _) in SOURCES {
            assert!(
                sentences.iter().any(|(from, _)| from == &name),
                "{name} yielded no sentence-shaped literal: either its messages left for a module \
                 that was never added to the list above, or the walk stopped matching for this \
                 file alone — which no total would notice"
            );
        }

        let said: Vec<String> = problems
            .iter()
            .map(ToString::to_string)
            .chain(failures.iter().map(ToString::to_string))
            .collect();
        assert_eq!(
            said.len(),
            22,
            "the twenty-two rendered sentences were not walked: {said:?}"
        );

        for message in said
            .iter()
            .map(|rendered| ("rendered", rendered.clone()))
            .chain(sentences)
        {
            let (where_from, message) = message;
            // **One character is allowed not to be ASCII, and it is a horizontal rule.** Adding
            // `lib.rs` to this walk brought in three English sentences that are not ASCII, and the
            // first attempt exempted that file from `is_ascii()` altogether. **That was a hole, and
            // it was demonstrated**: `is_ascii()` was carrying a third thing nobody had named —
            // « », — and the accents are all how this repository writes Spanish, and a sentence
            // like «esto» no responde; cierra su division y vuelve a intentarlo carries no listed
            // word and no accent, so it passed in `lib.rs` and failed in `session.rs`.
            //
            // Two of the three sentences did not need the exemption at all: the launch failure now
            // cites the program with ASCII quotes, the way `lib.rs` already cites a name elsewhere,
            // and the window-size warning writes `x` — which is what the measurement script folds
            // `×` to anyway. What is left is the report's rule, `── tab n, pane n ──`, whose
            // character is drawn and never Spanish.
            assert!(
                message.chars().all(|c| c.is_ascii() || c == DRAWN_RULE),
                "«{message}» ({where_from}) carries a character that is neither ASCII nor the \
                 report's rule: the angle quotes, the dashes and the accents are how this \
                 repository writes Spanish"
            );
            assert_eq!(
                spanish_word_in(&message),
                None,
                "the message «{message}» ({where_from}) is still in Spanish, and it is what appears \
                 on screen"
            );
        }
    }

    #[test]
    fn what_is_not_a_file_is_not_attempted_to_be_read() {
        let dir = TestDir::new("no-file");
        let as_directory = dir.0.join(FILE);
        std::fs::create_dir_all(&as_directory).expect("create the directory with the file's name");

        assert_eq!(
            read(&as_directory),
            Err(ConfigProblem::NotAFile),
            "a directory with the file's name was attempted to be read as configuration"
        );
    }

    #[test]
    fn a_file_that_is_not_text_leaves_the_terminal_at_factory() {
        // It is promised in writing in the document given to whoever writes the file.
        let dir = TestDir::new("no-text");
        let path = dir.with(&[0xFF, 0xFE, 0x00, 0x80]);

        assert_eq!(
            read(&path),
            Err(ConfigProblem::NotText),
            "some bytes that are not text were parsed all the same"
        );
    }

    #[test]
    fn the_file_that_is_not_there_keeps_quiet_and_the_broken_link_does_not() {
        // **The difference matters and no one saw it.** Whoever has not written configuration must not hear anything
        // on every startup; whoever wrote it and had it moved **has to find out**, and before it came out through
        // the same silent path. A review caught it.
        let dir = TestDir::new("absent");
        let never = dir.0.join(FILE);
        assert_eq!(read(&never), Err(ConfigProblem::Absent));
        assert!(
            !ConfigProblem::Absent.warrants_warning(),
            "whoever has configured nothing would be warned on every startup"
        );

        #[cfg(unix)]
        {
            let link = dir.0.join("link");
            std::os::unix::fs::symlink(dir.0.join("does-not-exist"), &link)
                .expect("create the link");
            assert_eq!(
                read(&link),
                Err(ConfigProblem::BrokenLink),
                "a broken link was mistaken for «there is no file»"
            );
            assert!(
                ConfigProblem::BrokenLink.warrants_warning(),
                "a broken link keeps quiet, and whoever created it does not find out that their configuration is not applied"
            );
        }
    }

    #[test]
    fn a_real_file_is_really_read() {
        // **None of the twenty previous tests called `read`**: they all entered through the parser,
        // so the whole disk path could be deleted with the suite green. A review caught it with
        // seven live mutations.
        let dir = TestDir::new("for-real");
        let path = dir.with(b"theme = tokyonight\ncolor.1 = #ff0000\nbad\n");

        let (settings, warnings) = read(&path).expect("the test file had to be read");
        assert_eq!(
            settings.colors.palette[1],
            Some(Color32::from_rgb(0xFF, 0, 0))
        );
        assert_eq!(
            settings.resolved_theme().background(),
            Color32::from_rgb(0x1A, 0x1B, 0x26)
        );
        assert_eq!(
            warnings.total(),
            1,
            "the bad line did not warn when reading from disk"
        );
    }

    #[test]
    fn a_measurement_still_vetoes_the_whole_settings_file() {
        let dir = TestDir::new("theme-measurement-veto");
        let path = dir.with(b"theme = tokyonight\n");
        let (settings, reading) = to_use(Entry::Measuring, Some(&path));
        assert_eq!(reading, crate::measure::ReadingSettings::Vetoed);
        assert_eq!(settings.theme, None);
        assert_eq!(
            settings.resolved_theme().background(),
            crate::theme::Theme::default().background()
        );
    }

    #[test]
    fn the_file_is_looked_for_where_the_document_says() {
        // **`path_from` was pulled out explicitly «to be able to test it without touching the environment», and it was not
        // tested**: pointing it at the layout file left the suite green. A review caught it.
        let path = path_from(Some(std::ffi::OsStr::new("/home/config")), None)
            .expect("an absolute path has to give a location");
        assert_eq!(
            path,
            PathBuf::from("/home/config/agent-terminal/config"),
            "the file is not looked for where the document says"
        );

        assert_eq!(
            path_from(None, Some(std::ffi::OsStr::new("/home"))),
            Some(PathBuf::from("/home/.config/agent-terminal/config")),
            "without the dedicated variable it does not fall back to the home's «.config»"
        );
        assert_eq!(
            path_from(None, Some(std::ffi::OsStr::new("relative"))),
            None,
            "a relative path was accepted: the file would end up inside the working directory"
        );
    }
}
