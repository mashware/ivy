# Prose crib for the remaining deliveries (#13, #15, #16)

**Why this file exists.** D27's annex decides **identifiers**. It does not decide the ordinary words the
prose uses, and those are 5 499 appearances in `lib.rs`'s comments alone. Delivery #12 split its
translation across seven agents without a shared list and the result was inconsistent **between
files** — `measure.rs` used the glossary's central term correctly 34 times out of 34 while `panel.rs`
got it wrong 27 out of 27, and nothing in the suite could see it. With a single file, the same gap
produces inconsistency **inside** one file, which is worse to read.

**Every row below is backed by something already merged**, not by preference: either the glossary names
the identifier, or the already-translated files settled the prose word. Where a term is genuinely
context-dependent it says so instead of pretending.

## The one that has already been got wrong twice

| Spanish | English | Evidence, and the trap |
|---|---|---|
| `división` (the thing) | **pane** | `docs/02-decisiones.md:3457`, `:3470`, `:3553`. **`split` is the GESTURE** (`Action::SplitRight`, `key.split-right`, `close-pane`) and D27 forbids reusing a word already spent. #12 called the thing a "split" in 41 places across 11 files. |
| `reparto` | **tiling** | `:3494`, `:3504-3505`; and the types are `Tiling` and `PanelTiling`. **Never "split" and never "division"** — the second resurrects the very word D27 retired. #12 used both, 11 places. |
| `tanda` | **batch** | `scripts/measure-frame.sh` ("EVERY BATCH IS RUN TWICE"). **And its letters `A`–`R` are never translated** (`:3524`). |
| `corrida` | **run** | Same source: "the two **runs** of the same **batch**". #12 rendered both as "run" in `session.rs`, producing *"every run of the run"*. |
| `fuente` | **font** *or* **source** | **A false friend that inverted a claim in #12**: `la fuente de epaint` is its *source code*, not a typeface. Decide per sentence: a typeface is `font`, a library's code is `source`. |

## Settled by the merged translations

| Spanish | English | | Spanish | English |
|---|---|---|---|---|
| `consola` | console | | `hoja` | leaf |
| `ventana` | window | | `rama` | branch |
| `pestaña` | tab | | `peso` | weight |
| `fotograma` | frame | | `cola` | queue |
| `celda` | cell | | `techo` | ceiling |
| `fila` | row | | `suelo` | floor |
| `columna` | column | | `tramo` | span |
| `rejilla` | grid | | `bandera` | flag |
| `disposición` | layout | | `vigilante` | watcher |
| `maqueta` | layout (already laid out text) | | `productor` | producer |
| `hueco` | gap · slot | | `informe` | report |
| `frontera` | border (the panel's) · divider (the tree's) | | `medida` | measurement |
| `divisor` | divider | | `pasada` | pass |
| `gesto` | gesture | | `envoltura` | wrapping |
| `arrastre` | drag | | `recorte` | clip |
| `reclamo` | claim | | `selección` | selection |
| `marca` | mark (the `panel.json` key) | | `historial` | scrollback |
| `sonda` | probe | | `presupuesto` | budget |
| `bombeo` | pumping | | `volcado` | dump |
| `ratón` | mouse · `teclado` keyboard · `pintor` painter | | | |

## Three rules that are not vocabulary

1. **The verb is not the noun.** "splits the batch", "splits the text", "can be split in two",
   "splitting requires a human gesture" are all **correct** — there are ~65 such uses in `app` alone. A
   blind `split`→`pane` replacement corrupts more than it fixes; anchor on the determiner and then read
   each hit.
2. **A comment that quotes a literal quotes it as the code has it.** #12 translated the quoted literals
   inside the prose that explained them and left three comments arguing for a guard with a claim that is
   false. If the literal stays Spanish, the quotation stays Spanish.
3. **Literals whose LENGTH is the payload are measured in columns, not words.** `examples/screen_demo.rs`
   has rows built so a wide character lands on the column-80 boundary, and that file has zero tests.

## What to run after each region, not at the end

- `cargo test -p app --test prose` — the guard that did not exist until #12. It names every deliberate
  Spanish island.
- `cargo test -p app --test glossary` — identifiers, and the citation guard that reads backticks.
- `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` ·
  `cargo fmt --all -- --check`
- `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps` — AC3's other half.
  It prints 12 `private_intra_doc_links` warnings that are **pre-existing**; compare the set, not the count.
- **And a cross-region sweep by term**, counting: the defect this file exists to prevent is invisible
  inside any one region.
