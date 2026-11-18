# Ivy epaint patch

Source: crates.io epaint0.36.1, MIT OR Apache-2.0; licenses retained locally.

Only runtime delta: src/text/font.rs caches whether a font prefers its interpreter but has no
glyph instructions, then selects skrifa automatic hinting at both creation and reconfiguration.
CFF/unknown outline formats and fonts with any native glyph instructions retain AutoFallback.
The font scan runs once per loaded face and only when hinting is enabled.

Cargo.toml only changes the license include paths to the local retained files.
No other runtime source changes. Coverage policy lives in Ivy startup, not in this dependency.

Behavioral guard: crates/app/tests/font_rasterization.rs checks Ubuntu bitmap changes and exact
Geist native-mask stability at11pt/scales1,2. Remove this patch when upstream covers this case.
