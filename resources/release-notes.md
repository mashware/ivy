# Ivy 0.1.0-alpha.6

## New in this alpha

- Start Codex by typing `codex` in an Ivy console: its status panel now stays attached to that console, even when another Codex client is already running. New conversations and local resume work without changing Codex's global settings.
- New installers include the helper needed for this integration in Bash, Zsh and Fish. Restart Ivy after upgrading and use Connect agents for the existing MCP setup.
- Ivy now answers terminal capability queries used by clients such as Codex.
- Preferences and saved layouts now use Ivy's configuration directory, with migration from the previous agent-terminal directory.

## Since 0.1.0-alpha.4

- The Overview is now a **board**: `Ctrl+Shift+O` (or the same menu entry) shows every console grouped into one section per Git repository, with its name and colour, and «Other consoles» at the end for everything outside a repository.
- Each console is a card with its name and tab, whether it is waiting for you, stuck or finished, its phase, a stale-state warning and the first lines of its panel.
- Sections that need you come first. Arrow keys move through the cards as they are laid out, and Enter or a click opens the console.
- The shortcut setting is now `key.board`; `key.overview` is reported as an unknown key.

## Since 0.1.0-alpha.3

- Reopen your AI conversations: opt into **Layout, directories and AI sessions** and each pane brings back its Codex, Claude Code or OpenCode conversation in its saved directory. Connect agents sets it up explicitly.
- On macOS, the window title no longer shows twice and the red, yellow and green buttons line up with the header icons.
- The Git review shows the changes of the console's own worktree, and says so plainly when a folder is not a Git repository.

## Experimental installers

- Install Ivy and its MCP bridge together from a `.deb`, with an application-menu entry and icon.
- Install the same Linux executables on Arch from a native `.pkg.tar.zst` using `pacman -U`.
- On macOS 15 or later, choose the Apple Silicon or Intel `.dmg` and drag Ivy into Applications.
- Install on Windows 11 x64 with a per-user installer, Start menu shortcut and uninstaller.
- Updates and removal preserve existing preferences and saved layouts.
- Downloads for Ubuntu, Arch, macOS and Windows are unsigned and experimental.
- Windows installer lifecycle checks pass in CI; real Windows desktop trials are still pending. The Windows MCP/control listener is not available yet.

## Work across consoles

- New splits inherit the source console's local working directory.
- Choose whether startup restores the previous layout; additional windows keep independent layouts.
- Identify projects and agents from pane titles, accents and agent logos, with human-input alerts.
- Use the console context menu to copy, paste, rename or split; Linux also supports PRIMARY selection paste.

- Organize independent consoles in tabs, split them, drag them into place and resize their borders.
- Restore your tabs, console layout and information-panel widths when Ivy opens again.
- Search console history, select and copy text, follow web links and preview local Markdown files.

## Keep track of your agents

- Each console has a status panel published by its agent or another tool.
- Tab indicators and the board bring waiting, blocked and finished work together.
- Use Connect agents for a setup prompt and diagnostics for your AI client.

## Make it yours

- Choose themes, installed fonts, colors, cursor behavior and keyboard shortcuts in Preferences.
- Review Git changes beside your work and insert reviewed comments into the originating console.

## Updates and release notes

- A dot on the application menu announces a newer stable version without interrupting your work.
- Choose Check for updates to check again, or What's new to read these notes and recent releases.
- View download opens the release page in your browser. Ivy does not install updates automatically.

## Choose a download

- Ubuntu 24.04 / 26.04 x86_64: `ivy_0.1.0-alpha.6_amd64.deb`.
- Arch Linux x86_64: `ivy-terminal-0.1.0alpha6-1-x86_64.pkg.tar.zst`.
- macOS 15+ Apple Silicon: `Ivy-0.1.0-alpha.6-macos-arm64.dmg`.
- macOS 15+ Intel: `Ivy-0.1.0-alpha.6-macos-x86_64.dmg`.
- Windows 11 x64: `Ivy-0.1.0-alpha.6-windows-x86_64-setup.exe`.

Download `release-manifest.json` and `SHA256SUMS` with the chosen package and verify its SHA-256
before installing. See the [installation instructions](https://github.com/mashware/ivy/blob/v0.1.0-alpha.6/docs/10-installation.md)
for verification, installation, upgrade and removal commands. No Rust toolchain is needed.
