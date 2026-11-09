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

## Bundled themes

Theme origins, modifications and attribution are documented in
[the theme catalog](resources/themes/README.md). Retain the accompanying notices:

- [Catppuccin](resources/themes/catppuccin-LICENSE)
- [GitHub](resources/themes/github-LICENSE)
- [Nord](resources/themes/nord-LICENSE)
- [Rose Pine](resources/themes/rose-pine-LICENSE)

## Test font

`crates/app/tests/fixtures/ligatures/JetBrainsMono-Regular.ttf` is distributed under the
[SIL Open Font License 1.1](crates/app/tests/fixtures/ligatures/OFL.txt).
Its origin is documented in [the fixture notes](crates/app/tests/fixtures/ligatures/README.md).

## Cargo dependencies

External Cargo dependencies retain their individual licenses; the resolved versions are in
`Cargo.lock`. Their licensing metadata is available through `cargo metadata --locked` and in each
dependency's source distribution. The root GPLv3 license grants no additional rights to these works.
Binary distributions must retain the notices required by the dependencies and assets they include.
