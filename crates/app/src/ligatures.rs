//! Controlled ligature measurement startup and included painting attribution.
use crate::{Window, fonts, measure, settings::Settings};
use eframe::egui;
use skrifa::MetadataProvider;
use std::time::{Duration, Instant};

const IDENTITY: &str = "JetBrainsMono-v2.304-Regular";
const SCENE: &str = "programming-ascii-v1";

pub(crate) struct Measurement {
    ctx: egui::Context,
    registration: Duration,
}

fn mode(value: &str) -> Result<bool, &'static str> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("MEASURE_LIGATURES must be exactly on or off"),
    }
}

/// Normal startup remains unchanged; only an explicit benchmark mode selects a fixture.
pub(crate) fn configure(
    window: &mut Window,
    settings: &Settings,
    ctx: &egui::Context,
    reading: measure::ReadingSettings,
) {
    window.reading_settings = reading;
    let Some(requested) = std::env::var_os("MEASURE_LIGATURES") else {
        window.chosen_font =
            fonts::choose_the_font(settings.font.as_deref(), &mut window.discoverer, ctx);
        return;
    };
    let requested = requested.to_str().expect("MEASURE_LIGATURES is not UTF-8");
    let enabled = mode(requested).expect("invalid ligature measurement mode");
    assert!(
        matches!(reading, measure::ReadingSettings::Vetoed),
        "ligature measurement requires the settings veto"
    );
    let started = Instant::now();
    let path =
        std::env::var_os("MEASURE_LIGATURE_FONT").expect("MEASURE_LIGATURE_FONT is required");
    let bytes = std::fs::read(path).expect("could not read the explicit ligature measurement font");
    validate_font(&bytes).expect("invalid ligature measurement font");
    fonts::use_measurement_face(
        &mut window.discoverer,
        egui::FontData::from_owned(bytes),
        ctx,
    );
    window.ligatures = enabled;
    // Same fractional size in both modes; historical batches keep their original size.
    window.font_size = 14.7;
    window.ligature_measurement = Some(Measurement {
        ctx: ctx.clone(),
        registration: started.elapsed(),
    });
}

fn validate_font(bytes: &[u8]) -> Result<(), &'static str> {
    // The report names one pinned release, so another valid face is not interchangeable.
    let fingerprint = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    if fingerprint != 5339921062256255186 {
        return Err("font differs from the pinned JetBrains Mono fixture");
    }
    let font = skrifa::FontRef::from_index(bytes, 0).map_err(|_| "font bytes cannot be parsed")?;
    if !(32u8..=126).all(|c| font.charmap().map(char::from(c)).is_some()) {
        return Err("font does not cover the benchmark's printable ASCII");
    }
    Ok(())
}

/// This stopwatch is INSIDE painting: publishing it must never subtract it again.
pub(crate) fn publish(window: &Window, to: &mut dyn std::io::Write) -> std::io::Result<()> {
    let cost = window.discoverer.ligature_cost();
    if let Some(probe) = &window.ligature_measurement {
        publish_scene(probe, window.font_size, window.ligatures, to)?;
    }
    writeln!(
        to,
        "Ligature work: {:.3} us (included in painting; never subtracted twice)",
        cost.spent.as_secs_f64() * 1e6
    )?;
    writeln!(
        to,
        "Ligature effects: eligible_runs={} shaped_runs={} fallback_runs={} vertices={}",
        cost.eligible_runs, cost.shaped_runs, cost.fallback_runs, cost.vertices
    )
}

fn publish_scene(
    probe: &Measurement,
    font_size: f32,
    enabled: bool,
    to: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    writeln!(
        to,
        "Ligature scene: {SCENE} font={IDENTITY} size={} scale={} mode={}",
        font_size,
        probe.ctx.pixels_per_point(),
        if enabled { "on" } else { "off" }
    )?;
    writeln!(
        to,
        "Ligature startup: {:.3} us (explicit font read, validation and registration; before first frame)",
        probe.registration.as_secs_f64() * 1e6
    )?;
    Ok(())
}

/// Real explicit-face startup producer, without opening a PTY or a control socket.
#[cfg(test)]
pub(crate) fn reference_report() -> String {
    let ctx = egui::Context::default();
    let started = Instant::now();
    let bytes = include_bytes!("../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf");
    validate_font(bytes).expect("pinned fixture");
    let mut discoverer = fonts::Discoverer::default();
    fonts::use_measurement_face(&mut discoverer, egui::FontData::from_static(bytes), &ctx);
    let probe = Measurement {
        ctx,
        registration: started.elapsed(),
    };
    let mut report = Vec::new();
    publish_scene(&probe, 14.7, true, &mut report).expect("memory report");
    String::from_utf8(report).expect("text report")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn benchmark_modes_reject_ambiguous_values() {
        assert_eq!(mode("on"), Ok(true));
        assert_eq!(mode("off"), Ok(false));
        for bad in ["", "1", "true", "ON", "off "] {
            assert!(mode(bad).is_err());
        }
    }
    #[test]
    fn fixture_validation_rejects_corruption_before_registration() {
        let bytes = include_bytes!("../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf");
        assert!(validate_font(bytes).is_ok());
        assert!(validate_font(&bytes[..16]).is_err());
        assert!(validate_font(b"not a font").is_err());
    }
}
