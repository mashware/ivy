# Bundled theme sources

The eight JSON files are Windows Terminal palettes from
[iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes),
revision `b84cd1e68c9033d967fccd9febbac3072d127b7b`, directory
`windowsterminal/`. Their original MIT license notices are retained here:

- Catppuccin Mocha and Latte: `catppuccin-LICENSE` (catppuccin/palette).
- GitHub Dark Default and Light Default: `github-LICENSE` (primer/github-vscode-theme).
- Nord: `nord-LICENSE` (nordtheme/iterm2).
- Rose Pine, Dawn and Moon: `rose-pine-LICENSE` (rose-pine/build).

Run `python3 scripts/generate-themes.py` after changing fixtures. The application
uses generated Rust constants and does not load these JSON files at runtime.
