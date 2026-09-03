//! Measures **what it costs to hand the terminal the machine's fonts**, which is what decides how
//! the rest of the milestone is built.
//!
//! ```text
//! cargo run --release --example fonts -p app
//! ```
//!
//! **It does not touch production.** Just as the history bench measured the engine beneath the facade
//! of `term-core`, here `fontique` and `skrifa` are measured **before** either one enters the
//! application: the number first, and the number decides what code is needed.
//!
//! **In debug the numbers come out several times worse.** The one that counts is the `--release` one.
//!
//! ## Three things the first version of this bench did wrong, and why they matter
//!
//! 1. **It published the first family it found, and the order is not stable.** Two runs in a row
//!    gave `Noto Sans Mono CJK JP` and `Noto Sans Mono CJK HK` for the same character, and `Noto Color
//!    Emoji` and `DejaVu Sans` for the same emoji. A bench whose result changes between runs does not
//!    measure: **here the families are ordered by name** so the answer is reproducible.
//! 2. **A single sample per point is not a measurement.** Enumerating gave 13.6 ms and 24.3 ms in two
//!    runs of the same binary. Several are taken and the **median of the warm ones** is published, as
//!    the history bench did after its review forced it to.
//!
//!    **But that does not reach the cold column, and it is the limit of this bench.** The cold one
//!    **happens once per run** —sample 1 fills the caches and the other four already find them full—, so
//!    in cold there are not five samples to average: there is **one**. And it is exactly the one the lazy
//!    path pays. Its spread is only seen **by repeating the run**: three in a row with the machine at
//!    rest moved the deciding number from 3.4 to 4.8 ms. That is why **D24** publishes the cold column as
//!    a **range of runs** and not as a single value, and why whoever repeats this measurement has to run
//!    it **more than once**.
//! 3. **"Covers the character" is not "paints it in color".** `DejaVu Sans` maps 😀 to a **monochrome**
//!    glyph and wins by alphabetical order. It is exactly the defect Alacritty has documented —it takes
//!    the DejaVu glyph and only falls back to Noto Emoji if that one does not carry it—, so here we
//!    **also** ask whether that face has color tables.
//!
//! ## The four questions it answers
//!
//! 1. **How long it takes to enumerate the machine's fonts.** Decides whether discovery is paid **at
//!    startup** —where there are already 5.42 ms measured for restoring the layout— or only when a
//!    character appears that nobody covers.
//! 2. **What it costs to find who covers a character**, by the two paths: going through everything and
//!    asking by script or by generic family.
//! 3. **What it costs to register the faces in the text engine, and how much memory it takes.** It is
//!    what would be paid when opening the window, and it goes separately from enumerating because they
//!    are two distinct costs.
//! 4. **How much a color glyph takes up and in what form it comes.** Decides the cache ceiling and
//!    whether the emoji is rescaled from a bitmap or drawn at the requested size.
//!
//! **What this bench does NOT answer, and it has to be said:** what it costs to *paint* the already
//! rasterized glyph. That lives in the frame and is measured with a batch, not here.

use std::time::{Duration, Instant};

use eframe::egui;
use fontique::{Collection, CollectionOptions, GenericFamily, Script, SourceCache};

/// How many times each measurement is repeated within a run. Repeating instead of measuring once is
/// what the history bench ended up doing after its review knocked down the single sample.
///
/// **Five samples give a warm median of four and a cold of one.** Raising this constant does not
/// improve the cold: there is still a single cold sample per run, because the second one already finds
/// the caches full. What improves the cold is **repeating the run**.
const SAMPLES: usize = 5;

/// The body size of the font this terminal paints with today, copied from the application.
///
/// **The cell height is NOT invented: it is asked of the text engine**, which is the rule the painter
/// has written down —"the cell measurement is asked, not invented"—. The first version of this bench
/// put a 17 by eye, and on top of that treated it as pixels when it is **points**: with the desktop at
/// 200 % the cell is twice as many pixels and the cache **four times** bigger, which is exactly the
/// number its ceiling would come from. The review uncovered it.
const BODY: f32 = 14.0;

/// The desktop scale factors the cache size is published with.
///
/// **Two and not one**, because the ceiling is sized in pixels and the desktop decides how many there
/// are per point. **D10** already established that zoom cannot change these figures silently.
const SCALES: [f32; 2] = [1.0, 2.0];

/// The characters coverage is asked about, and why each one.
///
/// **None is decorative:** each one represents one of the things that today come out as a box.
const SAMPLE: [(char, &str); 5] = [
    ('日', "CJK — the D3 case"),
    ('한', "hangul — another coverage, another font"),
    ('\u{e0b0}', "Private Use Area — the icon of a modern `ls`"),
    ('😀', "emoji with default color presentation"),
    ('☁', "emoji with default TEXT presentation"),
];

fn main() {
    println!("── what it costs to hand the terminal the machine's fonts ──");
    println!(
        "cold: THE sample 1, just one — warm: the median of the remaining {}",
        SAMPLES - 1
    );
    println!("the cold happens once per run: to know how much it moves, repeat this run\n");

    // ── 1. Enumerate ───────────────────────────────────────────────────────────────────────────
    //
    // **The whole construction is timed**, which is what a startup would do: measuring from inside
    // would give a number nobody pays.
    let mut times = Vec::new();
    let mut count = 0;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: true,
            ..Default::default()
        });
        count = collection.family_names().count();
        times.push(started.elapsed());
    }
    println!("1. enumerate the system fonts");
    println!("   {}  ·  {count} families", summary(&times));
    if count == 0 {
        println!(
            "   ⚠ NO family at all: either there is no fontconfig running, or there are no fonts."
        );
        println!(
            "     It is the case AC9 forces us to withstand, and this bench has just reproduced it."
        );
        return;
    }

    let mut collection = Collection::new(CollectionOptions {
        system_fonts: true,
        ..Default::default()
    });
    // **Ordered by name**, which is what makes the answer reproducible across runs.
    let mut families: Vec<String> = collection.family_names().map(str::to_owned).collect();
    families.sort();
    let mut cache = SourceCache::default();

    // ── 2. Who covers what, going through everything ───────────────────────────────────────────
    println!("\n2. who covers what, opening the {count} families in order (the worst case)");
    for (which, reason) in SAMPLE {
        let mut times = Vec::new();
        let mut found = None;
        for _ in 0..SAMPLES {
            let started = Instant::now();
            found = who_covers(&mut collection, &mut cache, &families, which);
            times.push(started.elapsed());
        }
        // **And whether the winner paints it in color, because that is where the finding was.** Without
        // this column, the bench said "covered" and kept quiet that whoever wins the emoji on this
        // machine paints it in black and white while having a color font installed.
        let winner = found.as_ref().map_or_else(
            || "(nobody covers it)".to_owned(),
            |q| {
                let color = if q.renders_color(which) {
                    " [in color]"
                } else {
                    " [monochrome]"
                };
                format!("{}{color}", q.family)
            },
        );
        println!("   {which}  {}  {winner}   ({reason})", summary(&times));
    }

    // ── 3. The two fast paths ──────────────────────────────────────────────────────────────────
    //
    // **Going through everything is the worst case on purpose; this is what the application would
    // use.** The difference between the two numbers decides how the delivery that makes CJK visible is
    // written.
    println!("\n3. the fast paths: candidates by script and by generic family");
    for (which, script) in [('日', "Hani"), ('한', "Hang")] {
        let mut times = Vec::new();
        let mut candidates = 0;
        let mut found = None;
        for _ in 0..SAMPLES {
            let started = Instant::now();
            let ids: Vec<_> = collection
                .fallback_families(Script::parse(script).expect("valid script code"))
                .collect();
            candidates = ids.len();
            found = first_that_covers(&mut collection, &mut cache, &ids, which);
            times.push(started.elapsed());
        }
        let winner = found.as_ref().map_or_else(
            || "(no candidate covers it)".to_owned(),
            |q| q.family.clone(),
        );
        println!(
            "   {which}  {}  {winner}   ({candidates} candidates by script)",
            summary(&times)
        );
    }
    // **The emoji does NOT come out by script**: `Zsym` does not cover it on this machine. The path
    // that does is the generic family the system declares for that.
    let mut times = Vec::new();
    let mut candidates = 0;
    let mut found = None;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let ids: Vec<_> = collection.generic_families(GenericFamily::Emoji).collect();
        candidates = ids.len();
        // **With the rule the decision concludes, not with "the first one that covers".** And that was
        // a review correction: measuring only the coverage, the number did not include continuing to
        // search when the first candidate turns out to be monochrome — which is exactly the case this
        // bench discovered by going through in order. Here it worked by luck, because on this machine
        // the color one is first; on another it would not be, and the published number would be that of
        // a shorter path than the real one.
        found = first_in_color(&mut collection, &mut cache, &ids, '😀');
        times.push(started.elapsed());
    }
    let winner = found
        .as_ref()
        .map_or_else(|| "(none covers it)".to_owned(), |q| q.family.clone());
    println!(
        "   😀  {}  {winner}   ({candidates} candidates by generic family)",
        summary(&times)
    );

    // ── 4. Register the faces in the text engine ───────────────────────────────────────────────
    //
    // **It is what would be paid when opening the window**, and it goes separately from enumerating
    // because they are two distinct costs: one is charged by the system and the other by the framework.
    println!("\n4. register the faces in the text engine");
    register(&mut collection, &mut cache, &families);

    // ── 4bis. Register a face HOT ──────────────────────────────────────────────────────────────
    //
    // **This is the number that decides between the two paths**, and without it the decision would be
    // a preference. If discovery is lazy, the new face is registered with the window already open: the
    // text engine is rebuilt and that cost **falls within a frame**, not before the first one. A frame
    // is 16.7 ms at 60 Hz, so whatever comes out here says whether it is noticed as a jolt or not.
    println!("\n4bis. register ONE more face with the window already open (the lazy path)");
    register_hot(&mut collection, &mut cache);

    // ── 5. The color glyph ─────────────────────────────────────────────────────────────────────
    println!("\n5. in what form a color glyph comes, and how much it takes up");
    match found {
        Some(face) => measure_color(&face),
        None => println!("   no emoji family covers 😀 on this machine"),
    }

    println!("\n── what this bench does NOT say ──");
    println!(
        "what it costs to PAINT the already rasterized glyph: that lives in the frame and a batch measures it."
    );
}

/// A specific face of a font, with what is needed to ask it things.
///
/// **It carries the borrowed `Blob` and not a copy of the bytes**, and that was a review finding:
/// copying the font file **inside the stopwatch** made the timings measure the memory allocator and
/// not the search. With `Noto Sans CJK HK` —19.5 MiB— the copy cost 5 ms and the published number
/// followed the **file size**, not how many families had been gone through. A `Blob` is a
/// reference-counted pointer, which is what the application would have.
struct Face {
    family: String,
    blob: fontique::Blob<u8>,
    index: u32,
}

impl Face {
    /// Whether that face brings the glyph of that character **in color**.
    ///
    /// **It is asked OUTSIDE the stopwatch**, for the same reason as the copy: probing the color tables
    /// is opening the whole font with `skrifa`, and inside the clock it inflated all the search
    /// measurements.
    fn renders_color(&self, which: char) -> bool {
        use skrifa::MetadataProvider as _;
        skrifa::FontRef::from_index(self.blob.data(), self.index).is_ok_and(|font| {
            let Some(glyph) = font.charmap().map(which) else {
                return false;
            };
            let hay_bitmap = font
                .bitmap_strikes()
                .glyph_for_size(skrifa::instance::Size::unscaled(), glyph)
                .is_some();
            hay_bitmap || font.color_glyphs().get(glyph).is_some()
        })
    }
}

/// The first family, in order, whose face covers that character.
fn who_covers(
    collection: &mut Collection,
    cache: &mut SourceCache,
    families: &[String],
    which: char,
) -> Option<Face> {
    let ids: Vec<_> = families
        .iter()
        .filter_map(|name| collection.family_id(name))
        .collect();
    first_that_covers(collection, cache, &ids, which)
}

/// The first of those families whose face covers the character **and paints it in color**.
///
/// **It is the rule the decision concludes, and that is why it is measured this way**: "the first one
/// that covers" takes the glyph even if it is monochrome, which is the defect this bench found and that
/// Alacritty has documented. Probing the color costs opening the font with `skrifa`, so this path is
/// **more expensive** than the coverage-only one — and that is exactly the number needed.
fn first_in_color(
    collection: &mut Collection,
    cache: &mut SourceCache,
    ids: &[fontique::FamilyId],
    which: char,
) -> Option<Face> {
    for &id in ids {
        let Some((name, faces)) = collection
            .family(id)
            .map(|family| (family.name().to_owned(), family.fonts().to_vec()))
        else {
            continue;
        };
        for face in faces {
            let Some(blob) = cache.get(face.source()) else {
                continue;
            };
            let candidate = Face {
                family: name.clone(),
                blob,
                index: face.index(),
            };
            if candidate.renders_color(which) {
                return Some(candidate);
            }
        }
    }
    None
}

/// The first of those families whose face covers the character.
fn first_that_covers(
    collection: &mut Collection,
    cache: &mut SourceCache,
    ids: &[fontique::FamilyId],
    which: char,
) -> Option<Face> {
    for &id in ids {
        // **The faces are cloned before continuing**: the family borrows from the collection, and the
        // collection is needed on the next round.
        let Some((name, faces)) = collection
            .family(id)
            .map(|family| (family.name().to_owned(), family.fonts().to_vec()))
        else {
            continue;
        };
        for face in faces {
            let Some(blob) = cache.get(face.source()) else {
                continue;
            };
            let covers = face
                .charmap_index()
                .charmap(blob.data())
                .is_some_and(|strike| strike.map(which as u32).is_some_and(|glyph| glyph != 0));
            if covers {
                return Some(Face {
                    family: name,
                    blob,
                    index: face.index(),
                });
            }
        }
    }
    None
}

/// What it costs to register in the text engine the faces that are needed, and how much they weigh.
fn register(collection: &mut Collection, cache: &mut SourceCache, families: &[String]) {
    let needed: Vec<Face> = SAMPLE
        .iter()
        .filter_map(|(which, _)| who_covers(collection, cache, families, *which))
        .collect();

    let mut definitions = egui::FontDefinitions::default();
    let mut total_bytes = 0usize;
    let mut registered = 0usize;
    for face in &needed {
        if definitions.font_data.contains_key(&face.family) {
            continue;
        }
        total_bytes += face.blob.data().len();
        definitions.font_data.insert(
            face.family.clone(),
            std::sync::Arc::new(egui::FontData::from_owned(face.blob.data().to_vec())),
        );
        definitions
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(face.family.clone());
        registered += 1;
    }

    // **Several samples, like everything else.** The first version published a single one under the
    // label "median of five", which is the same rule this module forbids in its point 2 — and it moved
    // by 16 % between runs.
    let mut times = Vec::new();
    for _ in 0..SAMPLES {
        let ctx = egui::Context::default();
        let started = Instant::now();
        ctx.set_fonts(definitions.clone());
        // **A pass is forced**: `set_fonts` stores the configuration and the text engine is rebuilt on
        // the next frame. Timing only the call would publish a zero that nobody pays.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("日本語 ☁ 😀");
        })
        .drop_without_applying_deltas();
        times.push(started.elapsed());
    }
    println!(
        "   {}  ·  {registered} faces  ·  {:.1} MiB of font in memory",
        summary(&times),
        total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "   and that memory is the WHOLE font file per face, not just the glyphs that are used"
    );
}

/// What it costs to add a face to the text engine **with the window already running**.
fn register_hot(collection: &mut Collection, cache: &mut SourceCache) {
    // **The face that THE CHOSEN PATH returns, not the one from the alphabetical sweep**, and that was
    // a review correction. The two paths give different fonts —`Droid Sans Fallback` of 3.8 MiB against
    // `Noto Sans CJK JP` of 19.5 MiB— so adding "search by script" with "register what the sweep found"
    // was adding two halves of different gestures, and the second with a font five times smaller.
    let ids: Vec<_> = collection
        .fallback_families(Script::parse("Hani").expect("valid script code"))
        .collect();
    let Some(face) = first_that_covers(collection, cache, &ids, '日') else {
        println!("   there is no face that covers 日 by the chosen path on this machine");
        return;
    };

    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.insert(
        face.family.clone(),
        std::sync::Arc::new(egui::FontData::from_owned(face.blob.data().to_vec())),
    );
    definitions
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(face.family.clone());

    let mut times = Vec::new();
    for _ in 0..SAMPLES {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("hello");
        })
        .drop_without_applying_deltas();
        let started = Instant::now();
        ctx.set_fonts(definitions.clone());
        // The frame in which the text engine is rebuilt **and** something that uses the new face is
        // painted: that is what the user would see as a jolt, if there were one.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("日本語");
        })
        .drop_without_applying_deltas();
        times.push(started.elapsed());
    }
    let how_long = *times.iter().max().expect("there are samples");

    println!(
        "   {}  ·  {} ({:.1} MiB)",
        summary(&times),
        face.family,
        face.blob.data().len() as f64 / (1024.0 * 1024.0)
    );
    // **It is judged against the WORST of the times, not the best.** What decides whether it is seen as
    // a jolt is the bad frame, not the good one.
    let budget = Duration::from_micros(16_667);
    if how_long > budget {
        println!(
            "   ⚠ exceeds the budget of one frame at 60 Hz ({:.1}×): it would be seen as a jolt",
            how_long.as_secs_f64() / budget.as_secs_f64()
        );
    } else {
        println!(
            "   fits in one frame at 60 Hz ({:.0} % of the 16.7 ms)",
            100.0 * how_long.as_secs_f64() / budget.as_secs_f64()
        );
    }
}

/// The height of a cell, asked of the text engine as the painter does.
fn cell_height() -> f32 {
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label("M");
    })
    .drop_without_applying_deltas();
    ctx.fonts_mut(|cache| cache.row_height(&egui::FontId::monospace(BODY)))
}

/// In what form the color glyph comes and how much it takes up.
fn measure_color(face: &Face) {
    use skrifa::MetadataProvider as _;
    use skrifa::instance::Size;

    println!(
        "   {} · does it bring color? {}",
        face.family,
        yes_no(face.renders_color('😀'))
    );
    let Ok(font) = skrifa::FontRef::from_index(face.blob.data(), face.index) else {
        println!("   the font could not be read");
        return;
    };
    let Some(glyph) = font.charmap().map('😀') else {
        return;
    };

    let height = cell_height();
    let bitmaps = font.bitmap_strikes();
    let hit = bitmaps
        .glyph_for_size(Size::new(height), glyph)
        .or_else(|| bitmaps.glyph_for_size(Size::unscaled(), glyph));
    match hit {
        Some(strike) => {
            let pixels = strike.width as usize * strike.height as usize;
            println!(
                "   bitmap {}×{} px  ·  {} KiB once DECODED to RGBA",
                strike.width,
                strike.height,
                pixels * 4 / 1024
            );
            println!(
                "   (the font stores it compressed: {:.1} MiB the whole file with all its glyphs)",
                face.blob.data().len() as f64 / (1024.0 * 1024.0)
            );
            println!("   cell {height:.1} pt tall, asked of the text engine:");
            for scale in SCALES {
                let side = (height * scale) as usize;
                let bytes = side * side * 4;
                println!(
                    "     at {:>4.0} %: {side}×{side} px · {bytes} B per glyph · {} KiB a thousand glyphs",
                    scale * 100.0,
                    bytes * 1000 / 1024
                );
            }
        }
        None => {
            println!("   no bitmap: its color comes by outlines and is drawn at the requested size")
        }
    }
}

fn yes_no(which: bool) -> &'static str {
    if which { "yes" } else { "no" }
}

/// **The first sample apart from the median of the others**, and that was a review correction.
///
/// `SourceCache` keeps the files it opens and the search by script has its own cache, so sample 1 is
/// **cold** and the rest **warm**. Publishing the median of the five gave the warm number under a label
/// that said "median of five", and **the lazy path pays the cold one** — once per new write, which is
/// exactly what happens in production. Measured: 875 µs warm against 5.2 ms cold for the same
/// character. Publishing only the first underestimated the number that holds up the decision **by
/// four**.
fn summary(times: &[Duration]) -> String {
    let cold = times[0];
    let mut warms: Vec<Duration> = times[1..].to_vec();
    warms.sort_unstable();
    let median = warms[warms.len() / 2];
    format!("{cold:>11.3?} cold · {median:>11.3?} warm")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(how_long: u64) -> Duration {
        Duration::from_millis(how_long)
    }

    /// Five samples as they come out of a loaded machine: **the cold one is not the slowest** —there is
    /// a warm one that doubles it— and **they do not arrive sorted**. With sorted samples, or with the
    /// cold one always being the maximum, almost any wrong way of summarizing them gives the same number
    /// as the correct one and the test ties nothing down.
    ///
    /// Cold = 3 ms (the first). Warm = 12, 8, 5, 1 → sorted 1, 5, 8, 12 → median **8 ms**.
    fn samples() -> [Duration; 5] {
        [ms(3), ms(12), ms(8), ms(5), ms(1)]
    }

    /// The two numbers the bench publishes, already separated and without the format padding.
    fn published(times: &[Duration]) -> (String, String) {
        let line = summary(times);
        let (frio, caliente) = line
            .split_once(" · ")
            .expect("the summary publishes BOTH numbers, the cold and the warm one");
        (frio.trim().to_owned(), caliente.trim().to_owned())
    }

    #[test]
    fn the_cold_sample_is_the_first_one_and_not_the_slowest() {
        // **Cold is a position, not a magnitude.** The file cache and the search-by-script cache fill
        // up on sample 1, so the cold one is that one by construction; on a loaded machine any warm
        // sample can come out the slowest of the five. Publishing "the worst" under the label "cold"
        // would hang on the lazy path a cost it does not pay, and that is the number **D24** uses to
        // decide between discovering at startup or discovering when needed.
        let (frio, _) = published(&samples());

        assert_eq!(
            frio, "3.000ms cold",
            "what is published as cold is not the first sample"
        );
    }

    #[test]
    fn the_warm_median_sorts_the_samples_and_leaves_out_the_cold_one() {
        // **Two halves, and both publish a different number if they fall.** Without sorting, "median" is
        // the sample that landed in the middle of the vector —5 ms here— and the number moves with the
        // arrival order, which is what correction 2 of this bench forbids. And putting the cold one into
        // the heap, the median gets contaminated with the only sample that pays for the empty caches:
        // the 5.2 ms against 875 µs that this bench measured for the same character are the reason they
        // go separately.
        let (_, caliente) = published(&samples());

        assert_eq!(
            caliente, "8.000ms warm",
            "the warm median did not come from sorting the four samples left after removing the cold one"
        );
    }
}
