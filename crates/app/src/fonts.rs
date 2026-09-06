//! What font paints each character, and **with what presentation**.
//!
//! Two halves of the same question, and that is why they live together: **what face covers a character** and **with what
//! presentation it has to be painted**. Rasterizing the color glyph comes later, in this same module.
//!
//! ## Discovery is LAZY, and a measurement decided that
//!
//! The terminal **does not enumerate the machine's fonts when opening the window**. It looks for them the first time
//! a character appears that no registered face covers. Enumerating and registering at startup would cost between
//! **23.5 and 31.0 ms** —more than four times the only stretch of startup this project has measured— and
//! would be paid **always**; looking when needed costs between **4.0 and 5.7 ms** and is only paid if the
//! character actually appears. The numbers and the full subtraction are in **D24**.
//!
//! **With one exception, and it is asked for out loud.** A settings file that names a family —`font =
//! JetBrains Mono`— has that name resolved **at start-up**, before the first frame, and resolving a
//! name is exactly the enumeration above. That is the whole of the exception: the file has to say it.
//! A file that names no family, or no file at all, never reaches [`Discoverer::use_the_family`] and
//! the start-up is the one described in the paragraph above, bit for bit. The report declares which
//! of the two happened, so a measurement run that paid it cannot be mistaken for one that did not.

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;
use fontique::{Collection, CollectionOptions, GenericFamily, SourceCache};
use skrifa::MetadataProvider as _;
use unicode_properties::emoji::{
    EmojiStatus, UnicodeEmoji, is_emoji_presentation_selector, is_text_presentation_selector,
};

/// How a character has to be painted.
///
/// **It exists because `☁` and `☁️` are the same character.** The second carries the emoji selector behind it, and
/// painting the two the same is wrong in one direction or the other: either the emoji the program
/// asked for is lost, or a color drawing is painted where there was a text symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// By the usual text path, with the color the theme says.
    Text,
    /// As a drawing with its own colors, which the font carries inside.
    Color,
}

/// What presentation a character gets, given what it carries attached.
///
/// **The rule, and it is Ghostty's:** it goes color **if and only if** the emoji selector
/// (`U+FE0F`) follows it **and the character admits that selector**, **or** its default presentation is already emoji and
/// the text selector (`U+FE0E`) does **not** follow it. Everything else goes monochrome.
///
/// **The "and the character admits that selector" is the half that gets forgotten.** Unicode only defines
/// variation sequences for characters with `Emoji=Yes`; behind any other, the selector means
/// nothing and honoring it sends the color font an `a` or a blank space. And it is not a
/// far-fetched case: a loose selector —a string cut in half through an emoji— the engine attaches to the
/// cell in front of it, **even if it is empty**.
///
/// `attached` are the zero-width characters of the cell, just as the view gives them. Only
/// **the first** is looked at: a variation selector applies to the character right in front of it, so one that
/// appeared deeper in the sequence does not speak about this character.
///
/// **What this rule does NOT reach, and it has to be said:** the skin-tone modifiers
/// (`U+1F3FB`–`U+1F3FF`) are **wide**, so the engine gives them **their own cell** instead of attaching them.
/// In `☝🏽` the base `U+261D` arrives with **nothing attached**, and since its default presentation is text,
/// this rule sends it to monochrome — while the modifier, in the cell next to it, would go color.
/// Unicode and Ghostty treat the whole sequence as emoji presentation. **Resolving it demands looking at the
/// next cell, which is information this function does not have by signature**, and today it does not show because nobody
/// paints color yet: it is work for the delivery that brings it. A review found it.
pub fn presentation_of(c: char, attached: &[char]) -> Presentation {
    match attached.first().copied() {
        Some(selector) if is_emoji_presentation_selector(selector) && c.is_emoji_char() => {
            Presentation::Color
        }
        Some(selector) if is_text_presentation_selector(selector) => Presentation::Text,
        _ if goes_to_color_by_default(c) => Presentation::Color,
        _ => Presentation::Text,
    }
}

/// Whether the **default** presentation of the character is emoji, that is `Emoji_Presentation=Yes`.
///
/// **The variants are enumerated instead of asking with a method**, because the properties crate does not
/// carry one: it exposes `Emoji`, `Emoji_Component` and the combination of the two, but
/// `Emoji_Presentation` only appears **inside** the names of its enum. The four below are
/// exactly those whose documentation declares `Emoji_Presentation=YES`; it was checked by reading its
/// table, not by deducing it from the names. **Two the name gives away wrongly**: `EmojiModifierBase` and
/// `EmojiOtherAndEmojiComponent` look like candidates and are not.
///
/// **Its enum is open (`#[non_exhaustive]`), so a category they add tomorrow falls into the
/// case below and is painted monochrome**, without the compilation saying anything. It is left this way knowingly: that
/// character would come out **as it comes out today**, which is the worst acceptable case — inventing color for a category
/// we do not know would be the opposite of the rule that what is not there is not invented.
fn goes_to_color_by_default(c: char) -> bool {
    matches!(
        c.emoji_status(),
        EmojiStatus::EmojiPresentation
            | EmojiStatus::EmojiPresentationAndModifierBase
            | EmojiStatus::EmojiPresentationAndEmojiComponent
            | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
    )
}

// ── Font discovery ───────────────────────────────────────────────────────────────────────────────

/// The character by which the replacement box is recognized.
///
/// **`U+0378` is unassigned in Unicode and is OUTSIDE the private-use areas.** Both things
/// matter: unassigned, no font ever covers it —so its drawing **is** that of the box—; and outside
/// private use, it is always asked of the same family as the character it is compared with. A
/// referent from private use would be asked of the icon family as soon as there were one, and would be
/// comparing apples to oranges. A review pointed out the second.
const REPLACEMENT_BOX: char = '\u{0378}';

/// How much time is spent looking for fonts in a single round.
///
/// **The TIME is bounded and not the number of searches, because time is what is noticed.** What a
/// search costs depends on how much the winning font weighs and on how many families have to be opened first: the
/// measured range goes from **94 µs to 18 ms** (**D24**). A ceiling of "four searches" bounds four things that
/// can cost 0.4 ms or 72; a time ceiling bounds the only thing that matters.
///
/// Two milliseconds are a third of the frame (5.78 ms, **D3**) and are paid **only when there is something new
/// to look for**. What does not fit waits for the next frame, which is what the queue exists for.
///
/// **The first version put the ceiling on the waiting queue and not on the work**, and with that a `cat` of a
/// binary cost **6.0–6.6 ms sustained on every frame** —more than the whole frame—. Measured by
/// a review, in release, and confirmed here before changing it.
const BUDGET_PER_ROUND: std::time::Duration = std::time::Duration::from_millis(2);

/// How many searches in a row finding nothing make it stop looking.
///
/// **It is the brake for a broken program, and without it there is none.** A `cat` of a binary produces characters
/// that **no** font covers, so no search registers anything and no other ceiling gets
/// triggered: it would keep looking as long as garbage keeps coming out. With this, after a handful of failures
/// in a row the terminal stops trying — and those characters come out as a box, which is what they came out as
/// anyway.
///
/// **It resets as soon as a search succeeds**, so a normal session never reaches it.
const MAX_FAILURES_IN_A_ROW: usize = 12;

/// How many unresolved characters are kept while they wait for the next round.
///
/// **The ceiling is against a broken program, not against a user.** Whoever spits binary garbage produces
/// distinct characters by the hundreds per frame, and each one costs a whole search. With the ceiling, a
/// screen of garbage spends what forty characters spend and the rest comes out as a box — which is what
/// would have come out anyway. Without it, the window stays looking for fonts for characters nobody is going to
/// read.
const MAX_PENDING: usize = 40;

/// How many system families end up being registered with the text engine.
///
/// **The ceiling exists because there is no way to ask what the engine already covers.** `Fonts::has_glyph` is no
/// use —see `Discoverer::resolve`—, so it looks for everything non-ASCII and sometimes registers a
/// face that was not needed: it goes behind the factory ones and does not change what is seen, but **each one fattens
/// the glyph atlas and lengthens the fallback chain** that is traversed per character.
///
/// **Eight, and it is not a round number for taste:** it covers the cases this milestone pursues —CJK, hangul,
/// icons, emoji— with plenty of margin, and leaves the worst case bounded. On reaching the ceiling it stops
/// registering: whatever appears afterward comes out as a box, which is exactly what came out before the milestone.
///
/// **It counts what the terminal registers ON ITS OWN.** The family asked for by name in the settings
/// file is not one of these — it is one, it was asked for out loud, and it is the point of the
/// setting — so with a configured font the chain holds nine faces at its widest.
const MAX_FAMILIES: usize = 8;

/// The name of the family by which the icons of the private-use areas are requested.
///
/// **It exists because egui orders the fallback chain by whole family and not by range**, so the only
/// way for a font to win in some areas and not in others is to give it its own family and let the painter
/// choose which to request. See [`Discoverer::hand_over_the_faces`].
pub const ICON_FAMILY: &str = "iconos-del-sistema";

/// Whether that character lives in one of the three areas Unicode leaves **for private use**.
///
/// **That is where the icons of a `Nerd Font` and the Powerline separators live**, and that is why they
/// need to be distinguished: Unicode assigns no meaning to those areas, so **two different fonts
/// put different drawings in the same slot** and there is no way to say which is "the correct one" by looking at the
/// character. Only whoever installed the font can say.
fn is_private_use(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

/// What is prepended to a system family's name to name it before the text engine.
///
/// **Without it, a system font named the same as a factory one REPLACES it.** egui's keys
/// are ordinary names —`Hack`, `Ubuntu-Light`, `NotoEmoji-Regular`— and `Hack` is installed on
/// many development machines: registering it would change **the bytes** of the factory one while keeping **its
/// position** at the front of the chain, so the terminal would switch its whole font at once, without
/// any character outside private use and without even passing through the family tiling. A
/// review found it, searching for paths instead of reading the happy one.
const SYSTEM_PREFIX: &str = "sistema:";

/// What is prepended to the name of the family **whoever configures asked for**.
///
/// **It is a prefix of its own, and not the absence of one the design wrote.** Not reusing
/// [`SYSTEM_PREFIX`] is the part that matters —the chosen face goes in front, and the discovered ones
/// behind, so they cannot share a name— but registering it under its bare name would put the same
/// defect back that the prefix above exists to avoid, and it would bite the built-in font this very
/// delivery leaves as the net: whoever asks for `Hack`, which ships built in, would **replace the
/// built-in one's bytes** with the machine's copy, and the net that is supposed to stay behind for
/// what the chosen face does not carry would be the chosen face itself.
const CHOSEN_PREFIX: &str = "chosen:";

/// A system face, and whether it goes in the icon family.
struct SystemFace {
    /// **Already prefixed**: it is the name the text engine knows it by, not the one the system gives it.
    family: String,
    data: Arc<egui::FontData>,
    /// **Among the DISCOVERED ones, only the icon ones go in front**, and it is a named exception to
    /// the rule that the factory ones rule. The face asked for by name goes in front too and does
    /// **not** set this: it rules the letters, not the icons. See `Discoverer::hand_over_the_faces`.
    for_icons: bool,
}

/// Looks in the machine for the face that covers a character, and registers it in the text engine.
///
/// **It also remembers the failures, and that is not a detail.** A covered character is looked for once and its face
/// is registered, so the next one no longer looks. But one that **no** font on the machine
/// has **leaves nothing registered**: without remembering the failure it would traverse the candidates again every time
/// it appeared — and that is exactly the character a broken program spits in a loop. It comes from **D24**.
#[derive(Default)]
pub struct Discoverer {
    /// The ones already tried: covered **and** not covered.
    settled: std::collections::HashSet<char>,
    /// The ones the painter has seen and not yet looked for. Ordered so that two runs with the
    /// same screen do the same work in the same order.
    pending: BTreeSet<char>,
    /// The faces already given to the engine, in the order they were given.
    ///
    /// **They need to be kept** because registering in egui is not adding: it is handing it the **whole**
    /// configuration again. Without this list, each new face would erase the previous one.
    faces: Vec<SystemFace>,
    /// The face **whoever configures asked for by name**, if the machine had it.
    ///
    /// **It is kept apart from `faces` because it is the only one that goes in FRONT of the built-in
    /// ones**, and because it does not count against [`MAX_FAMILIES`]: that ceiling bounds what the
    /// terminal registers **on its own**, hunting for a character nobody asked about. This one was
    /// asked for out loud, it is one, and it is the whole point of the setting.
    chosen: Option<SystemFace>,
    /// The system collection, **created the first time it is needed and not before**. It is what makes
    /// discovery lazy: as long as only characters that the factory fonts cover appear,
    /// this is never built and nothing is paid.
    ///
    /// **Unless the settings file names a family**, which is the one thing that builds it at
    /// start-up — asked for out loud, once, by whoever wrote that line. See
    /// [`Discoverer::use_the_family`].
    collection: Option<Collection>,
    sources: SourceCache,
    /// Whether enumerating already failed. Without this it would be retried on every round on a machine without fontconfig.
    no_system_fonts: bool,
    /// Whether it was already said that the family ceiling was reached. Saying it once informs; saying it on every character
    /// fills the error output of a program that has done nothing wrong.
    said_the_ceiling: bool,
    /// How many searches in a row have ended finding nothing. See [`MAX_FAILURES_IN_A_ROW`].
    failures_in_a_row: usize,
    /// Whether the text engine **already knows** the icon family, that is whether there was a round after the
    /// one that declared it.
    ///
    /// **It goes one step behind `faces` on purpose, and without that the application CRASHES.** `set_fonts`
    /// saves the configuration and the engine is remade **on the next pass**; but discovery
    /// runs **before painting, on the same pass**, so between registering the face and painting the cell there
    /// is no pass in between. Offering the family there is asking epaint for a named family
    /// that does not yet exist, and that **is a panic**, not a warning: "is not bound to any fonts".
    /// Checked by running it. A review found it probing that exact path.
    icon_family_ready: bool,
    /// The color glyphs already rasterized.
    ///
    /// **It goes here and not in a separate type because it always travels with the discoverer**: whoever paints a
    /// character needs both things at once —what family it gets and, if it goes color, what texture— and
    /// separating them would force passing two mutable references through all the painter's signatures. The
    /// new state does live in its own box: [`Glyphs`] is tested and clears itself.
    glyphs: Glyphs,
    /// What has been spent rasterizing color glyphs, and how many there are.
    ///
    /// **It is timed because it runs INSIDE painting, unlike face discovery.**
    /// Registering a face remakes the atlas and that is why it is done outside; rasterizing an emoji invalidates nothing,
    /// so it is done where needed. But then its cost would fall inside the "enqueuing the
    /// painting" stretch —which would stop meaning what its name says— and from there, by subtraction, into the figure
    /// **D3** judges egui with. A mislabeled number is worse than no number.
    color: ColorCost,
}

/// What the color path has cost, so the report declares it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorCost {
    /// How much time has been spent in total.
    pub spent: std::time::Duration,
    /// How many glyphs have actually been rasterized —the ones already remembered do not count.
    pub rasterized: usize,
    /// How many times a color glyph was requested that the machine does not paint that way.
    pub failed: usize,
}

/// How asking the machine for a family by name ended.
///
/// **Four states and not two, because three different things all look like «it did not work» from
/// outside** and only one of them is the user's doing. Telling a family the machine does not have
/// from one whose files cannot be opened is what stops the terminal blaming a name that is written
/// perfectly well.
///
/// **It carries no time and it does not name the report's vocabulary**: what this module knows is
/// what happened to the face. What that is worth to the measurement report is the report's word,
/// and [`crate::measure::ChoosingTheFont`] is where it is said — the same seam `Round` already has
/// with `Discovery`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// Handed to the text engine, carrying **the name the machine gives the family** and not the one
    /// typed. What the engine is keyed by adds [`CHOSEN_PREFIX`] in front of it, so the family cannot
    /// take the place of a built-in one with the same name.
    Registered(String),
    /// The machine has no such family.
    NotInstalled,
    /// The machine **has** it and not one of its faces can be opened: a truncated file, a
    /// half-finished package update. Saying «it is not installed» here sends whoever suffers it to
    /// reinstall a font that is already there.
    Unusable,
    /// There are no system fonts at all to look among. It is said by whoever finds that out, so
    /// whoever reports this one keeps quiet instead of blaming the name.
    NoFontsOnTheMachine,
}

/// Leaves the Regular face of a family first.
///
/// **It is shared by the two searches on purpose.** Measured by a review on the discovery path: `日`
/// came out of face **index 5** of its family, chosen for being enumerated first, and in a family
/// whose Bold is enumerated first **all** the terminal's CJK would come out bold. The same is true of
/// a family asked for by name, and having the criterion written twice is how the two stop agreeing.
/// The tiebreak by index keeps the order stable, so the answer does not change between start-ups.
fn regular_first(faces: &mut [fontique::FontInfo]) {
    faces.sort_by_key(|face| {
        (
            face.style() != fontique::FontStyle::Normal,
            (face.weight().value() - fontique::FontWeight::NORMAL.value()).abs() as i32,
            ((face.width().ratio() - fontique::FontWidth::NORMAL.ratio()).abs() * 100.0) as i32,
            face.index(),
        )
    });
}

/// What a discovery round cost, so the report can declare it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Round {
    /// How many characters were looked for.
    pub searched: usize,
    /// How many new faces were given to the text engine.
    pub registered: usize,
}

impl Round {
    /// Whether this round did anything. One that looks for nothing costs nothing and does not deserve to appear in the report.
    pub fn did_work(self) -> bool {
        self.searched > 0
    }
}

impl Discoverer {
    /// What family the text engine has to be asked for to paint that character.
    ///
    /// The characters of the private-use areas go by the icon family, where the font the user
    /// installed for them rules; everything else by the usual monospaced one — where the factory
    /// fonts rule over what the terminal discovered by itself, and the family named in the settings
    /// file rules over both.
    ///
    /// **And it is asked here, not in a loose function, because the icon family DOES NOT EXIST until the
    /// engine is remade**: epaint panics —"is not bound to any fonts"— if asked for a named
    /// family nobody has declared. See `icon_family_ready` for why it is not enough to look at whether there are
    /// registered faces.
    pub fn family_of(&self, c: char) -> egui::FontFamily {
        if is_private_use(c) && self.icon_family_ready {
            egui::FontFamily::Name(ICON_FAMILY.into())
        } else {
            egui::FontFamily::Monospace
        }
    }

    /// The texture of that character's color glyph, rasterizing it if needed.
    ///
    /// Returns `None` when the machine has no font that paints it in color: the caller
    /// **has to fall to the text path**, which is Contract 5 of the design —what is not there is not
    /// invented—.
    pub fn color_glyph(
        &mut self,
        ctx: &egui::Context,
        which_char: char,
        height: u32,
        max_width: u32,
    ) -> Option<(egui::TextureId, egui::Rect, egui::Vec2)> {
        // **The collection is ensured BEFORE splitting `self`**, because building it touches two more fields and
        // cannot be done with half of it borrowed.
        if !self.ensure_the_collection() {
            return None;
        }
        // **And here it is split into disjoint borrows**: the glyph cache, the collection and the file
        // cache are distinct fields, so the compiler lets all three be held at once. It is what
        // allows `color_glyph_of` to be a free function instead of a method that would ask for the whole thing.
        let Self {
            glyphs,
            collection,
            sources,
            color,
            ..
        } = self;
        let collection = collection.as_mut()?;
        // **Only the path that does work is timed.** A glyph already remembered is returned with a
        // map lookup, and putting that lookup in the stretch would dilute it until the number stopped
        // describing what rasterizing costs — which is what the report has to declare.
        let was_already_there = glyphs.remembers(which_char, height);
        let started = (!was_already_there).then(std::time::Instant::now);
        let measurements = glyphs.texture(ctx, collection, sources, which_char, height, max_width);
        if let Some(started) = started {
            color.spent += started.elapsed();
            if measurements.is_some() {
                color.rasterized += 1;
            } else {
                color.failed += 1;
            }
        }
        measurements
    }

    /// What the color path has cost since it was last asked, reset to zero.
    ///
    /// **It is emptied on reading because the report counts per frame**, like the rest of its stretches:
    /// returning the session accumulation would give an average that grows on its own and describes no frame.
    pub fn color_cost(&mut self) -> ColorCost {
        std::mem::take(&mut self.color)
    }

    /// How many color glyphs are remembered. So the measurement report can declare it.
    pub fn glyphs_remembered(&self) -> usize {
        self.glyphs.how_many()
    }

    /// Fills the color glyph atlas until not one more fits, at that cell height.
    ///
    /// **It exists so the painter can test what is seen when the atlas fills**, which is what
    /// Contract 5 promises: it falls to the text path and the emoji comes out the same, in monochrome. The atlas and its
    /// height belong to this module, so from the painter there is no way to leave it in that state — and
    /// filling it with real emojis demands that the machine have eight hundred distinct ones installed.
    #[cfg(test)]
    pub fn fill_the_atlas(&mut self, ctx: &egui::Context, height: u32) {
        self.glyphs.height = height;
        let side = ATLAS_SIDE / 4;
        while self
            .glyphs
            .blit(
                ctx,
                egui::ColorImage::filled([side, side], egui::Color32::RED),
            )
            .is_some()
        {}
    }

    /// Notes a character the painter has run into.
    ///
    /// **It is the only thing that runs per cell, and that is why it is the only thing that has to be cheap**: a character already
    /// resolved is discarded with a set lookup. Looking and registering happen outside painting.
    pub fn note(&mut self, which_char: char) {
        // **ASCII is never looked at.** egui's factory fonts cover it whole, and it is the vast
        // majority of what passes through a terminal screen: asking it would be paying a lookup per
        // cell to always get the same thing.
        if which_char.is_ascii() || self.settled.contains(&which_char) || self.no_system_fonts {
            return;
        }
        if self.pending.len() < MAX_PENDING {
            self.pending.insert(which_char);
        }
    }

    /// Notes all the characters of a text.
    ///
    /// **It exists because it is not only the console grid that paints text.** The status panel shows what
    /// its producer publishes, which is free text and can come in any language: without this, a summary
    /// in Japanese came out as boxes **forever**, because nothing triggered the search. And that in the milestone
    /// whose very object is that it be seen. A review found it.
    pub fn note_text(&mut self, text: &str) {
        for which_char in text.chars() {
            self.note(which_char);
        }
    }

    /// Looks for the faces of what was noted and gives them to the text engine. Returns what it did.
    ///
    /// **It runs OUTSIDE painting**, and for two reasons: asking the text engine whether it covers a character
    /// needs exclusive access to the context —asking for it in the middle of an interface pass is re-entering what
    /// is already borrowed—, and registering a face **remakes the whole glyph atlas**, which is
    /// milliseconds and not microseconds.
    pub fn resolve(&mut self, ctx: &egui::Context) -> Round {
        // **First of all: the family declared in the PREVIOUS round is already known to the engine.** Between
        // that round and this one a whole frame has passed, which is what `set_fonts` needs to
        // take effect. It goes here and not at the end so it does not get ahead of itself.
        // **The same list `icon_ones` builds from, chosen face included.** With two readers walking
        // different lists, a face that reached the icon family through one of them and not through
        // the other would have the painter asking epaint for a family it does not know, which is a
        // panic and not a warning.
        let some_icon_face = self.every_face().any(|face| face.for_icons);
        self.icon_family_ready = some_icon_face;

        if self.pending.is_empty() {
            return Round::default();
        }
        let pending = std::mem::take(&mut self.pending);

        // **What already shows fine is discarded BEFORE touching the system, and that costs 165 ns.**
        //
        // `Fonts::has_glyph` is no use for this: it promises "can we show this glyph?" and what it does is
        // `resolve_face(c) != replacement_face_key`, that is **it returns `false` for the `'a'`** —checked
        // by running it against epaint 0.36.1—. The first version of this module, left without an oracle,
        // looked for **everything** non-ASCII: a Spanish accent cost 18 ms and registered a face that
        // changed nothing, spending on top of that one of the few family slots. A review measured it.
        //
        // **The oracle that does work is the one the tests use**: laying out the character and comparing its
        // drawing with that of the replacement box. It tells the truth and costs **four orders of magnitude
        // less** than traversing the machine's families.
        let to_look_for = ctx.fonts_mut(|fonts| {
            let the_box = drawing_of(fonts, REPLACEMENT_BOX, egui::FontFamily::Monospace);
            let mut to_look_for = Vec::new();
            for which_char in pending {
                if drawing_of(fonts, which_char, self.family_of(which_char)) != the_box {
                    // Already shows: there is nothing to look for, and it is not looked at again.
                    self.settled.insert(which_char);
                } else {
                    to_look_for.push(which_char);
                }
            }
            to_look_for
        });

        let mut round = Round::default();
        // **Budget is spent, not a number of rounds.** And it is checked BEFORE each search: one
        // that starts within budget can end outside it —the worst case is 18 ms— and that is
        // acceptable; starting the next one already outside is not.
        let started = std::time::Instant::now();
        for which_char in to_look_for {
            let out_of_budget = started.elapsed() >= BUDGET_PER_ROUND;
            let gave_up = self.failures_in_a_row >= MAX_FAILURES_IN_A_ROW;
            if gave_up {
                // The machine has already said twelve times in a row that it has nothing to do it with. They are given as resolved
                // without looking: keeping on asking costs up to 18 ms each for the same "no".
                self.settled.insert(which_char);
                continue;
            }
            if out_of_budget {
                // **They are left NOT marked as resolved**, so the painter notes them again and they are
                // looked for on the next frame. Marking them here would condemn them to come out as a box
                // forever just for having arrived late to this round.
                continue;
            }
            self.settled.insert(which_char);
            round.searched += 1;
            if self.record_who_covers(which_char) {
                round.registered += 1;
                self.failures_in_a_row = 0;
            } else {
                self.failures_in_a_row += 1;
            }
        }
        // **The file cache is emptied as soon as the round finishes, and it is not optional.** It keeps a
        // live `Blob` for **each** font file the search got to open, and a failed search
        // opens **all** of the machine's: without pruning, a session that sees a rare character ends up
        // with hundreds of megs resident forever. What is really needed is already copied into
        // `faces`. A review measured it.
        self.sources.prune(0, true);

        if round.registered > 0 {
            self.hand_over_the_faces(ctx);
            // **Without this the discovered glyph can stay a box indefinitely.** Two more
            // passes are needed —one for `set_fonts` to take effect and another to repaint— and nothing triggers them:
            // repaints are requested by pending output or by input. `echo 日本語` and stopping left the
            // kanji a box **until someone touched something**. A review found it.
            ctx.request_repaint();
        }
        round
    }

    /// Paints with the family whoever configures asked for **by its name**.
    ///
    /// **It is only called when the file names one, and that is what keeps D24 standing.** Resolving a
    /// name forces building the system collection — between 17.3 and 23.2 ms, the very cost D24
    /// decided never to pay at start-up. Whoever configures no font never reaches this line and their
    /// start-up is the one of always, bit for bit.
    ///
    /// **It runs before the first frame**, like restoring the layout: `set_fonts` takes effect on the
    /// next pass, and there has been no pass yet.
    ///
    /// The name is matched **the way `fontique` matches it**, which lowercases both sides
    /// (`family_name.rs`, `NameKey::from_str`), so `jetbrains mono` finds `JetBrains Mono`.
    pub fn use_the_family(&mut self, name: &str, ctx: &egui::Context) -> Chosen {
        let found = self.look_for_the_family(name);
        // **The file cache is emptied right away, and it is not optional**: it keeps a live `Blob` for
        // each font file the search got to open, and what is really needed has already been copied.
        // Same reason as in `resolve`.
        self.sources.prune(0, true);
        let (family, data) = match found {
            Ok(face) => face,
            Err(why) => return why,
        };
        self.chosen = Some(SystemFace {
            family: format!("{CHOSEN_PREFIX}{family}"),
            data,
            // **Not for the icons.** Those are the one place where the installed font rules over
            // everything, and it is the font that carries the drawing that has to win there — not the
            // one somebody picked to read code with.
            for_icons: false,
        });
        self.hand_over_the_faces(ctx);
        Chosen::Registered(family)
    }

    /// Whether the machine's fonts have been enumerated at all.
    ///
    /// **It exists so the laziness can be asserted on, and that is not a detour**: what D24 decided is
    /// that nobody pays the enumeration without asking, and the only observable difference between
    /// paying it and not is whether this collection exists.
    pub fn has_searched_the_machine(&self) -> bool {
        self.collection.is_some()
    }

    /// The face of that family the text engine can open, or **why there is none**.
    ///
    /// **The failure carries its reason and that is the whole point of the signature.** A bare
    /// `Option` here made three different endings —no fonts at all, no such family, a family whose
    /// files cannot be opened— arrive as the same `None`, and the terminal then told whoever
    /// configured it that their name was wrong in all three.
    fn look_for_the_family(&mut self, name: &str) -> Result<(String, Arc<egui::FontData>), Chosen> {
        if !self.ensure_the_collection() {
            return Err(Chosen::NoFontsOnTheMachine);
        }
        let collection = self
            .collection
            .as_mut()
            .ok_or(Chosen::NoFontsOnTheMachine)?;
        let id = collection.family_id(name).ok_or(Chosen::NotInstalled)?;
        // **The name is taken from the collection and not from what was typed**, so what the report
        // declares is the family that is really being painted with: asking for `jetbrains mono` and
        // publishing that lowercase name would have the report saying something the machine does not.
        let (real_name, mut faces) = collection
            .family(id)
            .map(|family| (family.name().to_owned(), family.fonts().to_vec()))
            .ok_or(Chosen::NotInstalled)?;
        regular_first(&mut faces);
        let sources = &mut self.sources;
        let face = faces.into_iter().find_map(|face| {
            let blob = sources.get(face.source())?;
            // Same check and for the same reason as in `first_that_covers`: bytes epaint cannot parse
            // are a `panic!` **inside its pass**, and these are files from outside.
            skrifa::FontRef::from_index(blob.data(), face.index()).ok()?;
            let mut data = egui::FontData::from_owned(blob.data().to_vec());
            data.index = face.index();
            Some(Arc::new(data))
        });
        // **The family is here and none of its files can be opened.** It is a different answer from
        // «it is not installed» and it has to stay different: whoever gets the wrong one reinstalls a
        // font that was already there and reads the very same line again.
        face.map(|data| (real_name, data)).ok_or(Chosen::Unusable)
    }

    /// Looks for who covers that character and adds its face to the list. `true` if it added one.
    fn record_who_covers(&mut self, which_char: char) -> bool {
        if self.faces.len() >= MAX_FAMILIES {
            // **It is said, and only once.** A ceiling that swallows what it discards is indistinguishable from a
            // failure: whoever sees a box after installing the right font would have no way to know
            // that the terminal found the font and decided not to use it. It is the same rule the
            // rest of the project declares its limits with instead of keeping them quiet.
            if !self.said_the_ceiling {
                eprintln!(
                    "agent-terminal: there are already {MAX_FAMILIES} system fonts registered, which is the \
                     ceiling. Characters the factory ones do not cover will keep coming out as boxes."
                );
                self.said_the_ceiling = true;
            }
            return false;
        }
        let Some(face) = self.look_for(which_char) else {
            // **And here the attempt ends: what is not there is not invented.** The character is painted with whatever
            // the engine gives —its box— and nothing else is attempted. It is already in `settled`, so it is not
            // looked for again.
            return false;
        };
        let name = format!("{SYSTEM_PREFIX}{}", face.0);
        if self.faces.iter().any(|ours| ours.family == name) {
            // Another face of the same family is already registered: it covers this too.
            return false;
        }
        self.faces.push(SystemFace {
            family: format!("{SYSTEM_PREFIX}{}", face.0),
            data: face.1,
            for_icons: is_private_use(which_char),
        });
        true
    }

    /// The first face on the machine that covers that character, with the name of its family.
    fn look_for(&mut self, which_char: char) -> Option<(String, Arc<egui::FontData>)> {
        // **It is checked here IN ADDITION to in `note`, and it is not redundant:** the characters that were already
        // in the queue when enumerating failed reach here anyway, and without this line each one would
        // build the whole collection again only to find it empty again.
        if !self.ensure_the_collection() {
            return None;
        }
        let collection = self.collection.as_mut()?;
        // **The two fields are borrowed separately on purpose**: asking for them through a method that
        // returned the collection would borrow the whole `self` and the second borrow would not compile.
        let sources = &mut self.sources;
        // **Two routes, and in this order.** The monospaced families the system declares are few and
        // are the right ones for a terminal, so they are asked first; traversing them all is the net
        // underneath and the measured worst case —from 94 µs to 18.2 ms depending on how much the winner weighs, not on
        // where it falls in the order (**D24**)—.
        let generic_ones: Vec<_> = collection
            .generic_families(GenericFamily::Monospace)
            .collect();
        let mut all_names: Vec<String> = collection.family_names().map(str::to_owned).collect();
        // **By name, so the answer does not change between startups.** The order in which the system
        // delivers them is not stable: two bench runs gave different families for the same character
        // until it was sorted.
        all_names.sort();
        let of_the_rest: Vec<_> = all_names
            .iter()
            .filter_map(|name| collection.family_id(name))
            .collect();

        for ids in [&generic_ones, &of_the_rest] {
            if let Some(face) = first_that_covers(collection, sources, ids, which_char) {
                return Some(face);
            }
        }
        None
    }

    /// Builds the system collection if it does not yet exist. Returns whether there is anything to search with.
    ///
    /// **It comes out of `look_for` so the searches —the coverage one, the color one and the family
    /// asked for by name— share a single collection.** Enumerating costs between 17.3 and 23.2 ms (**D24**): doing it twice for having two
    /// functions would be paying the module's most expensive stretch twice over.
    fn ensure_the_collection(&mut self) -> bool {
        if self.no_system_fonts {
            return false;
        }
        if self.collection.is_some() {
            return true;
        }
        // **It is built here and not at startup: that is what makes discovery lazy.**
        // As long as only characters that the factory fonts cover appear, this line never runs —
        // **and the exception is a settings file that names a family**, which reaches here through
        // `use_the_family` before the first frame, because somebody asked for it.
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        // **A machine without fontconfig returns an empty collection instead of crashing**, and that is the case
        // **AC9** requires enduring. It is said through the error output and it stops trying: without this it
        // would enumerate again on every round, spending the whole cost for nothing.
        if collection.family_names().next().is_none() {
            eprintln!(
                "agent-terminal: no system font was found. Characters the \
                 factory fonts do not cover will keep coming out as boxes."
            );
            self.no_system_fonts = true;
            return false;
        }
        self.collection = Some(collection);
        true
    }

    /// Gives the text engine **all** the faces discovered so far.
    ///
    /// **The DISCOVERED ones behind the factory ones, and that is Contract 2 of the design**: the
    /// fallback chain is traversed in order, so a system face placed in front would steal glyphs from
    /// the terminal font —and the terminal would stop looking as it looks today in what already works
    /// today—. **The face asked for by name is the named exception and goes in front**: it was chosen
    /// by a person, for everything, and behind the factory font it would not paint one letter.
    ///
    /// **And the icon ones go to a SEPARATE FAMILY, not in front of the factory ones.** That was the first
    /// version and it was wrong: egui orders the fallback chain **by whole family**, not by range of
    /// characters, so placing an icon font in front —which is almost always a `Nerd Font`, and those
    /// **carry the full Latin set**— takes the letters away from the terminal font. Measured before
    /// changing it: discovering a single icon **changed the drawing of three out of every five letters**, which is
    /// exactly what Contract 2 exists to prevent.
    ///
    /// With its own family, the painter asks for [`ICON_FAMILY`] **only for the private-use
    /// characters** and the monospaced one for everything else. The per-character order egui does not give, the
    /// painter gives, since it already traverses cell by cell.
    fn hand_over_the_faces(&self, ctx: &egui::Context) {
        let mut definitions = egui::FontDefinitions::default();
        let factory = definitions
            .families
            .get(&egui::FontFamily::Monospace)
            .cloned()
            .unwrap_or_default();

        definitions
            .families
            .insert(egui::FontFamily::Monospace, self.monospaced(&factory));
        definitions.families.insert(
            egui::FontFamily::Name(ICON_FAMILY.into()),
            self.icon_ones(&factory),
        );
        for face in self.every_face() {
            definitions
                .font_data
                .insert(face.family.clone(), Arc::clone(&face.data));
        }
        ctx.set_fonts(definitions);
    }

    /// Every face this discoverer holds: the chosen one first, then the ones it found by itself.
    ///
    /// **One walk for the three readers.** The chosen face and the discovered ones live in separate
    /// fields —only one of them goes in front of the factory fonts— and that is exactly the shape in
    /// which two readers drift apart. Whether a face rules the icons is decided by `for_icons`, in
    /// all three, and never by which field it was stored in.
    fn every_face(&self) -> impl Iterator<Item = &SystemFace> {
        self.chosen.iter().chain(&self.faces)
    }

    /// The monospaced family: **the chosen one in front, then the factory ones, and the system ones
    /// behind**.
    ///
    /// **The one in front is the named exception, and without it the setting cannot work.** The chain
    /// is traversed in order and the built-in font already covers the whole Latin range, so a face
    /// placed behind it would not paint a single letter — see [`Discoverer::use_the_family`]. The
    /// difference that justifies it: the ones behind are chosen **by the terminal, to cover a gap**;
    /// this one is chosen **by a person, and for everything**.
    ///
    /// **It is set apart so it can be asserted on**, and it is not a detour: this list **is** what egui
    /// consumes. Asserting instead on the drawing that comes out does not distinguish the cases — registering a face
    /// remakes the atlas and **all** the glyphs change place, so the comparison passes the same with the
    /// defect in and out. Mutating it exposed it.
    fn monospaced(&self, factory: &[String]) -> Vec<String> {
        self.chosen
            .iter()
            .map(|c| c.family.clone())
            .chain(factory.iter().cloned())
            .chain(self.faces.iter().map(|c| c.family.clone()))
            .collect()
    }

    /// The icon family: **the system ones first, and the factory ones behind as a net**.
    ///
    /// Only the faces discovered **looking for a private-use character** enter: one found
    /// by a kanji does not sneak in here even if it also carries icons — **and neither does the one
    /// whoever configures asked for**. Those areas are the one place where the installed font rules,
    /// and the one that rules there is the one that carries the drawing, not the one somebody picked
    /// to read code with.
    ///
    /// **It walks the same list as [`Discoverer::monospaced`]**, chosen one included, so that the
    /// only thing deciding who paints an icon is `for_icons` and not which of the two lists a face
    /// happened to be stored in.
    fn icon_ones(&self, factory: &[String]) -> Vec<String> {
        self.every_face()
            .filter(|c| c.for_icons)
            .map(|c| c.family.clone())
            .chain(factory.iter().cloned())
            .collect()
    }
}

/// What that character's glyph measures in the atlas, or `None` if there ends up being no glyph.
///
/// **It is the oracle for "does this already show?"**: compared with the replacement box's it tells the truth,
/// unlike `Fonts::has_glyph`. It costs **165 ns**, measured in release.
fn drawing_of(
    fonts: &mut egui::epaint::text::FontsView<'_>,
    which_char: char,
    family: egui::FontFamily,
) -> Option<(u16, u16)> {
    let galley = fonts.layout_no_wrap(
        which_char.to_string(),
        egui::FontId::new(SIZE_TO_ASK_WITH, family),
        egui::Color32::WHITE,
    );
    let uv = galley.rows.first()?.glyphs.first()?.uv_rect;
    Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
}

/// The size the engine is asked with whether a character shows.
///
/// **It does not matter which it is as long as it is the same for both sides of the comparison**: what is compared is
/// whether two characters paint the same drawing, not how much they measure.
const SIZE_TO_ASK_WITH: f32 = 14.0;

/// The first of those families that brings that character's glyph.
fn first_that_covers(
    collection: &mut Collection,
    sources: &mut SourceCache,
    ids: &[fontique::FamilyId],
    which_char: char,
) -> Option<(String, Arc<egui::FontData>)> {
    for &id in ids {
        // **The name and the faces are copied before continuing**: the family borrows from the collection, and the
        // collection is needed on the next round.
        let Some((name, faces)) = collection
            .family(id)
            .map(|family| (family.name().to_owned(), family.fonts().to_vec()))
        else {
            continue;
        };
        // **The Regular face is preferred, not the first the system enumerates.** Measured by a review:
        // `日` came out of face **index 5** of its family, chosen because it happened to go first. In a
        // family whose Bold or Thin is enumerated first, **all the terminal's CJK would come out bold**,
        // permanently and with no way to fix it. See [`regular_first`], which is shared with the
        // search by name so that the two cannot stop agreeing.
        let mut faces = faces;
        regular_first(&mut faces);
        for face in faces {
            let Some(blob) = sources.get(face.source()) else {
                continue;
            };
            let covers = face
                .charmap_index()
                .charmap(blob.data())
                .is_some_and(|charmap| {
                    charmap
                        .map(which_char as u32)
                        .is_some_and(|glyph| glyph != 0)
                });
            if covers {
                // **And it is checked that the text engine is going to be able to open it, BEFORE giving it.** The
                // only failure mode epaint has for bytes it does not understand is
                // `panic!("Error parsing … TTF/OTF font file")`, and it happens **inside its pass**: a
                // file whose `cmap` is read but whose table directory is not —a half-finished
                // package update, a truncated file, some old fontconfig offsets—
                // **would kill the process** on the next frame. And they are files from outside: what
                // they carry is not controlled. A review pointed it out.
                //
                // It is validated with the same crate epaint uses underneath, so what opens here,
                // opens there.
                if skrifa::FontRef::from_index(blob.data(), face.index()).is_err() {
                    continue;
                }
                // **With ITS face index, not with zero.** A font file can carry several
                // faces inside —a `.ttc` collection— and `FontData::from_owned` keeps the first:
                // registering the file without saying which was being looked for leaves the engine with a face that does **not**
                // cover the character, and the terminal keeps painting the box after having paid for the
                // whole search. Checked: it is what happened with the Private-Use Area icons.
                let mut data = egui::FontData::from_owned(blob.data().to_vec());
                data.index = face.index();
                return Some((name, Arc::new(data)));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// The color glyph
// ---------------------------------------------------------------------------------------------

/// What the color glyph atlas measures on a side, in pixels.
///
/// **The ceiling of this milestone is a FIXED-size texture, not a counter**, and that is better than what the
/// design asked for: the memory the graphics card dedicates to the emojis is **1 MiB and does not change**, whether
/// one or a thousand appear. At this machine's cell height about eight hundred glyphs fit —17 px tall, 18
/// wide—, many more than a person sees in a session; when it fills, painting continues, but in
/// monochrome by the usual text path.
///
/// It is the same criterion **D8** bounded the output queue with and **D21** the history: an entry that
/// arrives from outside and nobody inside controls carries a ceiling, and the ceiling is said.
const ATLAS_SIDE: usize = 512;

/// The size the bitmap is asked of the font with.
///
/// **It is not the cell height on purpose.** `Noto Color Emoji` brings **a single run** of 136×128
/// pixels —checked by running it, not by reading the specification— so asking it for one size or another
/// returns exactly the same image. Asking for the largest of the reasonable ones is the only thing that changes anything in
/// a font that does bring several: the largest is taken and reduced, which loses less than enlarging a
/// small one.
const SIZE_TO_RASTERIZE_WITH: f32 = 128.0;

/// Where a glyph is within the atlas and how much it measures.
#[derive(Debug, Clone, Copy)]
struct InAtlas {
    /// The piece of the atlas it occupies, in coordinates from 0 to 1 — which is how it is asked for when painting.
    uv: egui::Rect,
    /// What it actually measures, in pixels. The painter needs it to center it without deforming it.
    measurements: egui::Vec2,
}

/// The color glyphs already rasterized, **all within a single texture**.
///
/// **One texture and not one per glyph, and the difference is measured.** With one texture per emoji, the engine
/// cannot join two in the same drawing batch —its tessellator says so: mixing texture identifiers
/// in the same mesh is an error— so each emoji on the screen splits the batch. Measured with the two
/// consoles dumping emojis against their control in ASCII: **the rest of the frame went from 275 µs to
/// 743 µs**, that is the figure
/// **D3** judges the framework with almost tripled, and for work that is not the framework's. With an atlas,
/// all the emojis on the screen are drawn together. It is what the terminals that are the bar do.
///
/// **The height is a field and not part of the key, and that is a decision with a consequence.** The whole
/// window shares a single metric —it is measured once per pass, with the same size for all the
/// panes—, so two different heights never coexist: they only follow one another, when the desktop zoom
/// changes. Keeping the height in the key would leave the old entries taking up space forever;
/// having it here allows **emptying the atlas whole** as soon as it changes, which is what the design contract
/// requires and what prevents painting an emoji of the previous size in a cell of the new one.
#[derive(Default)]
struct Glyphs {
    /// The texture where they all live. **It does not exist until the first one is needed**: a session without
    /// emojis does not ask the graphics card for a single byte.
    atlas: Option<egui::TextureHandle>,
    /// What is remembered, failures included: a `None` is "nobody on this machine paints this in color".
    ///
    /// **Remembering the failure is not a detail.** Without it, a character that no color font covers
    /// would traverse the emoji families **on every frame** — and it is exactly the character a broken program
    /// spits in a loop. It is the same reason [`Discoverer`] remembers its own.
    saved_ones: std::collections::HashMap<char, Option<InAtlas>>,
    /// Where the free slot of the row being filled starts.
    next_spot: [usize; 2],
    /// How tall the row being filled measures. On jumping rows this is advanced.
    row_height: usize,
    /// The cell height the ones above are rasterized at, in physical pixels.
    height: u32,
    /// Whether it was already said that the atlas filled.
    said_the_ceiling: bool,
}

impl Glyphs {
    /// Where that character's color glyph is within the atlas, rasterizing it if needed.
    ///
    /// Returns `None` when the machine has no font that paints it in color, or when the atlas
    /// filled: the caller **has to fall to the text path**, which is Contract 5 of the design —what
    /// is not there is not invented—.
    fn texture(
        &mut self,
        ctx: &egui::Context,
        collection: &mut Collection,
        sources: &mut SourceCache,
        which_char: char,
        height: u32,
        max_width: u32,
    ) -> Option<(egui::TextureId, egui::Rect, egui::Vec2)> {
        if height == 0 || max_width == 0 {
            return None;
        }
        // **The cell height changed: what is remembered is no longer valid.** Without this, raising the desktop
        // zoom would leave the emojis of the previous size painted in cells of the new one — and since the
        // atlas is never emptied, for the rest of the process's life.
        if self.height != height {
            self.saved_ones.clear();
            self.next_spot = [0, 0];
            self.row_height = 0;
            self.height = height;
            self.said_the_ceiling = false;
        }
        let spot = if let Some(saved) = self.saved_ones.get(&which_char) {
            *saved
        } else {
            let spot = color_glyph_of(collection, sources, which_char, height, max_width)
                .and_then(|image| self.blit(ctx, image));
            self.saved_ones.insert(which_char, spot);
            spot
        };
        // **The atlas exists whenever there is a saved spot**: it is created on pasting the first glyph, and what
        // is saved is precisely the result of having pasted it.
        let spot = spot?;
        Some((self.atlas.as_ref()?.id(), spot.uv, spot.measurements))
    }

    /// Puts an image in the first free slot of the atlas. `None` if it no longer fits.
    fn blit(&mut self, ctx: &egui::Context, image: egui::ColorImage) -> Option<InAtlas> {
        let [width, height] = [image.width(), image.height()];
        if width > ATLAS_SIDE || height > ATLAS_SIDE {
            return None;
        }
        // Placement by rows: the row is filled left to right and it jumps to the next when
        // it does not fit. **It leaves gaps and it does not matter**: the glyphs of a session all measure almost the same —the
        // same cell height rasterizes them— so the rows come out even.
        if self.next_spot[0] + width > ATLAS_SIDE {
            self.next_spot = [0, self.next_spot[1] + self.row_height];
            self.row_height = 0;
        }
        if self.next_spot[1] + height > ATLAS_SIDE {
            // **It is said, and only once.** A silent ceiling is indistinguishable from a failure: whoever sees their
            // emojis go monochrome all at once would have no way to know that the terminal knows how to paint them and has
            // run out of room to store them.
            if !self.said_the_ceiling {
                eprintln!(
                    "agent-terminal: the color glyph atlas ({ATLAS_SIDE}x{ATLAS_SIDE} \
                     pixels) has filled. Emojis not already in it will keep coming out in \
                     monochrome."
                );
                self.said_the_ceiling = true;
            }
            return None;
        }
        let spot = self.next_spot;
        let atlas = self.atlas.get_or_insert_with(|| {
            ctx.load_texture(
                "glifos-de-color",
                egui::ColorImage::filled([ATLAS_SIDE, ATLAS_SIDE], egui::Color32::TRANSPARENT),
                // **Linear and not `NEAREST`**: the bitmap comes at 136×128 and the cell measures a fraction
                // of that, so the reduction is done by the CPU resampling; what the card does
                // here is only the final adjustment to physical pixels. With `NEAREST` that adjustment shows as
                // sawtooth on the edge of the drawing.
                egui::TextureOptions::LINEAR,
            )
        });
        // **Only the rectangle of the new glyph, not the whole atlas.** Re-uploading the complete megabyte for
        // each discovered emoji would be half a gig of traffic to the card in a session that reaches the
        // ceiling, to change a few hundred bytes.
        atlas.set_partial(spot, image, egui::TextureOptions::LINEAR);
        self.next_spot[0] = spot[0] + width;
        self.row_height = self.row_height.max(height);
        let side = ATLAS_SIDE as f32;
        Some(InAtlas {
            uv: egui::Rect::from_min_max(
                egui::pos2(spot[0] as f32 / side, spot[1] as f32 / side),
                egui::pos2(
                    (spot[0] + width) as f32 / side,
                    (spot[1] + height) as f32 / side,
                ),
            ),
            measurements: egui::vec2(width as f32, height as f32),
        })
    }

    /// Whether that character is already remembered —including the memory that it is not painted in color.
    ///
    /// **It looks at the height too**, because a change of height empties the whole atlas: without that, the first
    /// glyph after a zoom change would be rasterized **without** its cost entering the stretch.
    fn remembers(&self, which_char: char, height: u32) -> bool {
        self.height == height && self.saved_ones.contains_key(&which_char)
    }

    /// How many glyphs are remembered.
    fn how_many(&self) -> usize {
        self.saved_ones.len()
    }
}

/// The color image of that character, already rescaled, if any font on the machine paints it that way.
///
/// **It is a free function and not a method, and that solves a real problem.** It needs the system
/// collection and the file cache, which are **two fields** of the discoverer; as a method it would borrow the
/// whole discoverer, and its caller —the glyph cache, which lives **inside** it— already has its
/// part borrowed. The first version got around it by taking the cache out with `mem::take` and returning it afterward, which
/// left a trap: a second entry by that path would have seen an empty cache and the first would have
/// overwritten on return whatever the second saved. Nobody re-enters today, and nothing prevented it.
///
/// **It returns the image and not the face**: the caller stores textures, not fonts. Lending it the open face
/// would force it to carry the file's twenty megs while it uses it, and to speak the font model
/// of a third-party crate for nothing.
fn color_glyph_of(
    collection: &mut Collection,
    sources: &mut SourceCache,
    which_char: char,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    // **Only the families the system declares as emoji, and there the search ends.** It does not fall to
    // traversing them all as `look_for` does: a family that is not emoji and still covers an emoji
    // paints it **in monochrome** —on this machine `DejaVu Sans` takes the 😀 with `Noto Color Emoji`
    // installed—, so the net underneath would give exactly what this function exists to avoid.
    let generic_ones: Vec<_> = collection.generic_families(GenericFamily::Emoji).collect();
    for id in generic_ones {
        let Some(faces) = collection.family(id).map(|family| family.fonts().to_vec()) else {
            continue;
        };
        for face in faces {
            let Some(blob) = sources.get(face.source()) else {
                continue;
            };
            let Ok(font) = skrifa::FontRef::from_index(blob.data(), face.index()) else {
                continue;
            };
            let Some(glyph) = font.charmap().map(which_char) else {
                continue;
            };
            // **Covering it is not enough: it has to bring it in color.** The families the system
            // declares as emoji on this machine are dozens —almost all the symbol `Noto`s— and
            // most are monochrome. Without this check the first the system enumerates would win,
            // which is the defect this project has documented since **D24** and the one
            // Alacritty carries.
            if let Some(image) = rasterize(&font, glyph, height, max_width) {
                return Some(image);
            }
        }
    }
    None
}

/// The pixels of a bitmap, in RGBA and with **the alpha already premultiplied**. `None` if what it brings
/// is not a color drawing.
///
/// **It is apart from `rasterize` so it can be asserted on, and it is not a detour.** Which of the three
/// paths an emoji travels **is not decided by this project: it is decided by the font the machine has
/// installed**. On this one `Noto Color Emoji` rules, which brings PNG, so the other two would be delivered without
/// any test ever having run them — and they are the two that come first in **F2.7**.
/// Checked by mutating them with the body inside `rasterize`: removing the BGRA-to-RGBA swap, which paints
/// blue what the emoji has as red, left the 647 tests green.
fn pixels_of(
    bitmap: &skrifa::bitmap::BitmapData<'_>,
    width: u32,
    height: u32,
) -> Option<image::RgbaImage> {
    match bitmap {
        skrifa::bitmap::BitmapData::Png(bytes) => {
            let mut pixels = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()?
                .into_rgba8();
            // **It is premultiplied BEFORE rescaling, and that is the whole difference between a clean emoji and
            // one with a halo.** Averaging transparent pixels with opaque ones without premultiplying mixes the
            // color of the transparent ones into the edge of the drawing. It is the classic defect of rescaling with
            // alpha, and here it would happen in all of them: an emoji is a round drawing inside an empty box.
            for pixel in pixels.pixels_mut() {
                let alpha = u32::from(pixel.0[3]);
                for channel in &mut pixel.0[..3] {
                    *channel = u8::try_from(u32::from(*channel) * alpha / 255).unwrap_or(*channel);
                }
            }
            Some(pixels)
        }
        skrifa::bitmap::BitmapData::Bgra(bytes) => {
            let mut raw = bytes.to_vec();
            // From BGRA to RGBA. It already comes with the alpha premultiplied, which is what is needed above.
            for pixel in raw.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(width, height, raw)
        }
        // An alpha mask is a **monochrome** drawing: it is not a color glyph and it is not painted here.
        skrifa::bitmap::BitmapData::Mask(_) => None,
    }
}

/// The color bitmap of a glyph, rescaled to fit in the slot it is given.
///
/// Returns `None` when that face does **not** bring the glyph in color: either it has no bitmap table, or it
/// has one and for that glyph it stores an **alpha mask**, which is a monochrome drawing and therefore not
/// this. The caller falls to the text path.
///
/// **The color outlines —`COLR`— are left out, and not by oversight.** Painting them demands rasterizing
/// outlines by hand, which is the custom rasterizer this milestone discarded on purpose for invalidating the
/// whole measurement history. On Linux it is not noticed —the system emoji fonts are bitmap—
/// but on Windows it is, because `Segoe UI Emoji` is `COLR`: it is declared as what
/// **F2.7** lacks, not as something that works here.
fn rasterize(
    font: &skrifa::FontRef<'_>,
    glyph: skrifa::GlyphId,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    let bitmap = skrifa::bitmap::BitmapStrikes::new(font)
        .glyph_for_size(skrifa::instance::Size::new(SIZE_TO_RASTERIZE_WITH), glyph)?;
    let pixels = pixels_of(&bitmap.data, bitmap.width, bitmap.height)?;
    if pixels.width() == 0 || pixels.height() == 0 {
        return None;
    }
    // **The factor is the SMALLER of the two and not the height's, even though the contract speaks of the height.**
    // The two at once —"at the cell height" and "without invading the one next to it"— are only met this way: the
    // bitmap of `Noto Color Emoji` is wider than it is tall (136×128), so scaling it by the height
    // leaves it **wider than the two columns** the engine reserved for it, and the emoji would eat the edge
    // of its neighbor. Measured: 18 px wide for a cell 17 px tall.
    let factor =
        (height as f32 / pixels.height() as f32).min(max_width as f32 / pixels.width() as f32);
    let final_width = ((pixels.width() as f32 * factor).round() as u32).clamp(1, max_width);
    let final_height = ((pixels.height() as f32 * factor).round() as u32).clamp(1, height);
    let shrunk = image::imageops::resize(
        &pixels,
        final_width,
        final_height,
        // **Triangular and not Lanczos.** It is reduced from 136 px to seventeen: at that scale the two give a
        // result that cannot be told apart by looking, and the triangular one costs half. The cost is paid once
        // per character, but it is paid **inside a frame**.
        image::imageops::FilterType::Triangle,
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [final_width as usize, final_height as usize],
        shrunk.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The emoji selector: `U+FE0F`.
    const EMOJI_PRESENTATION_SELECTOR: char = '\u{FE0F}';
    /// The text selector: `U+FE0E`.
    const TEXT_PRESENTATION_SELECTOR: char = '\u{FE0E}';

    #[test]
    fn an_everyday_emoji_goes_color_without_anyone_asking() {
        // `Emoji_Presentation=Yes`. Requiring the selector here would leave gray the vast majority of the
        // emojis a program writes, which is how they are seen today.
        assert_eq!(presentation_of('😀', &[]), Presentation::Color);
    }

    #[test]
    fn a_text_symbol_stays_text_even_though_its_emoji_version_exists() {
        // `☁` U+2601 has `Emoji=Yes` but `Emoji_Presentation=No`. Looking only at "is emoji" would paint it
        // color, and it is exactly the case that separates this rule from the naive one.
        assert_eq!(presentation_of('☁', &[]), Presentation::Text);
    }

    #[test]
    fn the_emoji_selector_takes_to_color_a_symbol_that_is_text_by_default() {
        // It is `☁️` — the whole reason for being of this delivery.
        assert_eq!(
            presentation_of('☁', &[EMOJI_PRESENTATION_SELECTOR]),
            Presentation::Color
        );
    }

    #[test]
    fn the_text_selector_returns_to_monochrome_an_emoji_that_goes_color_by_default() {
        // The half that gets forgotten: the rule has to work **in both directions**. With only the
        // top half, this case would pass the same and the test would tie nothing down.
        assert_eq!(
            presentation_of('😀', &[TEXT_PRESENTATION_SELECTOR]),
            Presentation::Text
        );
    }

    #[test]
    fn a_selector_that_is_not_first_does_not_speak_about_this_character() {
        // A variation selector applies to the character right in front of it. If it appears deeper
        // in the sequence, it refers to another — looking at it would attribute to this cell a presentation
        // that was asked for something else.
        assert_eq!(
            presentation_of('☁', &['\u{200D}', EMOJI_PRESENTATION_SELECTOR]),
            Presentation::Text
        );
    }

    #[test]
    fn an_ordinary_letter_is_not_an_emoji_however_much_it_carries_attached() {
        // An `a` with a combining accent attached. Without this case, a rule that returned Color as
        // soon as there was something attached would pass all the other tests.
        assert_eq!(presentation_of('a', &['\u{0301}']), Presentation::Text);
    }
    #[test]
    fn the_emoji_selector_over_an_emoji_leaves_it_color() {
        // **The case where a wrong branch order falls.** The result coincides with the one that ignoring
        // the selector would give, so without this case the contract table is left with a hole right where
        // the two conditions overlap.
        assert_eq!(
            presentation_of('😀', &[EMOJI_PRESENTATION_SELECTOR]),
            Presentation::Color
        );
    }

    #[test]
    fn the_text_selector_over_a_text_symbol_leaves_it_text() {
        // The other corner of the overlap, for the same reason: the two conditions point to the same side.
        assert_eq!(
            presentation_of('☁', &[TEXT_PRESENTATION_SELECTOR]),
            Presentation::Text
        );
    }
    #[test]
    fn a_character_that_is_not_an_emoji_does_not_go_color_for_carrying_the_selector_behind() {
        // **An emoji selector behind an `a` does not make the `a` an emoji.** Unicode only defines
        // variation sequences for characters with `Emoji=Yes`; behind any other, the
        // selector means nothing. And `a\u{FE0F}` is not a far-fetched case: it comes from any
        // program that cuts a string in half through an emoji, and even from a line break —the
        // engine attaches a loose selector to the EMPTY cell in front of it—, so without this rule
        // a blank space is sent to the color font.
        assert_eq!(
            presentation_of('a', &[EMOJI_PRESENTATION_SELECTOR]),
            Presentation::Text
        );
        assert_eq!(
            presentation_of(' ', &[EMOJI_PRESENTATION_SELECTOR]),
            Presentation::Text
        );
    }
    // ── Discovery ────────────────────────────────────────────────────────────────────────────────

    use eframe::egui;

    /// An egui context with its text engine already assembled.
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        pass(&ctx);
        ctx
    }

    /// **`set_fonts` does not take effect until the next pass**: it saves the configuration and the text
    /// engine is remade afterward. Without this, any check looks at the state from before.
    fn pass(ctx: &egui::Context) {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }

    /// What that character's glyph measures in the atlas, in pixels.
    ///
    /// **It is the only reliable oracle there is, and that is why the tests here use it instead of asking.**
    /// `Fonts::has_glyph` returns `false` for the `'a'` —see `Discoverer::resolve`—, so asserting
    /// on it would be asserting on false data. The drawing that comes out to the atlas does not lie.
    fn glyph_size(ctx: &egui::Context, c: char) -> Option<(u16, u16)> {
        ctx.fonts_mut(|f| {
            let galley = f.layout_no_wrap(
                c.to_string(),
                egui::FontId::monospace(14.0),
                egui::Color32::WHITE,
            );
            let uv = galley.rows.first()?.glyphs.first()?.uv_rect;
            Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
        })
    }

    /// Whether that character is being painted with **the replacement box**.
    ///
    /// It is compared against `U+10FFFD`, which lives in a private-use plane no normal font covers.
    /// **Measured in the same state as the character**: the atlas is remade on registering a face, so a
    /// size from before is not comparable with one from after.
    /// What the glyph measures **asking for the family the painter would ask for** for that character.
    fn with_its_family(
        ctx: &egui::Context,
        discoverer: &Discoverer,
        c: char,
    ) -> Option<(u16, u16)> {
        ctx.fonts_mut(|f| {
            let id = egui::FontId::new(14.0, discoverer.family_of(c));
            let uv = f
                .layout_no_wrap(c.to_string(), id, egui::Color32::WHITE)
                .rows
                .first()?
                .glyphs
                .first()?
                .uv_rect;
            Some((uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]))
        })
    }

    fn comes_out_as_a_box(ctx: &egui::Context, c: char) -> bool {
        let its_own = glyph_size(ctx, c);
        its_own.is_some() && its_own == glyph_size(ctx, '\u{10FFFD}')
    }

    #[test]
    fn ascii_is_never_noted() {
        // **It is the only thing that runs per cell**, so discarding the ordinary case without touching anything is
        // half of the milestone not costing anything. The factory fonts cover ASCII whole.
        let mut discoverer = Discoverer::default();
        for c in ' '..='~' {
            discoverer.note(c);
        }

        assert_eq!(
            discoverer.pending.len(),
            0,
            "the ASCII ended up in the queue of what has to be looked for"
        );
    }

    #[test]
    fn a_screen_of_garbage_does_not_eat_the_frame_looking_for_fonts() {
        // **This is the test that was missing, and its first version proved nothing.** It asserted on
        // `pending.len()` —the field where the value is recorded— and passed the same whether the work was
        // bounded or not, which is exactly the disguise the project rules name. What
        // has to be bounded is **the time spent looking**, and it is on that that this asserts.
        //
        // The scenario is real: a `cat` of a binary produces **tens of thousands** of distinct
        // characters, none covered by anything. Measured before bounding it: **6.0–6.6 ms sustained on every
        // frame**, more than the whole frame.
        let ctx = context();
        let mut discoverer = Discoverer::default();

        // **It asserts on the SEARCHES, not on milliseconds, and that is not dodging the problem.** The
        // tests run in debug, where everything costs an order of magnitude more than in the binary that
        // is published: a ceiling in milliseconds would measure the compilation profile as much as the code, and
        // would be green or red depending on where it was run. What the budget really bounds is **how many
        // searches fit in a round**, and that is indeed the same number here and in release.
        //
        // Ten frames with **two thousand new characters each**, like the ones a dump produces.
        let mut most_wanted = 0;
        for frame in 0..10u32 {
            for i in 0..2000u32 {
                if let Some(c) = char::from_u32(0xE000 + frame * 2000 + i) {
                    discoverer.note(c);
                }
            }
            most_wanted = most_wanted.max(discoverer.resolve(&ctx).searched);
            pass(&ctx);
        }

        // **The key is that it does NOT grow with the queue.** Without brakes the whole queue would be looked for —forty per
        // frame, indefinitely—, which is what cost 6.0–6.6 ms sustained.
        //
        // **And both brakes are needed: removing one alone does not bring this test down, removing both does.**
        // Checked by mutating them. It is not redundancy: they protect distinct scenarios. The budget bounds
        // the case where the searches **succeed** and are expensive —a CJK font weighs 20 MB—; the failure
        // brake bounds the case where **none** succeeds, where the budget is replenished every frame
        // and would never get to cut anything.
        assert!(
            most_wanted < MAX_PENDING,
            "{most_wanted} characters were looked for in a single round, which is the whole queue: the work \
             of looking for fonts is not bounded and grows with whatever the program spits"
        );
    }

    #[test]
    fn a_character_already_looked_for_is_not_looked_for_again() {
        // **The FAILURES have to be remembered too**, and that is the half that gets forgotten: a covered character
        // leaves its face registered, so the next one no longer looks; but one that NOBODY covers leaves
        // nothing, and without remembering it the candidates would be traversed every time it appeared — which is exactly what
        // a broken program spits in a loop. It comes from **D24**.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        // A private-use plane no normal font covers: the search is going to fail for sure.
        discoverer.note('\u{10FFFD}');
        let first = discoverer.resolve(&ctx);

        discoverer.note('\u{10FFFD}');
        let second = discoverer.resolve(&ctx);

        assert_eq!(first.searched, 1, "the first round did not get to look");
        assert_eq!(
            first.registered, 0,
            "it registered a face for something nobody covers"
        );
        assert_eq!(
            second.searched, 0,
            "it looked again for a character it had already given up as impossible"
        );
    }

    #[test]
    fn a_character_nobody_covers_leaves_the_window_standing_and_comes_out_as_a_box() {
        // **AC6.** What is not there is not invented: it is painted with whatever the engine gives and nothing
        // else is attempted. What this test defends is that that path **does not crash** or leave the engine half-done.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        discoverer.note('\u{10FFFD}');
        discoverer.resolve(&ctx);
        pass(&ctx);

        assert!(
            comes_out_as_a_box(&ctx, '\u{10FFFD}'),
            "a character nobody covers stopped coming out as a box, so something invented it"
        );
        assert!(
            !comes_out_as_a_box(&ctx, 'a'),
            "looking for an impossible character took the rest of the font down with it"
        );
    }
    #[test]
    fn japanese_stops_coming_out_as_a_box_when_the_machine_has_the_means() {
        // **AC1, and it is the reason for the whole milestone.** A terminal that shows a row of boxes where there is
        // Japanese is disqualified against the ones that are the bar (**D19**).
        //
        // **The test asserts in BOTH scenarios and that is why it does not skip itself**: if the machine has a
        // CJK font, the kanji has to stop being the box; if it does not have one, it has to keep being it
        // and the window has to stay standing — which is **AC6** and **AC9**. Skipping it on a bare machine
        // would leave the scarcity path uncovered, which is exactly where an implementation crashes.
        let ctx = context();
        assert!(
            comes_out_as_a_box(&ctx, '日'),
            "Japanese already showed before discovering anything: this test would not be proving the milestone"
        );

        let mut discoverer = Discoverer::default();
        discoverer.note('日');
        let round = discoverer.resolve(&ctx);
        pass(&ctx);

        if round.registered > 0 {
            assert!(
                !comes_out_as_a_box(&ctx, '日'),
                "a face was registered for the Japanese and the box keeps coming out"
            );
        } else {
            assert!(
                comes_out_as_a_box(&ctx, '日'),
                "no face was registered and yet the Japanese changed: something invented it"
            );
        }
        assert_eq!(round.searched, 1, "it did not get to look for it");
    }

    #[test]
    fn the_system_font_does_not_steal_the_glyph_from_the_terminals() {
        // **Contract 2, and it is what prevents this milestone from breaking what already worked.** The
        // fallback chain is traversed in order: a system face placed in front would take the letters away from the
        // terminal font, and the terminal would change appearance for discovering a kanji.
        let ctx = context();
        let before = glyph_size(&ctx, 'a');

        let mut discoverer = Discoverer::default();
        discoverer.note('日');
        discoverer.resolve(&ctx);
        pass(&ctx);

        assert_eq!(
            glyph_size(&ctx, 'a'),
            before,
            "registering a system face changed the drawing of an ordinary letter"
        );
    }

    #[test]
    fn an_icon_of_the_private_use_area_is_won_by_the_installed_font() {
        // **AC2, and the exception to the contract above.** egui ships a factory icon font that
        // occupies those areas, so without the exception **no box comes out: the wrong icon comes out** —and the
        // font the user installed never gets used—. Unicode gives no meaning to those
        // areas: who is right there is decided by whoever installed the font.
        //
        // **It asserts on the drawing, not on the order of a list.** Asserting that the face ended up
        // first would pass the same even if the engine ignored it afterward.
        let ctx = context();
        let icon = '\u{F015}';
        let before = glyph_size(&ctx, icon);

        let mut discoverer = Discoverer::default();
        discoverer.note(icon);
        let round = discoverer.resolve(&ctx);
        pass(&ctx);

        let factory = ["Hack".to_owned(), "emoji-icon-font".to_owned()];

        if round.registered > 0 {
            assert!(
                before.is_some() && before == glyph_size(&ctx, '\u{10FFFD}'),
                "the icon already showed before: this test would not be proving anything"
            );
            assert_eq!(
                discoverer.icon_ones(&factory).first().map(String::as_str),
                Some(discoverer.faces[0].family.as_str()),
                "the face discovered for an icon did not end up first in the icon family, so \
                 the factory font will keep winning the glyph from it"
            );
            // **And the monospaced one is NOT touched: it is the other half and without it the fix is worthless.**
            assert_eq!(
                discoverer.monospaced(&factory)[..factory.len()],
                factory,
                "the icon face sneaked into the monospaced family, where it would steal the letters from the \
                 terminal font"
            );
        } else {
            assert_eq!(
                discoverer.icon_ones(&factory),
                factory,
                "no face was registered and yet the icon family changed"
            );
        }
    }

    #[test]
    fn a_face_that_is_not_for_icons_stays_behind_the_factory_ones() {
        // **The other half of the exception, and without it "put everything in front" would pass the same.** It is the
        // symmetric error and the most expensive: a face found looking for a kanji, placed in front, would take the
        // letters away from the terminal font.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        discoverer.note('日');
        let round = discoverer.resolve(&ctx);

        let factory = ["Hack".to_owned()];
        if round.registered > 0 {
            let mono = discoverer.monospaced(&factory);
            assert_eq!(
                mono.first().map(String::as_str),
                Some("Hack"),
                "a face discovered by a kanji sneaked in front of the terminal font"
            );
            assert_eq!(mono.len(), 2, "the discovered face was not added");
            assert_eq!(
                discoverer.icon_ones(&factory),
                factory,
                "a face discovered by a kanji sneaked into the icon family"
            );
        }
    }
    /// A family this machine really has, or `None` when it has none.
    ///
    /// **It is asked of the machine instead of being written down**, because a name written here
    /// —`JetBrains Mono`, `DejaVu Sans Mono`— turns the test into "is that font installed on the
    /// machine running the suite", which is a question about the machine and not about this code.
    ///
    /// **With a capital in it**, because the tests that ask for it in lowercase have nothing to say
    /// about a family that is already lowercase — and which family `family_names()` hands back first
    /// is not something a test gets to choose.
    fn an_installed_family() -> Option<String> {
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        // **Sorted, and that is what makes these tests deterministic.** `fontique` backs that
        // iterator with a hash map seeded per process, so «the first family» is a **different one on
        // every run** — which is how one of these guards came out intermittent before a review
        // measured it. `Discoverer::look_for` sorts for this same reason, in production.
        let mut names: Vec<String> = collection.family_names().map(str::to_owned).collect();
        names.sort();
        names.into_iter().find(|name| name.to_lowercase() != *name)
    }

    /// The family the chosen face was registered under, or `None` if none was.
    fn registered_family(discoverer: &Discoverer) -> Option<&str> {
        discoverer.chosen.as_ref().map(|face| face.family.as_str())
    }

    #[test]
    fn the_configured_font_goes_in_front_of_the_factory_ones_and_they_stay_as_the_net() {
        // **AC1, and the named exception of Contract 7.** The fallback chain is traversed in order and
        // the factory font covers the whole Latin range: a chosen face placed behind it would not paint
        // a single letter, and the setting would do nothing while looking like it works.
        //
        // **It asserts on the list and not on the drawing**, for the reason the sibling guard above
        // states: registering a face remakes the atlas and **every** glyph changes place, so comparing
        // drawings passes the same with the defect in and out.
        let ctx = context();
        let Some(installed) = an_installed_family() else {
            return;
        };
        let mut discoverer = Discoverer::default();

        let chosen = discoverer.use_the_family(&installed, &ctx);

        // **A family the machine lists and cannot open is the machine's business, not this code's**:
        // a stale fontconfig entry or a half-written file leaves nothing to register. That is not a
        // failure of what this test claims, so it stops instead of failing — but being told the
        // family is MISSING would be this code's fault, and that still fails.
        let Chosen::Registered(family) = chosen else {
            assert_ne!(
                chosen,
                Chosen::NotInstalled,
                "the family '{installed}' came from the machine itself and was reported as missing"
            );
            return;
        };
        let factory = ["Hack".to_owned(), "emoji-icon-font".to_owned()];
        let mono = discoverer.monospaced(&factory);
        assert_eq!(
            mono.first().map(String::as_str),
            Some(format!("{CHOSEN_PREFIX}{family}").as_str()),
            "the configured font did not end up in front, so the factory one keeps winning every letter"
        );
        // **And the factory ones stay, in their order**: they are the net for what the chosen face does
        // not carry. A version that replaced them instead of pushing them back would pass the assertion
        // above and leave the terminal without a net.
        assert_eq!(
            mono[1..],
            factory,
            "the factory fonts stopped being behind the chosen one as a net"
        );
        // **And the engine really has the bytes.** Naming a family in the list without handing over its
        // `font_data` is not a wrong drawing: it is `is not bound to any fonts`, a **panic inside
        // epaint's own pass**, on the frame after this one. Nothing but taking a pass here sees it —
        // checked by dropping the chosen face from the registration loop, which left the suite green.
        pass(&ctx);
        assert!(
            glyph_size(&ctx, 'a').is_some(),
            "the chosen family was named to the engine without its bytes, which panics on the next pass"
        );
    }

    #[test]
    fn the_configured_font_does_not_rule_over_the_icons() {
        // **The private-use areas are the one place where the INSTALLED font rules**, and the one that
        // rules there is the one that carries the drawing —a `Nerd Font`—, not the one somebody picked
        // to read code with. Without this, asking for a font would take the icons away from whoever
        // installed one for them.
        let ctx = context();
        let Some(installed) = an_installed_family() else {
            return;
        };
        let mut discoverer = Discoverer::default();

        let chosen = discoverer.use_the_family(&installed, &ctx);

        // **The registration is asserted first, and without this the test proves nothing**: a
        // `use_the_family` that did nothing at all leaves the icon family untouched too, and the
        // assertion below would pass for the wrong reason.
        if !matches!(chosen, Chosen::Registered(_)) {
            // Same as its sibling above: nothing registered means nothing to assert on, and the one
            // answer that WOULD be this code's fault is the family being called missing.
            assert_ne!(
                chosen,
                Chosen::NotInstalled,
                "the family '{installed}' came from the machine itself and was reported as missing"
            );
            return;
        }
        let factory = ["Hack".to_owned(), "emoji-icon-font".to_owned()];
        assert_eq!(
            discoverer.icon_ones(&factory),
            factory,
            "the configured font sneaked into the icon family"
        );
    }

    #[test]
    fn a_font_the_machine_does_not_have_leaves_everything_as_it_was_and_says_so() {
        // **The name comes from outside and is not validated when reading the file**: whether it exists
        // is something only the machine can answer. What matters is that the answer «it is not here»
        // changes nothing —the terminal keeps painting— and that it is **told apart from the found
        // one**, because from the outside both look identical: the window opens the same.
        let ctx = context();
        let mut discoverer = Discoverer::default();

        let chosen = discoverer.use_the_family("no family is called like this 4f7a2b", &ctx);

        assert!(
            matches!(chosen, Chosen::NotInstalled | Chosen::NoFontsOnTheMachine),
            "a family nobody has was reported as something other than missing: {chosen:?}"
        );
        let factory = ["Hack".to_owned()];
        assert_eq!(
            discoverer.monospaced(&factory),
            factory,
            "a font that was not found changed the family the terminal paints with"
        );
    }

    #[test]
    fn the_family_is_found_however_it_is_capitalized() {
        // **Whoever configures types the name by hand**, and a name typed by hand does not carry the
        // capitals of the file it came from. Getting «it is not installed» for having written
        // `jetbrains mono` would be a failure whoever suffers it cannot even see.
        //
        // **What is asserted is the family, not its exact name.** A machine can have two entries
        // under the same name in different capitalizations, and which of the two the collection hands
        // back is its business; that they are the same family is ours. Asserting on the name itself
        // was intermittent for exactly that reason.
        let ctx = context();
        let Some(installed) = an_installed_family() else {
            return;
        };
        let mut discoverer = Discoverer::default();

        let chosen = discoverer.use_the_family(&installed.to_lowercase(), &ctx);

        let Chosen::Registered(family) = chosen else {
            assert_ne!(
                chosen,
                Chosen::NotInstalled,
                "'{installed}' was not found when asked for in lowercase, and the machine has it"
            );
            return;
        };
        assert_eq!(
            family.to_lowercase(),
            installed.to_lowercase(),
            "asking for it in lowercase registered a different family"
        );
    }

    #[test]
    fn the_family_is_registered_under_the_machines_name_and_not_the_one_typed() {
        // **The report says which family the terminal paints with**, and a report that echoes back
        // what was typed says nothing: whoever wrote `jetbrains mono` would read `jetbrains mono`
        // whatever the machine actually resolved.
        //
        // **Two discoverers and not one assertion on a literal**, because the machine's own spelling
        // is not something this test may assume. Asking for the same family twice, once as it is
        // spelled and once in lowercase, has to register **the same** name; echoing the typed string
        // is exactly what makes the two differ.
        let ctx = context();
        let Some(installed) = an_installed_family() else {
            return;
        };
        let mut as_spelled = Discoverer::default();
        let mut in_lowercase = Discoverer::default();

        as_spelled.use_the_family(&installed, &ctx);
        in_lowercase.use_the_family(&installed.to_lowercase(), &ctx);

        if registered_family(&as_spelled).is_none() {
            // Nothing registered: the machine lists a family whose files it cannot open. Same
            // reasoning as the guards above.
            return;
        }
        assert_eq!(
            registered_family(&as_spelled),
            registered_family(&in_lowercase),
            "the family is registered under the name that was typed, so the report tells you what you \
             wrote instead of what the terminal paints with"
        );
    }

    /// Whether that face is the family's Regular: upright, normal weight, normal width.
    ///
    /// **The three attributes are written out here instead of being asked of `regular_first`**,
    /// because a test that decides what «Regular» means by calling the thing under test agrees with
    /// it whatever either of them says.
    fn is_the_regular_one(face: &fontique::FontInfo) -> bool {
        face.style() == fontique::FontStyle::Normal
            && face.weight() == fontique::FontWeight::NORMAL
            && face.width() == fontique::FontWidth::NORMAL
    }

    #[test]
    fn the_regular_face_of_a_family_is_the_one_that_gets_picked_however_they_come_enumerated() {
        // **A family is not one face, and which of them is taken is not a detail.** Measured on the
        // discovery path by a review: `日` came out of face **index 5** of its family, taken for
        // being enumerated first, and in a family whose Bold happens to come first **every** letter
        // the terminal painted with it would be bold. Since this delivery the same rule decides the
        // family somebody asks for by name, where it is not one character but **all** of them.
        //
        // **The faces are handed in the worst order on purpose**, with a non-Regular one first. The
        // order the machine enumerates them in is its business and changes between machines, so a
        // test that took it as it comes would be green here and asleep elsewhere: what has to be
        // caught is a criterion that no longer reorders anything.
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        let mut names: Vec<String> = collection.family_names().map(str::to_owned).collect();
        names.sort();
        // **Every family that has a Regular AND something that is not one, and not the first one
        // found.** That is the only shape that says anything —in a family of one face any criterion
        // leaves it first— and going through all of them is what makes the criterion's four terms
        // meet a family that separates them: one machine's first candidate differs only in slope,
        // another's only in weight, and a test that stopped at one would be asleep about the rest.
        let candidates: Vec<Vec<fontique::FontInfo>> = names
            .iter()
            .filter_map(|name| {
                let id = collection.family_id(name)?;
                let faces = collection.family(id)?.fonts().to_vec();
                let regular = faces.iter().filter(|face| is_the_regular_one(face)).count();
                (regular == 1 && faces.len() > regular).then_some(faces)
            })
            .collect();
        // Same as its neighbours: on a machine with no fonts installed there is nothing to sort,
        // and that is a fact about the machine and not about this code.
        if candidates.is_empty() {
            return;
        }

        for mut faces in candidates {
            // **Handed in the worst order**, with a non-Regular first, so what is caught is a
            // criterion that no longer reorders anything and not a machine that happened to
            // enumerate them well.
            faces.sort_by_key(is_the_regular_one);

            regular_first(&mut faces);

            assert!(
                is_the_regular_one(&faces[0]),
                "the Regular of a family did not end up first, so whichever face the machine \
                 enumerates first is what the whole terminal gets painted with: {:?}",
                faces
                    .iter()
                    .map(|face| (face.style(), face.weight(), face.width()))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn on_a_machine_with_no_fonts_at_all_the_name_that_was_typed_is_not_blamed() {
        // **Three of the four endings look identical from outside** —the window opens with the
        // factory fonts either way— and telling them apart is the whole reason [`Chosen`] has four
        // variants instead of a `bool`. This is the one that is **nobody's typing**: a machine with
        // no fontconfig has no family to offer, so answering «that family is not installed» sends
        // whoever configured it to check a name that is written perfectly well, and the caller
        // prints that very sentence for `NotInstalled`.
        //
        // **The state is forced instead of uninstalling fontconfig**, the same way its neighbour
        // below does it: there is no way to take the machine's fonts away from inside a test.
        let ctx = context();
        let mut discoverer = Discoverer {
            no_system_fonts: true,
            ..Default::default()
        };

        let chosen = discoverer.use_the_family("JetBrains Mono", &ctx);

        assert_eq!(
            chosen,
            Chosen::NoFontsOnTheMachine,
            "with no fonts on the machine the answer blames the family that was asked for, and \
             whoever reads it goes off to fix a name that is fine"
        );
        // **And nothing was enumerated on the way there.** The answer above could also be reached by
        // building the collection and finding it empty, which is the 17.3–23.2 ms of D24 paid to
        // learn what was already known.
        assert!(
            !discoverer.has_searched_the_machine(),
            "the machine was enumerated again after having already found out it has no fonts"
        );
    }

    #[test]
    fn the_chosen_face_shares_its_name_with_neither_a_factory_one_nor_a_discovered_one() {
        // **This is why the chosen face carries a prefix of its own instead of its bare name.**
        // egui's keys are ordinary family names —`Hack` ships built in and is installed on very many
        // development machines— so registering the chosen one bare would **replace the built-in
        // one's bytes**, and the net this delivery leaves behind the chosen face for what it does
        // not carry would be the chosen face itself.
        //
        // **And the prefix cannot be the discovered ones' either.** Those go behind the factory
        // fonts and this one in front; sharing a key would leave the same name twice in the chain,
        // with one of the two positions holding the other one's bytes.
        //
        // **The collision is manufactured rather than waited for**, like its sibling below: whether
        // a family called `Hack` is installed on the machine running the suite is a question about
        // the machine and not about this code.
        let mut discoverer = Discoverer {
            chosen: Some(SystemFace {
                family: format!("{CHOSEN_PREFIX}Hack"),
                data: Arc::new(egui::FontData::from_static(&[])),
                for_icons: false,
            }),
            ..Default::default()
        };
        discoverer.faces.push(SystemFace {
            family: format!("{SYSTEM_PREFIX}Hack"),
            data: Arc::new(egui::FontData::from_static(&[])),
            for_icons: false,
        });

        let factory = ["Hack".to_owned(), "emoji-icon-font".to_owned()];
        let mono = discoverer.monospaced(&factory);

        assert_eq!(
            mono.iter().filter(|name| *name == "Hack").count(),
            1,
            "the chosen font named Hack took the place of the factory Hack, so the net behind it is \
             itself: {mono:?}"
        );
        assert!(
            mono.contains(&"Hack".to_owned()),
            "the factory Hack stopped being in the chain at all: {mono:?}"
        );
        // **Three distinct keys for three faces**: the engine is handed one `font_data` entry per
        // name, so two faces under one name means the chain names a face whose bytes are somebody
        // else's.
        let distinct: BTreeSet<_> = mono.iter().collect();
        assert_eq!(
            distinct.len(),
            mono.len(),
            "two of the three faces are registered under the same name, so one of them is painting \
             with the other's bytes: {mono:?}"
        );
    }

    #[test]
    fn without_fonts_on_the_machine_enumerating_is_not_retried_per_character() {
        // **AC9.** Enumerating is the most expensive of the whole path —between 17 and 23 ms, **D24**—, so on a
        // machine without fontconfig retrying it for every rare character that passes across the screen would freeze the
        // window in exchange for nothing. It is said once through the error output and it stops trying.
        //
        // **The test forces the state instead of uninstalling fontconfig**, and it is said: there is no way to
        // take the fonts away from the machine from within a test. What is tied down is that, once
        // in that state, the system is not touched again.
        let ctx = context();
        let mut discoverer = Discoverer {
            no_system_fonts: true,
            ..Default::default()
        };
        discoverer.pending.insert('日');

        let round = discoverer.resolve(&ctx);

        assert_eq!(
            round.registered, 0,
            "with the machine taken as empty a face was registered"
        );
        assert!(
            discoverer.collection.is_none(),
            "the system fonts were enumerated again after knowing there are none"
        );
    }
    #[test]
    fn discovering_an_icon_does_not_change_the_drawing_of_the_letters() {
        // **This is the test that was missing, and its absence let a serious defect through.** The first version
        // sneaked the icon face IN FRONT of the factory ones in the monospaced family, and since a
        // `Nerd Font` carries the full Latin set, **it changed the drawing of three out of every five letters**: the
        // terminal changed appearance for having seen an icon. The test that existed only looked at the kanji
        // case, which goes behind and would never have exposed it.
        //
        // **It is measured with the family the painter would ask for for each character**, which is the only thing that
        // describes what is seen: always asking for the monospaced one would hide precisely the
        // mechanism being tested.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        let letters: Vec<(char, Option<(u16, u16)>)> = "aWgñ0"
            .chars()
            .map(|c| (c, with_its_family(&ctx, &discoverer, c)))
            .collect();

        discoverer.note('\u{F015}');
        discoverer.resolve(&ctx);
        pass(&ctx);

        for (c, before) in letters {
            assert_eq!(
                with_its_family(&ctx, &discoverer, c),
                before,
                "discovering an icon changed the drawing of {c:?}: the icon font sneaked into the \
                 monospaced family and is stealing the letters from the terminal font"
            );
        }
    }

    #[test]
    fn the_icon_family_is_not_asked_for_until_it_exists() {
        // **Asking epaint for a named family nobody has declared is a PANIC**, not a warning:
        // "is not bound to any fonts". And the painter asks for a family on every cell from the first frame,
        // long before anything has been discovered. Checked: it crashed.
        let discoverer = Discoverer::default();

        assert_eq!(
            discoverer.family_of('\u{F015}'),
            egui::FontFamily::Monospace,
            "the icon family was asked for without having registered any icon face"
        );
    }

    #[test]
    fn the_icon_family_is_not_asked_for_in_the_frame_that_registers_it_either() {
        // **And this is the half that really crashes, because it is the path the application travels.**
        // `set_fonts` saves the configuration and the engine is remade **on the next pass**; but
        // discovery runs **before painting, on the same pass**, so between registering the face and
        // painting the cell there is no pass in between. Asking there for the family **crashes the
        // application**. Checked by running it: `is not bound to any fonts`.
        //
        // The test above does not cover it: that one looks at the initial state, and this defect only appears
        // **after** a successful registration.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        discoverer.note('\u{F015}');
        let round = discoverer.resolve(&ctx);

        if round.registered > 0 {
            assert_eq!(
                discoverer.family_of('\u{F015}'),
                egui::FontFamily::Monospace,
                "the icon family was offered in the same frame that declared it, when the engine \
                 does not yet know it: asking for it there is a panic"
            );
            // And on the next round yes, which is the other half: without it, "never offer it" would pass.
            discoverer.resolve(&ctx);
            assert_eq!(
                discoverer.family_of('\u{F015}'),
                egui::FontFamily::Name(ICON_FAMILY.into()),
                "the icon family never got to be offered"
            );
        }
    }
    #[test]
    fn a_system_font_named_like_a_factory_one_does_not_replace_it() {
        // **egui's keys are ordinary names** —`Hack`, `Ubuntu-Light`, `NotoEmoji-Regular`— and
        // `Hack` is installed on very many development machines. Without a prefix, registering it would change
        // **the bytes** of the factory one while keeping **its position at the front of the chain**: the terminal
        // would switch its whole font at once, without any character outside private use and without even passing
        // through the family tiling.
        //
        // **The collision is manufactured instead of waiting to have it**: this machine has no family
        // named `Hack`, so a test that depended on what is installed would prove nothing here and
        // would fail on someone else's machine.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        discoverer.faces.push(SystemFace {
            family: format!("{SYSTEM_PREFIX}Hack"),
            data: Arc::new(egui::FontData::from_static(&[])),
            for_icons: false,
        });

        let factory = ["Hack".to_owned(), "emoji-icon-font".to_owned()];
        let mono = discoverer.monospaced(&factory);

        assert_eq!(
            mono.iter().filter(|n| *n == "Hack").count(),
            1,
            "the system font named Hack took the place of the factory Hack"
        );
        assert_eq!(
            mono.first().map(String::as_str),
            Some("Hack"),
            "the factory one stopped being the first of the chain"
        );
        assert!(
            mono.iter().any(|n| n == "sistema:Hack"),
            "the system one did not get added with its own name"
        );
        let _ = ctx;
    }
    /// Whether this machine has the means to paint emojis in color.
    fn has_color(ctx: &egui::Context) -> bool {
        Discoverer::default()
            .color_glyph(ctx, '\u{1F600}', 20, 20)
            .is_some()
    }

    #[test]
    fn an_ascii_with_nothing_attached_never_goes_color() {
        // **It ties down a shortcut that lives in the painter**: the color pass discards loose ASCII with two
        // comparisons, without consulting these tables, because it is the vast majority of what passes across a
        // terminal screen. That shortcut duplicates a rule
        // that lives here, and duplicating a rule is only acceptable if something warns when the two stop
        // saying the same thing. This is that something.
        for n in 0..=127u8 {
            assert_eq!(
                presentation_of(char::from(n), &[]),
                Presentation::Text,
                "the loose ASCII character U+{n:04X} asks for color, and the painter discards it without \
                 asking: it would not come out"
            );
        }
    }

    #[test]
    fn an_ascii_key_with_its_selector_does_go_color() {
        // **The other half, and the one that exposed a real defect.** The first version of the painter's
        // shortcut discarded **all** ASCII in the color pass, and ate `#️⃣` `*️⃣` and `0️⃣`–`9️⃣`:
        // Unicode gives `Emoji=Yes` to the hash, the asterisk and the ten digits, so with the
        // selector behind they are full-fledged emojis. The test above found it on running it.
        for c in "#*0123456789".chars() {
            assert_eq!(
                presentation_of(c, &['\u{FE0F}']),
                Presentation::Color,
                "\"{c}\" with the emoji selector behind stayed text: it is the base of a key and \
                 has to come out color"
            );
        }
    }

    #[test]
    fn the_color_glyph_is_rasterized_once_and_then_remembered() {
        // **The cache is not an optional optimization: without it, rasterizing happens on EVERY frame.** It is 45 to
        // 80 µs per emoji and per frame —measured—, that is a screen with twenty emojis would eat
        // more than a millisecond repeating identical work sixty times a second.
        //
        // **It asserts on the COUNTER of rasterized ones and not on the size of the map**, because the map
        // also keeps the failures: an implementation that always looked and saved nothing would leave the
        // map just as large.
        let ctx = context();
        let mut discoverer = Discoverer::default();

        discoverer.color_glyph(&ctx, '\u{1F600}', 20, 20);
        let first = discoverer.color_cost();
        assert_eq!(
            first.rasterized + first.failed,
            1,
            "the first time it did not get to try to rasterize anything"
        );

        for _ in 0..10 {
            discoverer.color_glyph(&ctx, '\u{1F600}', 20, 20);
        }
        let after = discoverer.color_cost();
        assert_eq!(
            (after.rasterized, after.failed),
            (0, 0),
            "ten more requests for the same emoji did work again: nothing is being remembered"
        );
        assert_eq!(
            after.spent,
            std::time::Duration::ZERO,
            "time was counted for consulting something already remembered: the report stretch would stop describing \
             what rasterizing costs"
        );
    }

    #[test]
    fn changing_the_cell_height_throws_away_what_is_remembered() {
        // **Without this, raising the desktop zoom leaves the emojis of the previous size painted in cells
        // of the new one** — and since the cache is never emptied, for the rest of the process's life.
        //
        // **It asserts on the SIZE of the returned texture, not on the map having been emptied.** A
        // cache that empties and rasterizes again at the old size would meet the second and would keep
        // painting wrong, which is the defect this exists to prevent.
        let ctx = context();
        if !has_color(&ctx) {
            return;
        }
        let mut discoverer = Discoverer::default();
        let (_, _, small) = discoverer
            .color_glyph(&ctx, '\u{1F600}', 20, 20)
            .expect("the machine has a color font and gave no glyph");
        let (_, _, big) = discoverer
            .color_glyph(&ctx, '\u{1F600}', 40, 40)
            .expect("it gave no glyph at the new height");

        assert!(
            big.y > small.y,
            "with the cell at double the height the emoji came out at {small:?}, the size of the previous \
             cell: the cache is returning the old one"
        );
        assert_eq!(
            discoverer.glyphs_remembered(),
            1,
            "both heights were left saved: the cache did not empty and the memory grows with each zoom"
        );
    }

    #[test]
    fn on_the_atlas_filling_remembering_stops_and_it_is_said() {
        // **AC8.** The ceiling of this milestone is a fixed-size texture: the memory the graphics card
        // dedicates to the emojis is 1 MiB and does not change, whether one or a thousand appear. Who decides how many distinct
        // characters appear is the program inside —a `cat` of an emoji file would traverse
        // them all—, so without a ceiling the texture would have to grow without limit.
        //
        // **BOTH halves are tied down**: that it stops accepting on filling, and that it says so. A silent ceiling is
        // indistinguishable from a failure — whoever sees their emojis go monochrome all at once would have no way to
        // know that the terminal knows how to paint them and has run out of room.
        //
        // **It is filled with synthetic images and not with real emojis**, and it is said: filling it for real
        // demands that the machine have eight hundred distinct emojis installed, which is a condition of the
        // machine and not of the code.
        let ctx = context();
        let mut glyphs = Glyphs::default();
        let side = ATLAS_SIDE / 8;
        let square = || egui::ColorImage::filled([side, side], egui::Color32::from_rgb(10, 20, 30));

        let mut pasted = 0;
        while glyphs.blit(&ctx, square()).is_some() {
            pasted += 1;
            assert!(pasted <= 8 * 8, "the atlas accepted more than fits in it");
        }
        assert_eq!(
            pasted,
            8 * 8,
            "the atlas did not use its surface: {} squares of {side} pixels fit and {pasted} entered",
            8 * 8
        );
        assert!(
            glyphs.said_the_ceiling,
            "the atlas filled and it was not said through the error output: a silent ceiling reads as a failure"
        );
    }

    #[test]
    fn two_distinct_glyphs_do_not_overlap_within_the_atlas() {
        // **It is the failure mode particular to putting them all in one texture**, and no test above
        // covers it: placing one wrong paints it on top of another, and then two distinct emojis come out the same.
        // With one texture per glyph this could not happen; with an atlas, it can.
        //
        // **All the pairs of a long batch are checked and not just the first two**, because the place
        // where they overlap is the **row jump**: two glyphs in a row fall fine with almost any count,
        // and the one that starts a row is the one that gets placed on top of the one above.
        let ctx = context();
        let mut glyphs = Glyphs::default();
        // Distinct widths on purpose: with all equal, a count that ignored the real width and
        // advanced a fixed step would place them fine anyway.
        let spots: Vec<_> = (0..80)
            .filter_map(|n| {
                let width = 15 + n % 7;
                glyphs.blit(
                    &ctx,
                    egui::ColorImage::filled([width, 17], egui::Color32::RED),
                )
            })
            .collect();
        assert!(
            spots.len() > 60,
            "only {} glyphs 17 pixels tall fit in an atlas of {ATLAS_SIDE}: the test does not \
             get to jump rows several times",
            spots.len()
        );

        let side = ATLAS_SIDE as f32;
        for (n, one) in spots.iter().enumerate() {
            for other in &spots[n + 1..] {
                let overlap = one.uv.intersect(other.uv);
                // **Less than half a pixel of overlap, not exact zero**: two adjacent glyphs share their
                // edge, and in floating point that edge does not give a clean width of zero.
                assert!(
                    overlap.width() * side < 0.5 || overlap.height() * side < 0.5,
                    "two glyphs overlap within the atlas ({:?} and {:?}): one is painted on top of the other \
                     and the two emojis come out the same",
                    one.uv,
                    other.uv
                );
            }
        }
    }

    #[test]
    fn a_monochrome_drawing_is_not_painted_by_the_color_path() {
        // **It is the finding of the first delivery, and here is where it is resolved.** A font can *cover*
        // an emoji and paint it in black and white —on this machine `DejaVu Sans` takes the 😀 with
        // `Noto Color Emoji` installed—, so "the first that covers it" is the wrong rule.
        //
        // What this test ties down is the half checkable without depending on what fonts there are: **an alpha
        // mask never comes out here**, and an alpha mask is exactly a monochrome drawing.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        // The 'a' is not an emoji and no emoji font brings it in color: it has to come out without a glyph.
        assert!(
            discoverer.color_glyph(&ctx, 'a', 20, 20).is_none(),
            "an ordinary letter came out by the color path"
        );
        let cost = discoverer.color_cost();
        assert_eq!(
            cost.rasterized, 0,
            "an ordinary letter was rasterized as if it were an emoji"
        );
        assert_eq!(
            cost.failed, 1,
            "the failed attempt was not counted, so the report does not declare it"
        );
    }

    #[test]
    fn a_character_without_a_color_glyph_is_not_looked_for_again() {
        // **Remembering the failure is not a detail.** Without it, a character that no color font covers
        // would traverse the emoji families **on every frame** — and it is exactly the character a broken program
        // spits in a loop.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        // **A character that NO font paints in color, and that is why it is Unicode's unassigned one.** The
        // first version of this test used a snowman and proved nothing: `Noto Color Emoji`
        // brings it, so it was remembered by the success path and removing the memory of the **failure**
        // left it passing the same. Mutating it exposed it.
        assert!(
            discoverer
                .color_glyph(&ctx, REPLACEMENT_BOX, 20, 20)
                .is_none(),
            "the unassigned character came out color: the test is not measuring the failure path"
        );
        let first = discoverer.color_cost();
        assert_eq!(first.failed, 1, "the failed attempt was not counted");

        for _ in 0..5 {
            discoverer.color_glyph(&ctx, REPLACEMENT_BOX, 20, 20);
        }
        let after = discoverer.color_cost();
        assert_eq!(
            after.rasterized + after.failed,
            0,
            "a character already looked for was looked for again: the whole traversal is paid per \
             frame while it is on screen"
        );
    }

    #[test]
    fn a_single_column_emoji_stays_within_it() {
        // **The case the criterion forces and that looks odd, so it is worth having it written down.** The terminal
        // engine gives **a single cell** to `☁️`, `❤️` and `✔️` —checked by running it: they arrive as
        // `Single` with the selector attached, while `😀` arrives as the head of a wide character—, and the
        // criterion says an emoji does not invade its neighbor beyond what its width declares.
        //
        // **Consequence, said with its number**: in a cell of 13×20 points, which is this machine's
        // with two panes, that emoji comes out at 13×12 — 60% of the line height, centered. It looks
        // small and **it is the correct thing**: painting it at the line height would cover what the program
        // wrote in the cell next to it, and who decides a character's width is the engine, not this
        // project.
        let ctx = context();
        if !has_color(&ctx) {
            return;
        }
        let mut discoverer = Discoverer::default();
        let (_, _, measurements) = discoverer
            .color_glyph(&ctx, '\u{2601}', 20, 13)
            .expect("the machine has a color font and gave no glyph");
        assert!(
            measurements.x <= 13.0,
            "the single-column emoji came out at {measurements:?} in a cell 13 wide: it eats the neighbor \
             and covers what the program wrote there"
        );
        assert!(
            measurements.x as u32 == 13,
            "the single-column emoji came out at {measurements:?} without filling the 13 of width it has: it looks \
             smaller than it fits"
        );
    }

    #[test]
    fn the_emoji_is_rescaled_to_fit_in_the_slot_it_is_given() {
        // **AC5, from the side of whoever rasterizes.** The painter checks the rectangle that comes out to screen;
        // here it is checked that the image handed to it already comes bounded, which is what makes that
        // rectangle fit.
        let ctx = context();
        if !has_color(&ctx) {
            return;
        }
        let mut discoverer = Discoverer::default();
        for (height, width) in [(20u32, 40u32), (40, 20), (17, 17)] {
            let (_, _, measurements) = discoverer
                .color_glyph(&ctx, '\u{1F600}', height, width)
                .expect("the machine has a color font and gave no glyph");
            assert!(
                measurements.y <= height as f32 && measurements.x <= width as f32,
                "for a slot of {width}x{height} the emoji came out at {measurements:?}: it goes out of its place"
            );
            // And fitting is not enough: it has to **fill** one of the two axes, or it would be wasting
            // space and would come out smaller than the cell gives.
            assert!(
                measurements.y as u32 == height || measurements.x as u32 == width,
                "for a slot of {width}x{height} the emoji came out at {measurements:?}: it fills no axis, \
                 so it looks smaller than it fits"
            );
        }
    }

    #[test]
    fn on_reaching_the_family_ceiling_registering_stops_and_it_is_said() {
        // **A ceiling that swallows what it discards is indistinguishable from a failure.** Whoever sees a box
        // after installing the right font would have no way to know that the terminal found it and
        // decided not to use it. And the case happens: without a perfect oracle extra faces are registered, and a
        // session that spends its slots on redundant faces will leave a real language in boxes.
        //
        // **The state is forced instead of waiting to fill it for real**, and it is said: filling eight slots
        // with real fonts demands that the machine have eight families covering eight distinct scripts,
        // which is a condition of the machine and not of the code.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        for n in 0..MAX_FAMILIES {
            discoverer.faces.push(SystemFace {
                family: format!("{SYSTEM_PREFIX}filler{n}"),
                data: Arc::new(egui::FontData::from_static(&[])),
                for_icons: false,
            });
        }

        discoverer.note('日');
        let round = discoverer.resolve(&ctx);

        assert_eq!(
            round.searched, 1,
            "with the ceiling full the character was not even looked at"
        );
        assert_eq!(
            round.registered, 0,
            "a face was registered above the ceiling"
        );
        assert_eq!(
            discoverer.faces.len(),
            MAX_FAMILIES,
            "the face list went past its ceiling"
        );
        assert!(
            discoverer.said_the_ceiling,
            "the ceiling was reached and it was not said through the error output: a silent ceiling reads as a failure"
        );
    }

    #[test]
    fn a_bitmap_that_is_a_monochrome_mask_does_not_come_out_by_the_color_path() {
        // **An alpha mask is a black-and-white drawing, and by the color path it would come out tinted
        // with whatever was in the channel.** It is the same finding the delivery already has written —a
        // font can *cover* an emoji without bringing it in color— but by the other path: here the font is
        // an emoji one and does bring a bitmap for that glyph, only monochrome. Whoever asks for it has to
        // fall to the text path, which is Contract 5.
        //
        // **The mask is built by hand and it is said why**: on this machine `Noto Color Emoji` rules,
        // which brings PNG, so no installed font travels this branch. It was checked by mutating it:
        // turning the mask into an image left the 647 tests green.
        let mask = skrifa::bitmap::BitmapData::Mask(skrifa::bitmap::MaskData {
            bpp: 8,
            is_packed: false,
            data: &[0xFF, 0x80, 0x40, 0x00],
        });

        assert!(
            pixels_of(&mask, 2, 2).is_none(),
            "a monochrome drawing came out by the color path: it is painted as an image and comes out tinted with \
             whatever the channel carries, instead of with the color the program asked for that cell"
        );
    }

    #[test]
    fn a_bitmap_in_bgra_keeps_the_colors_the_emoji_has() {
        // **The font delivers the channels backwards and they have to be swapped.** Without that a red emoji
        // comes out blue and a blue one comes out red — a defect that **does not show on this machine**, because here a
        // font that brings PNG wins, and that would show with the ones that deliver the bitmap uncompressed.
        // It was checked by mutating it: removing the swap broke no test.
        //
        // An opaque red pixel, written as the font writes it: blue, green, red, alpha.
        let bgra = skrifa::bitmap::BitmapData::Bgra(&[0, 0, 255, 255]);

        let pixels = pixels_of(&bgra, 1, 1).expect("a pixel in BGRA gave no image");

        assert_eq!(
            pixels.get_pixel(0, 0).0,
            [255, 0, 0, 255],
            "the emoji came out with red and blue swapped"
        );
    }

    #[test]
    fn a_transparent_pixel_does_not_carry_its_color_to_the_edge_of_the_emoji() {
        // **It is the halo, and it is the classic defect of reducing a drawing with alpha.** An emoji is a figure
        // inside an empty box, and the empty part of a PNG carries color even if it is not seen: averaged without
        // premultiplying, that invisible color mixes into the edge of the figure and leaves it a fringe. It
        // would show in **all** the emojis, not in a rare case, because all of them are reduced from 128 pixels to
        // seventeen.
        //
        // **It asserts on the transparent pixel and not on the already-reduced edge**, because it is where the
        // defect shows without depending on what drawing the installed font brings: premultiplied, what is not
        // seen has no color to contribute. It was checked by mutating it: without premultiplying, the suite passed
        // whole.
        let mut original = image::RgbaImage::new(2, 1);
        original.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        // Fully transparent white: it is not seen, but its color is written in the file.
        original.put_pixel(1, 0, image::Rgba([255, 255, 255, 0]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(original)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("the test PNG could not be written");

        let pixels = pixels_of(&skrifa::bitmap::BitmapData::Png(&png), 2, 1)
            .expect("the test PNG gave no image");

        assert_eq!(
            pixels.get_pixel(1, 0).0,
            [0, 0, 0, 0],
            "an invisible pixel kept its color: on reducing the emoji that color is averaged with the \
             drawing's and leaves a halo around it"
        );
        assert_eq!(
            pixels.get_pixel(0, 0).0,
            [255, 0, 0, 255],
            "the opaque pixel lost color on premultiplying: the whole emoji would come out muted"
        );
    }

    #[test]
    fn a_cell_without_height_or_without_width_does_not_bring_down_the_window() {
        // **It is a panic, not an odd drawing.** The rescaling bounds the final size between one and what the
        // cell gives, and with a cell of zero that bound asks for a minimum greater than its maximum — the
        // standard library panics there, so the whole window crashes. And the cell does get to
        // be worth zero: the height and the width come from multiplying its rectangle by the physical pixels and
        // rounding, and that of a console crushed against the edge measures a fraction of a point.
        //
        // It was checked by mutating it: without the guard, this test does not fail — it blows up.
        let ctx = context();
        let mut discoverer = Discoverer::default();

        assert!(
            discoverer.color_glyph(&ctx, '\u{1F600}', 0, 20).is_none(),
            "a cell without height gave a glyph"
        );
        assert!(
            discoverer.color_glyph(&ctx, '\u{1F600}', 20, 0).is_none(),
            "a cell without width gave a glyph"
        );
    }

    #[test]
    fn with_the_atlas_full_no_texture_is_invented() {
        // **AC8 from the side of whoever asks, which is the one that decides what is seen.** That the atlas stops
        // accepting is already tested above; what is tied down here is that an emoji asked for with the atlas full
        // **returns nothing** instead of any old piece of the texture. Without this, the natural way to
        // "fix" the ceiling —returning the last spot, or a default rectangle— paints in each new emoji
        // the drawing of another, and whoever looks will see the wrong face.
        //
        // **It is filled with synthetic images and the height is fixed on it first**, because a height different from
        // the one the atlas has inside empties it whole: without fixing it, the request below would find
        // plenty of room and this would prove nothing.
        let ctx = context();
        let mut discoverer = Discoverer::default();
        discoverer.glyphs.height = 20;
        let side = ATLAS_SIDE / 4;
        while discoverer
            .glyphs
            .blit(
                &ctx,
                egui::ColorImage::filled([side, side], egui::Color32::RED),
            )
            .is_some()
        {}

        assert!(
            discoverer.color_glyph(&ctx, '\u{1F600}', 20, 20).is_none(),
            "with the atlas full a texture was returned anyway: it is the piece another glyph occupies, so \
             the emoji comes out with another's drawing"
        );
    }
}
