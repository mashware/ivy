# Ivy identity

The terminal's public name is **Ivy**, pronounced approximately **eye-vee**.
The project owner selected the name and approved this logo in [#339](https://github.com/mashware/agent-terminal/issues/339).

- `ivy.svg`: the approved vector artwork, with a transparent background and its green-to-teal gradient.
- `ivy.png`: a transparent 512×512 rendering of that artwork, padded to a square without stretching.

The application embeds the PNG for its title bar and native window icon. The SVG remains the
editable source. Native window icons depend on the platform: X11 and Windows support them;
Wayland launcher icons require a matching installed desktop entry, and macOS Dock icons require
an application bundle. Those installers are separate from the in-window branding.

The repository, executable, Cargo package names, configuration directory and panel contract keep
their existing identifiers. Existing settings and saved layouts therefore remain in
`$XDG_CONFIG_HOME/agent-terminal/`.
