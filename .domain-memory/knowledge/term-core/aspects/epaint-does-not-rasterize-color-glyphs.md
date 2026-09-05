---
id: asp_epaint-does-not-rasterize-color-glyphs_291m37
slug: epaint-does-not-rasterize-color-glyphs
name: >-
  epaint 0.36 does not know how to rasterize a color glyph: it only draws
  outlines, so registering Noto Color Emoji would not even give the silhouette
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-25T13:58:00.081Z'
updated_at: '2026-08-25T13:58:00.081Z'
last_verified: '2026-08-25T13:58:00.081Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/app/Cargo.toml
symbols:
  - 'Painter::image'
  - FontDefinitions
  - 'FontData::from_owned'
  - paint_character
content_hashes:
  crates/app/src/painter.rs: 'sha256:db9bad5bd4d6b139d467d7b4690dc2b403a89ab5833919cb838db97c1170e0fb'
  crates/app/Cargo.toml: 'sha256:ca1a2443192911db903084df39c10e571f399a47c41b1583296970127d428aac'
tags:
  - egui
  - epaint
  - fonts
  - emoji
  - F2.9
  - D3
feature_id: feat_term-core_nx0g1r
---

## What it does

**epaint 0.36.1 does NOT know how to paint a color glyph, and that reorders any emoji milestone.** Read
in its source, not assumed: its rasterizer (`text/font.rs:241`) takes **only** `outline_glyphs` and
paints the outline with `set_paint(WHITE)`; the color is put on afterwards by the mesh tinting the
result. In its whole text module there is not **a single** mention of a font's color tables (COLR/CPAL,
CBDT/CBLC, sbix).

**The practical consequence, and it is the one that saves a whole delivery:** registering
`Noto Color Emoji` as one more face **would not give emojis in color**. It would give the silhouette —
and since its glyph is a **136×128 bitmap with no outlines** (verified the hard way), **it would give
nothing at all**.

### What egui does give, and why the path exists

- **Per-family fallback chain, already done.** `Font` wraps several faces (`text/font.rs:663`) and
  `resolve_face(c)` looks for which one covers the character, with a per-character cache at `:734`. **CJK
  and the icons do come out** just by giving it faces.
- **The atlas is a `ColorImage` with genuinely premultiplied RGBA** (`font.rs:283-292`): the place where
  a color glyph would live **exists**.
- **`Painter::image`** (`egui/painter.rs:447`) paints a rectangle with a texture. And this painter already
  draws **cell by cell** with a `painter.text` of a single character, so painting an emoji as an image is
  **one more branch in an `if` that already exists**, not an architecture change.
- **`skrifa`** (the crate epaint already uses underneath) brings `color/` and `bitmap.rs`: the missing
  piece **is already in the tree**.

> **That is why Alacritty's or Ghostty's architecture was NOT copied.** They rasterize everything
> themselves because **they have no interface framework underneath**; doing it here would be redoing what
> egui does well and would on top of that invalidate the entire measurement record. What gets copied is
> **the result**, not the how.

### A warning epaint writes itself

`text/font.rs:753-758`: the advance of a **fallback** face is computed with the metrics of the
**primary** one. For **placing**, it does not affect a painter that fixes each cell's rectangle; for the
**size**, it does, and the face has to be resolved beforehand or else declared and measured.

### And a compilation consequence

**Linking fontconfig demands `pkg-config` and its headers** — verified: a machine without them **does not
compile the repo**, and it fails in the dependency's build script, not in the code. It is loaded at
**runtime** instead, the same technique —and the same reason— by which `libwayland` arrives second-hand.
The price declared: its absence is discovered **on starting up** instead of on compiling.

**And watch out for «development-only» dependencies**: adding `fontique` moved `windows-sys` from 0.59 to
0.60 for the clipboard, which **does** go into the binary. On Linux nothing changes; on Windows it
already changes what gets compiled.

## Where it lives

The cell-by-cell painter is in `crates/app/src/painter.rs` (`paint_character`). The font readings are from
the local cargo registry: `epaint 0.36.1` (`src/text/font.rs`, `src/text/fonts.rs`) and `egui 0.36.1`
(`src/painter.rs`). The verdict on egui lives in **D3** and the decision on discovery in **D24**.
