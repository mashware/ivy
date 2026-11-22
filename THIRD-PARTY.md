# Third-party notices

The root [GNU GPLv3 license](LICENSE) applies to Ivy's original code and documentation. It does not replace
licenses or copyright notices belonging to third-party code and assets.

## Vendored egui-winit

`vendor/egui-winit` contains a locally patched copy of egui-winit 0.36.1 from
[emilk/egui](https://github.com/emilk/egui/tree/0.36.1/crates/egui-winit).
The original code is licensed under **MIT OR Apache-2.0**. Both upstream license texts are preserved
in [LICENSE-MIT](vendor/egui-winit/LICENSE-MIT) and
[LICENSE-APACHE](vendor/egui-winit/LICENSE-APACHE). The local input-routing patches are described
in D33 of [the decisions](docs/02-decisions.md).

## Vendored epaint

`vendor/epaint` contains a locally patched copy of epaint 0.36.1 from
[emilk/egui](https://github.com/emilk/egui/tree/0.36.1/crates/epaint).
The original code is licensed under **MIT OR Apache-2.0**, with both texts retained in
[LICENSE-MIT](vendor/epaint/LICENSE-MIT) and [LICENSE-APACHE](vendor/epaint/LICENSE-APACHE).
Its targeted font-hinting patch is documented in [IVY-PATCH.md](vendor/epaint/IVY-PATCH.md)
and D101 of [the decisions](docs/02-decisions.md).

## Bundled themes

Theme origins, modifications and attribution are documented in
[the theme catalog](resources/themes/README.md). Retain the accompanying notices:

- [Catppuccin](resources/themes/catppuccin-LICENSE)
- [GitHub](resources/themes/github-LICENSE)
- [Nord](resources/themes/nord-LICENSE)
- [Rose Pine](resources/themes/rose-pine-LICENSE)

## Bundled fonts

Geist and Geist Mono (`resources/fonts/*.ttf`), by Vercel in collaboration with basement.studio, are
distributed under the [SIL Open Font License 1.1](resources/fonts/Geist-OFL.txt). They are copied
unmodified from the `geist` 1.9.2 package and embedded in the binary as Ivy's interface and default
terminal fonts (D95).

## Interface icons

The Phosphor icon font (Phosphor Regular), by Phosphor Icons, is embedded in the binary through the
`egui-phosphor` 0.14.0 crate and draws every interface icon (D103). Both are under the MIT License;
their notices are kept in [Phosphor-MIT.txt](resources/fonts/Phosphor-MIT.txt).

## Windows console host

The Windows installer carries `conpty.dll` and `OpenConsole.exe` from Microsoft's
`Microsoft.Windows.Console.ConPTY` package (version and checksums pinned in
[conpty.json](packaging/windows/conpty.json)), copied unmodified beside `Ivy.exe` so an installed
Ivy runs its consoles on OpenConsole rather than the system's `conhost` (D112). They are under
the MIT License from [microsoft/terminal](https://github.com/microsoft/terminal); the notice is
kept in [conpty-MIT.txt](resources/licenses/windows/conpty-MIT.txt) and installed as
`licenses/conpty/LICENSE`.

## Test fonts

`crates/app/tests/fixtures/ligatures/JetBrainsMono-Regular.ttf` is distributed under the
[SIL Open Font License 1.1](crates/app/tests/fixtures/ligatures/OFL.txt).
Its origin is documented in [the fixture notes](crates/app/tests/fixtures/ligatures/README.md).

`crates/app/tests/fixtures/colr/colr_1.ttf` is distributed under the
[Apache License 2.0](crates/app/tests/fixtures/colr/colr_1_LICENSE).
Its origin is documented in [the fixture notes](crates/app/tests/fixtures/colr/README.md).

## Cargo dependencies

External Cargo dependencies retain their individual licenses; the resolved versions are in
`Cargo.lock`. Their licensing metadata is available through `cargo metadata --locked` and in each
dependency's source distribution. The root GPLv3 license grants no additional rights to these works.
Binary distributions must retain the notices required by the dependencies and assets they include.
