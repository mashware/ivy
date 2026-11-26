# Ivy 0.1.0-alpha.22

## New in this alpha

- Select text beyond the visible part of a code document: holding the pointer above or below the view scrolls it without losing the selection's starting point. Scrollbars remain visible, and the Open menu has clearer spacing and a visible Wrap lines checkbox.
- The Board gives project sections more room, keeps controls aligned and reveals tasks on demand.
- Codex stays connected when opening its plugins screen.
- On Windows, PowerShell reports its current directory to Ivy for project identity, new splits, Review and relative links. Its native Codex entry preserves arguments and exit status.
- On Windows, OpenCode can publish working, permission and question indicators through Ivy's bundled helper. Installed-client desktop verification and session recovery remain pending.
- Windows Codex route records use private storage. Shared mobile/panel conversations remain unavailable there; ordinary launches explain the limitation and preserve Codex's original behavior.
- Shell startup failures on Windows remain visible instead of closing Ivy silently.

Desktop appearance, operating-system clipboard copying and the reported code-toolbar alignment still need manual verification. Downloads remain unsigned and experimental.

## Previously in 0.1.0-alpha.21

- On Windows, launching Ivy and refreshing project or Git views no longer opens extra console windows. Ivy carries its own application icon and version.
- Windows installers include the session helpers and recovery adapters needed by agent integration.
- Claude title symbols remain legible when the selected font does not include them.

## Previously in 0.1.0-alpha.19

- See a large project roadmap in one compact milestone strip, with readable columns for work in progress, ready work and milestones you open yourself.
- Keep console cards beneath their explicitly linked roadmap tasks, with compact published summaries and clear waiting states.
- Expand milestones, show the whole roadmap and filter console cards without losing the project overview.
- Board columns and cards have aligned headers, consistent spacing and word-wrapped text.
- Agents can open images directly in their own console through Ivy's MCP bridge, including remote images.
- On Windows, Ctrl+Alt types characters from your keyboard layout once, including @ on Spanish keyboards, while ordinary Ctrl+Alt shortcuts still work.

## Previously in 0.1.0-alpha.16

- Saved layouts restore even when an obsolete session-ownership file was left by another account.
- Configured project roadmaps can be JSON documents as well as Markdown files.
- Codex responses stay connected while an active MCP request is still streaming.
- Console headers keep their title when an agent does not publish activity signals.

## Previously in 0.1.0-alpha.15

- Screenshots printed in a console open in the drawer, with zoom and pan. Each console keeps its own screenshot in an Image tab, and you can paste one from the clipboard into it.
- Annotate a screenshot in the drawer and send it to its console. The annotation drawer stays usable when short, undoes one note at a time and offers a way out of a stuck send.
- Open any file from Files as read-only, coloured code. Its header shows where the file lives and opens it safely.
- Search the console and Files with regular expressions.
- Each project gets a clearly different colour, and the focused console is outlined in green.
- Console header buttons linger, then fade out instead of vanishing.
- One click focuses a console after using the drawer, and a narrow drawer hides inactive tabs before the active one.
- Ivy keeps a log of its own failures and crashes, which you can find from Settings.
- A panel published over MCP no longer warns that its file is gone.
- On macOS, Codex keeps Ivy's panel after Ivy was launched from a temporary copy.

### Windows

- Ivy no longer closes on launch, and graphics startup failures are explained.
- Consoles run on Microsoft's own console host, shipped with Ivy. They end with their program, close without freezing, and say when their output was lost.
- Agents can publish status panels and use Ivy's MCP tools through a private, per-user channel.
- Clipboard shortcuts behave like a terminal's, while text fields keep their own.
- Emoji are drawn in colour.
- Review and links find a console's folder, and paths are handled in Windows' own form.
- The diagnostics log stays bounded even after a failed rotation.
- Real Windows desktop trials are still pending for this alpha.

## Previously in 0.1.0-alpha.14

- On macOS, closing a console no longer freezes Ivy when its program cannot be stopped right away.
- On macOS, Codex publishes its status panel to its own console instead of another one.
- On macOS, typing at the prompt no longer leaves doubled text that cannot be erased: consoles get a UTF-8 locale when the desktop provides none. Native Mac validation is pending in this alpha.
- Console search keeps arrow keys, Home, End and deletion in its own input, editing at a visible blinking caret instead of sending them to the running program.
- The search options list fits its longest label instead of clipping it.

## Previously in 0.1.0-alpha.13

- Claude Code hooks can clear an answered question's waiting claim when Ivy was launched from the desktop without Claude's native installation on PATH. Hooks still have to belong to the same running client.

## Previously in 0.1.0-alpha.12

- Linux consoles start in separate systemd user scopes when available, improving isolation when memory pressure selects one console. Confirmed memory-pressure exits keep their output visible with an explanation; startup still works without a user manager.
- Search terminal history from each console's header, with match-case, whole-word and wrap options in its own search box.
- Tabs read as separate cards, the active one carries an accent line, and each console's attention state appears as a ring around its tab dot.
- Interface controls use one consistent icon set, and Files shows icons for each file type.
- On macOS, Files discovers the console's current folder without shell setup, restoring file browsing, project colours and folder-aware splits. Native desktop validation remains pending.
- Rename, Connect agents and What's new use consistent themed dialog headers.
- Deferred session-recovery setup is hidden from Connect agents until it is ready.
- Codex upgrades preserve live mobile access when client and server capabilities remain compatible, instead of requiring exact version equality.

## Previously in 0.1.0-alpha.11

- Console text keeps its original edge coverage, with automatic fitting for fonts that have no native glyph instructions. Fonts with their own instructions keep their existing fitting.
- Font sizes in Settings and new version 2 configuration files use true typographic points for console text, pane titles and tab labels. Existing configurations retain their visual sizes; edited saves upgrade to version 2, which older Ivy versions do not read.
- Box-drawing lines render continuously instead of appearing dotted or blurred.
- Split a console from its own header. Controls are grouped and appear on the focused console or when you hover another one.
- Ivy Night's bright-red palette entry uses the reference orange for ANSI-coloured agent mascots.
- Connect agents separates one-time setup from reconnecting an existing Claude session. Empty panels no longer imply setup is missing, and startup publication guidance survives a control channel that becomes ready later.

## Previously in 0.1.0-alpha.10

- Ivy has a new look, Ivy Night (and Ivy Day for light desktops), with the Geist fonts. Settings, menus and dialogs follow the theme you choose, whatever the desktop prefers.
- The title bar holds the tabs, with each tab's claim shown as a small pill. Files, the Board, Markdown, split, search, Connect agents and the menu sit in the bar, with labelled buttons and the same icons everywhere.
- Files opens on the left of your consoles and follows the one you are working in. Its search lists matching names in a floating list: Enter shows the match in the tree, Ctrl+Enter opens a Markdown file.
- Git review and Markdown share a drawer below the consoles, as tabs you can close. Resize the drawer, maximize it over the consoles without resizing them, compare changes unified or side by side, and drag the divider between the two sides.
- Console headers name the project in its colour. A plain shell shows its folder and program instead of the long terminal title. The focused console is outlined in its project's colour.
- The status panel shows a waiting, stuck or finished claim as a band across its top, and its actions look like buttons, with the command visible before you press it.
- The Board can lay each project's roadmap out by milestone as well as by state, and remembers your choice.
- Hover and selection stay visible even when custom colours collide.

## Previously in 0.1.0-alpha.9

- Files, Markdown, Git review and Board now share a right sidebar, keeping running consoles visible beside the tools you use.
- Browse files independently in each console, with navigation and file previews tied to that console.
- The Board presents complete agent panels and foldable project roadmaps, with clearer states, branch information and console navigation. Menus and tabs remain clickable while it is open.
- Tabs make agent status easier to recognize, interactive controls show pointer feedback, and single-pane tabs no longer offer an unnecessary maximize control.
- Preview the selected theme throughout Preferences before applying it.
- Codex conversations support mobile control while each console keeps its own status-panel route.
- Selected terminal backgrounds no longer leave gaps, and unsafe inherited panel directories are rejected.

## Since 0.1.0-alpha.8

- The board now shows persistent project roadmaps, published by agents or read from a configured Markdown file, with console cards grouped under milestones and deliverables.
- Open and fold individual panel block headers to keep the information you need in view.
- Agents started in an Ivy console are prompted to publish their status panel.
- Hooks from the active agent can refresh progress and clear an obsolete waiting claim while the agent keeps its panel connection.
- Completed agents release their panel so file publishers can take over, and publications from processes outside the console are labelled.
- Selecting text no longer shifts shaped terminal text at selection boundaries.

## Since 0.1.0-alpha.6

- Each console header now carries a subtle project-color gradient, making consoles in the same project easy to recognize without tinting terminal output. Text, controls and agent indicators remain readable in light and dark themes.

## Since 0.1.0-alpha.5

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
- Windows installer lifecycle checks pass in CI; real Windows desktop trials are still pending.

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

- Ubuntu 24.04 / 26.04 x86_64: `ivy_0.1.0-alpha.22_amd64.deb`.
- Arch Linux x86_64: `ivy-terminal-0.1.0alpha22-1-x86_64.pkg.tar.zst`.
- macOS 15+ Apple Silicon: `Ivy-0.1.0-alpha.22-macos-arm64.dmg`.
- macOS 15+ Intel: `Ivy-0.1.0-alpha.22-macos-x86_64.dmg`.
- Windows 11 x64: `Ivy-0.1.0-alpha.22-windows-x86_64-setup.exe`.

Download `release-manifest.json` and `SHA256SUMS` with the chosen package and verify its SHA-256
before installing. See the [installation instructions](https://github.com/mashware/ivy/blob/v0.1.0-alpha.22/docs/10-installation.md)
for verification, installation, upgrade and removal commands. No Rust toolchain is needed.
