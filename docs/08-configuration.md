# The settings file

Where it goes: **`$XDG_CONFIG_HOME/agent-terminal/config`** — or `~/.config/agent-terminal/config` if
you do not have that variable set. Next to `layout.json`, which is where the terminal saves only how
you left the windows; this one can be written by hand or through Settings.

**If it does not exist, the terminal opens exactly as always.** You do not have to create it.

## Pane names and agent session titles

A console without a manual name follows the **terminal title published by its program**. Its
header and Overview use the same name. A shell or another program can publish a title too; the
terminal does not infer which agent or session is running. Client prefixes and shortened names
are kept as published.

Double-click the console's name, or open its context menu and choose **Rename**, to set a manual
name. A nonblank manual name takes priority over later program titles and survives reopening the
application. Choose **Use terminal title**, or save an empty name, to resume following the latest
title. Cancel preserves the previous choice. Tab names stay manual.

Automatic titles are not saved: reopening the terminal starts fresh programs and waits for them
to publish their own titles. Before any title arrives, the name is `Console N`. An empty or invalid
latest title returns to that fallback. Titles are trimmed and limited to 128 Unicode scalars;
control characters are refused. An observed title is text only, never a command or an identity.

### Client setup

The client must publish its session name in its terminal title. Agent-terminal does not change
client settings or read private session files to obtain a name.

- **Codex:** run `/title` and include the **thread** item. To keep only that item, the documented
  configuration is `[tui]` with `terminal_title = ["thread"]` in Codex's own `config.toml`.
  The documented default includes the spinner and project instead, so it need not show the session
  name. Use `/rename` to name the chat. See [Codex commands](https://learn.chatgpt.com/docs/developer-commands)
  and the [configuration sample](https://learn.chatgpt.com/docs/config-file/config-sample).
- **Claude Code:** the documented `terminalTitleFromRename` default is `true`, so `/rename` or
  `--name` can supply the terminal title. With that setting disabled, the title remains generated;
  with `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1`, title updates are disabled entirely. See
  [Claude Code settings](https://code.claude.com/docs/en/settings-reference#terminaltitlefromrename).
- **OpenCode:** enable terminal-title updates in the client. Current CLI documentation calls the
  setting `terminal.title`; check the documentation matching your installed version. The client
  can publish a prefix such as `OC |` and shorten long titles, which agent-terminal keeps intact.
  See [OpenCode terminal settings](https://opencode.ai/docs/cli/config#terminal) and its
  [title publication code](https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/app.tsx).

These sources describe client capabilities, not proof that every installed version has been
tested. Launch, rename, resume and switch behavior must be checked with the client version in use.
If a client does not publish a usable title, use a manual pane name.

### Reset and lifecycle limits

The last published title remains until another title or an explicit title-reset event replaces
it, or the console is recreated. A silent client exit does not identify a session ending. The
pinned terminal engine also clears its internal title on RIS (full terminal reset) without
publishing a title-reset event, so that reset alone does not clear the observed name. A shell
that publishes its own title can replace the agent's title on returning to its prompt.

## Connect agents

The **Connect agents** icon in the global title bar opens a setup guide. It is highlighted when no
console has a published panel. Once any console publishes a valid panel through MCP or `panel.json`,
the icon returns to its normal color. The same guide is available from the application menu.

Choose **Copy setup prompt** and paste it into the AI client running inside Ivy. The prompt asks
that client to check its inherited console identity, find the bridge, preserve its existing MCP
configuration, register the local stdio server if needed, and publish a test panel. It includes the
expected bridge path as JSON-encoded data. If the client needs a reconnect or restart to load new
tools, it explains that remaining step instead of claiming success.

**Manual setup and diagnostics** expands the published-panel and live-publisher counts, expected
bridge path, last file check and manual instructions. **Check bridge again** refreshes the check;
a missing sibling file does not mean the bridge is not installed somewhere else. Ivy only copies
text: it does not run these commands or edit the client's settings.

Tool discovery and publication are distinct: a client can know the tools without publishing a panel.
An empty console or ordinary shell is not an installation error. Close the guide with **×** at the top right or
Escape. Its clicks and keys, including those used to close it, do not reach the console. Opening the
guide changes no running programs or terminal sizes.

## Graphical settings

The global application menu in the integrated titlebar opens **Preferences**, including with a single tab.
The dialog groups Appearance, Cursor and Shortcuts,
with sidebar navigation on wide windows and horizontal navigation on compact ones.

Appearance groups **Theme & colors** together: the theme selector and a compact console-and-panel
preview sit side by side when there is room. The selector shows how many colors are customized,
because explicit colors remain applied above the selected theme. Open **Customize colors** to edit
or reset the six surface colors and expandable sixteen-color ANSI palette. Invalid color values
remain identified even when this group is folded. **Typography** follows with the lazily loaded
installed-font selector, size and text sample. The sample uses the selected font and point size
without changing the running consoles. Within the font list, Up/Down selects and scrolls to a family;
Enter dismisses the list without saving. Cursor and Shortcuts retain their existing controls.

Open the theme selector to search the offline catalog or filter it with **All**, **Dark** or
**Light**. Choose with the mouse or arrow keys; browsing updates only the draft and preview.
Escape dismisses an open theme, font or color popup before closing Settings.

The footer stays separate from scrolling content, with **Save and close** as its primary action,
**Save changes** as its secondary action, and Cancel and Restore defaults alongside them. Notices
and validation errors occupy a bounded area so they do not push saving outside the window.

**Saved changes apply without restarting agent-terminal or its programs.** Opening Settings, editing
and Cancel change neither the running terminal nor the file. Restore defaults stages removal of
the supported overrides; only Save writes and applies them. Blank fields inherit the built-in default; `none` unbinds an action. Explicit
colours still override the selected theme. The cursor override retains its focus rules and its
fifteen-second inactivity stop.

**Save changes** writes and applies the draft while keeping Settings open. **Save and close** uses
the same save operation and dismisses Settings after success. An invalid value, unavailable font,
external-file conflict or write failure keeps the dialog and draft open with an explanation.

New or blank files saved through Settings declare `version=1`. Existing version lines are retained.
Save explains invalid values and conflicting bindings before writing. Comments, unrelated entries,
unchanged supported entries, a byte-order mark and retained line endings survive. Changed keys are
rewritten consistently, including equivalent spellings; restoring a key removes its earlier
assignments as well. Existing line warnings are shown without making unrelated settings unusable.

If the file changed outside Settings, Save refuses and retains the draft: Cancel and reopen to load
the new file. A write or replacement failure leaves the old configuration in place. Comparison
cannot lock out an arbitrary editor writing between the final check and replacement; this is not a
cross-editor transaction. Symbolic links and unreadable, non-text, oversized or unsupported-version
files cannot be saved graphically; they need attention outside the screen.

The installed-font list is loaded only on explicit request. Merely opening Settings neither searches
fonts nor applies a font. Save prepares a newly selected font before writing; a missing or unreadable
new family leaves the file, draft and active settings intact. An unchanged family does not trigger
another lookup, including a startup request that already fell back to the built-in fonts.

Consoles, including those in background tabs, keep their programs, histories, identities and panels.
Font and size changes update their grids through the ordinary resize path. Removing an override
restores its factory behaviour; the old active value is not retained. Manual changes to the file
are still read on the next startup: this is not an external-file watcher.

Historical measurement batches retain their previous geometry. Explicit settings variants declare
the compact tab-strip regime and keep configuration I/O vetoed; reports from different chrome
regimes cannot be averaged. The config-free `settings-apply` variant uses four consoles to witness
colour, size, selected-family changes, unchanged-family reuse, reset and a failed preparation. Set
`MEASURE_SETTINGS_FAMILY` explicitly to an installed family for that probe; it performs no Save and
its costs are not measurements of configuration persistence.

## How it is written

```
# A line starting with a hash is a comment.
# Empty lines are nothing.

color.1 = #cf6a4c
background = #1d1f21
focus      = #6c9ef8
```

A line is `key = value`. The spaces around it do not matter.

**A comment takes the whole line and cannot go at the end of a value.** The hash is both the comment
character and the start of a colour, so a comment at the end would cut `color.1 = #cf6a4c` in half.
It is the same thing Kitty and Ghostty do.

## What you can set today

| Key | What it is |
|---|---|
| `color.0` … `color.15` | the sixteen colours a program can ask for. 0 to 7 the normal ones, 8 to 15 the bright ones |
| `background` | the console's background |
| `foreground` | the colour it writes with when the program does not ask for another |
| `cursor` | the cursor's **colour** — both the filled one of the console being typed into and the outline every other console draws |
| `divider` | the line that separates two consoles |
| `focus` | what marks out the console that has the keyboard |
| `panel` | the status panel's background |
| `theme` | a built-in set of all twenty-two colours; see the catalog below |
| `size` | the font size, in points |
| `font` | the font family it paints with, by its name |
| `font.ligatures` | programming ligatures from the selected compatible font: `on` or `off`; omitted means `off` |
| `cursor.blink` | whether the cursor blinks: `on` always, `off` never. Leave the line out and the program inside decides, over a default of «yes» |

Colours are written **`#rrggbb`**, in hexadecimal, and it makes no difference whether in upper or
lower case.

**The size goes from 4 to 96 points**, with decimals if you want. The usual one is **14**. Outside
that range you are told and the usual one is used: at two points nothing is legible and at two
hundred three columns fit.

Changing the size **changes how many rows and columns** fit in your window, so the programs running
inside notice.

### The blink, if you want it settled

```
cursor.blink = off
```

**The cursor blinks on its own**, in the console you are typing into and while the window has your
desktop's focus, and it **stops on its own** fifteen seconds after your last keystroke, staying
visible. Every other console keeps the still outline it always draws.

**The program inside can change that**, and most of them do: an editor that wants a still cursor asks
for one and gets it. That is the normal arrangement and you do not have to write anything for it.

This line is for settling the argument. **`off` never blinks and `on` always blinks**, whatever the
program asks for — leave the line out and the program decides. It is not a default, it is a ceiling:
a program cannot climb over it in either direction.

**One thing worth knowing before you need it**: what a program asks for **outlives it**. Leave a
full-screen editor that wanted a still cursor and the shell you come back to has a still cursor too,
until something asks otherwise — `reset` puts it back, and so does anything that restores the
terminal's defaults. That is what every other terminal does too, and `cursor.blink = on` is the line
that makes the question go away for good.

### Programming ligatures

```ini
font = JetBrains Mono
font.ligatures = on
```

Programming ligatures use a compatible selected monospaced font to join sequences such as `!=`,
`=>` and `===`. They start **off**. Set `font.ligatures = off` or remove the line to restore
independent characters. Missing or incompatible fonts keep normal drawing. Enabling this option
does not search for, install or select a font.

Preferences exposes **Programming ligatures** under Typography. Save applies and persists the
option without restarting programs; Cancel discards the draft, and reset restores off. Copy and
search retain the original characters, and terminal columns stay fixed. Selection, search and
cursor boundaries may separate a ligature into its characters.

### The font, by its name

```
font = JetBrains Mono
```

Write it **as the font calls itself** — `JetBrains Mono`, not `JetBrainsMono` — with its spaces.
Upper and lower case make no difference.

**What that font does not carry, the built-in ones still paint.** They stay behind it as a net, so a
kanji or an emoji your font does not have keeps showing.

**The icons of a `Nerd Font` are the exception**: there the font you installed for them keeps
winning, not the one you set here. That is where the drawings live and only whoever installed one
knows which is the right one.

If the family is not installed you are told and the terminal opens with the built-in ones. If it is
installed and its files cannot be read — a half-finished package update leaves them like that — you
are told **that** instead, so you do not go looking for a typo that is not there.

**It has to be a fixed-width font.** In a terminal every cell is the same width, taken from the `M`,
so a font whose letters measure different amounts leaves the columns out of line and the wide
letters spilling into the cell next door. You are told when that happens and the font is used
anyway: it is your file.

> **This is the only setting that costs anything on start-up.** Finding a font by its name means
> going through every font on your machine, which takes a few hundredths of a second. **If you do not
> write this line, nothing is searched** and the terminal opens exactly as fast as it does today.

**What you do not set stays as it was.** You can change a single colour and not write the rest.

### Choose a colour theme

```
theme = tokyonight
color.4 = #010203
```

The bundled catalog contains the following themes:

| Configuration name | Theme | Appearance |
|---|---|---|
| `tokyonight` | Tokyo Night (Night) | Dark |
| `gruvbox-dark` | Gruvbox Dark (medium contrast) | Dark |
| `catppuccin-mocha` | Catppuccin Mocha | Dark |
| `catppuccin-latte` | Catppuccin Latte | Light |
| `nord` | Nord | Dark |
| `github-dark-default` | GitHub Dark Default | Dark |
| `github-light-default` | GitHub Light Default | Light |
| `rose-pine` | Rose Pine | Dark |
| `rose-pine-dawn` | Rose Pine Dawn | Light |
| `rose-pine-moon` | Rose Pine Moon | Dark |

The factory/inherit choice omits the theme line. Names
ignore letter case. Each theme supplies all twenty-two colours. A colour you write yourself replaces
that one colour in the theme, whether its line comes before or after `theme`.

If you write `theme` more than once, the last valid name wins and the terminal reports the repeated
key. An unknown name is reported with its line number and the available names; other valid lines
still apply. With no `theme` line, the current built-in appearance remains the base.

The catalog is part of the binary and works offline. External terminal palettes are converted during
development from a pinned source revision, with attribution and original license notices in the
repository. The terminal reads no theme files or online catalogs. External changes to the config
file are read at startup; saving through Settings applies the draft immediately.

The preview includes explicit color overrides, just as the running terminal will. **Use theme
colours** clears those staged overrides when you want the complete selected palette. Added external
palettes supply the ANSI colors, background, foreground and cursor; divider, focus and panel colors
are completed locally as recorded in D65.

## The shortcuts

The terminal's thirteen shortcuts are changed with `key.` and the name of what they do:

```
key.split-right = ctrl+shift+e
key.close-pane = none
```

| Name | What it does | Out of the box |
|---|---|---|
| `split-right` | splits the console in two, across | `ctrl+shift+d` |
| `split-down` | splits it down | `ctrl+shift+b` |
| `close-pane` | closes the focused console | `ctrl+shift+x` |
| `new-tab` | opens a tab | `ctrl+shift+t` |
| `close-tab` | closes the tab | `ctrl+shift+w` |
| `previous-tab` · `next-tab` | moves between tabs | `ctrl+shift+←` · `ctrl+shift+→` |
| `scroll-page-up` · `scroll-page-down` | goes up and down the scrollback | `shift+pageup` · `shift+pagedown` |
| `scroll-to-top` · `scroll-to-bottom` | goes to the ends of the scrollback | `shift+home` · `shift+end` |
| `search` | searches the focused console's scrollback; pressed again, closes the search | `ctrl+shift+f` |
| `overview` | opens or closes the global task summary | `ctrl+shift+o` |

**A chord is written with `+`**, in any order and regardless of case: `ctrl+shift+e`, `shift+prior`,
`alt+f4`. `ctrl`, `shift` and `alt` are valid, and **at least one is required**: without one you
would be taking a letter away from yourself.

Keys are written the way the keyboard calls them: `e`, `f4`, `space`, `delete`, `home`, `end`,
`pageup`, `pagedown`, `left`, `right`, `up`, `down`. The usual abbreviations work too (`prior`,
`next`, `pgup`, `pgdn`, `esc`), and the arrows with their symbol (`←`, `↑`, `→`, `↓`).

**`Ctrl+C`, `Ctrl+V` and `Ctrl+X` cannot be bound to anything.** The desktop takes them before they
get here, so a shortcut with those chords would never fire and you would have no way of knowing why.
`Ctrl+X` is refused only without Shift: `Ctrl+Shift+X` does arrive, and it is the close shortcut.

### `none` gives you the chord back

```
key.close-pane = none
```

From then on that chord **reaches the program running inside**. It is what lets you get
`Ctrl+Shift+X` back for `vim` or for `tmux`.

### If two shortcuts tread on each other

**The last one you write wins**, and you are told which one is left with no chord:

```
agent-terminal: 'close-pane' is left without a shortcut: its chord goes to 'split-right', which comes later
```

### Copy and paste are not rebound: copy is only switched off

`Ctrl+Shift+C` and `Ctrl+Shift+V` **do not arrive as keys**: the desktop resolves them first and they
arrive already turned into a copy and a paste, **without even saying whether Shift was held**. There
is no chord to move, and that is why they are not in the table.

What you can do is **switch copy off**, which is what gives `Ctrl+Shift+C` back to `vim` or to
`tmux`:

```
key.copy = none
```

Any other value is rejected and you are told why — giving it another chord would be accepted and
would never work:

```
agent-terminal: line 3: the chord for 'key.copy' is the desktop's and cannot be changed; the only thing you can do is turn it off with 'none'
```

## What happens if you get it wrong

**One badly written line does not take the file down.** It is ignored, you are told which one it is
and what is wrong with it on the error output, and **the rest is applied**:

```
agent-terminal: line 7: 'purple' is not valid for 'color.3'
```

Up to twenty lines with problems are reported; if there are more, you are told how many were left
unlisted.

**A repeated key wins with the last one**, and you are told that too — so pasting one theme over
another does what you expect, and you do not spend half an hour changing a line that another one
further down is treading on.

**And if you forget the value** — `background =` — you are told exactly that, not that the equals
sign is missing: the equals sign is there, what is missing is the colour.

What does leave the terminal entirely as it comes out of the box, saying why:

- that it cannot be read, or that something with that name is not a file;
- that it is not text;
- that it is over 64 KiB;
- that it declares a version newer than the one this terminal understands.

That it **does not exist** is the only case that is not reported: that is the normal one.

## `version`

It is **optional**. If you set it, it is good for one thing: so that a terminal older than the format
**does not apply** a file it does not understand, instead of applying half of it.

```
version = 1
```

You do not have to write it. The format is flat, so a key a terminal does not know is simply ignored
and it tells you so.

If you write something that is not a number, you are warned about that line like any other and the
file goes on being applied.

## What still cannot be configured

**How fast the cursor blinks** — it is 600 milliseconds lit and 600 dark, in step with what this
desktop uses for every other window, and nobody has asked for another number. **What shape the cursor
is** either: that belongs to the program inside, which is the arrangement every terminal has.
Reloading the file without closing the terminal is left for later, and the reason is written down.

## Integrated window controls

The integrated titlebar and console headers use the active theme and explicit colour overrides.
Their surfaces derive from `background` and `panel`, their text from `foreground`, their separators
from `divider`, and active/hover marks from `focus`. Changing the named theme or saving colour
overrides updates these controls along with the consoles. No independent fixed chrome palette is
kept. Existing terminal foreground/background and ANSI colour behaviour is unchanged.

New tabs open one console. The application menu groups tab creation, splitting, the focused
console's information-panel toggle, Preferences and Keyboard shortcuts. Shortcut hints reflect
the current bindings and disappear when a chord is disabled. Interface labels keep their own
compact proportional type independently of the terminal font size.

Each console header includes its information-panel toggle, maximize/restore, close controls.
Double-click its label, or right-click its header and choose Rename. Double-click or right-click a tab label
to rename that tab. Both labels are optional inert layout metadata saved with the layout. Maximizing a
console temporarily hides its siblings; restoring reveals their original order and proportions.
Hidden programs keep running. Right-clicking a header retains its console action menu without an overflow button.


## Global task Overview

The global **Overview** control shows every console in every tab, including a console hidden by
maximization or a folded information panel. The default shortcut is **Ctrl+Shift+O**:

```text
key.overview = ctrl+shift+o
```

Like other bindings, it can be changed or disabled with `key.overview = none`, which returns the
chord to the program when Overview is closed. The Shortcuts section in Settings edits the same key.

Entries put producer-declared waiting work before blocked work, then finished work, then consoles
with no declared claim. Each entry shows its console and tab names, literal phase, up to three
nonempty lines of the panel and any warning about old or unreadable state. A missing claim means
**No attention requested**, not proof that the program is working; a console with no document says
**No published state**. When no console has a document, the screen says **No task state has been
published yet** and still lists all consoles.

Click an entry, or use Up/Down/Home/End and Enter, to open its console. A hidden target restores its
tab's arrangement before taking focus. Escape, Close or the Overview control returns to the console.
Opening and closing the screen preserves console dimensions, programs, history and panel ownership.
All programs continue running. Producer actions and links are inert here; return to the console to
use them. Settings and Overview open separately so a settings draft is not discarded.

## Markdown and Git diff viewers

Ctrl-click a local `.md` path printed by a console to open Markdown inside the terminal.
OSC 8 `file://` links use the same viewer. Relative paths resolve against the directory explicitly
published by that console using OSC 7; without one, the viewer asks for an absolute path.
Only empty authorities, localhost and the verified local hostname are accepted as local.
For a shell without OSC 7 integration, print the directory before printing a relative path:

```sh
printf '\033]7;file://%s%s\007' "$(hostname)" "$PWD"
```

Each console header has a **Review changes** control beside its information-panel and maximize
controls. Its context menu offers the same action when the header is narrow. It opens the clicked
console's repository directly, including when another console has keyboard focus. No repository
path form is shown. On Linux, when the console has not published OSC 7 metadata, the current
foreground program's working directory is read from the operating system. An explicitly invalid or
remote OSC 7 directory does not fall back to an unrelated local directory. On other platforms the
console must publish a usable local OSC 7 directory. If its directory cannot be read, the review
shows that fact instead of choosing another console's repository.

The global application menu retains **Markdown viewer** and **Git diff viewer**; the latter uses
the focused console. Markdown can also be opened using its file input.
The viewer floats above all console partitions, anchored to the right. Drag its left edge to resize,
use **Expand**/**Restore**, or dismiss it with **Close** or Escape while it owns keyboard focus.
Opening and resizing it preserves every console's grid. Clicking a console returns typing there.

Markdown displays headings, lists, tables, code and inline formatting. It refreshes every second
while open, including atomic file replacement; background refresh keeps the document in place.
Links, image descriptions and HTML remain inert. Documents must be UTF-8 regular files of at most
1 MiB; a single rendered paragraph, code line or table row must fit within 16 KiB. Failed reloads
show their error and preserve the last good document.

Git offers **Local** (tracked worktree against HEAD plus untracked regular files), **Unstaged**
(worktree against index), **Staged** (index against HEAD), and **Against base** (worktree against a
verified commit reference). The latter three cover tracked files. Choose files, unified or side-by-side
view, and previous/next hunk. Line numbers identify the old and new sides. Binary and skipped new files
have notices. Choose **Refresh** after further edits; it is disabled while review comments are pending.
The review is read-only: no staging, discarding or committing.

Use a line's **+** to add a comment. **Preview comments** shows the complete message, including
repository, comparison, source console and file/side/line references. **Insert into console** pastes
that message into the pinned originating console without adding Enter; review and submit it there.
The action refuses a closed origin or a console without multiline paste support and keeps the draft.
Clear or send comments before changing repository, comparison or origin.

Git output and included new-file contents share a 4 MiB limit. Each Git command has a five-second
timeout; errors appear in the viewer. Paths, modes, comments and width are transient and have no
configuration keys.


## Updates and release notes

The global application menu offers **Check for updates** and **What's new**, including with one tab
or a narrow window. Ivy checks public stable releases once per normal launch in the background.
A dot on the menu indicates a newer stable version. Opening the dialog and dismissing the dot do
not stop, resize or type into any console. **Dismiss notification** applies until the next launch.

The dialog shows the installed version and its bundled notes without a connection. Its version
selector also lists up to twenty recent published stable releases after a successful check.
**View download** and **Open release page** open the selected GitHub release in your browser after a
click. Ivy never downloads or installs an executable automatically.

No published releases, an up-to-date installation and a failed check are separate states. Retry a
failed check with **Check for updates**; repeated clicks cannot start concurrent checks. Remote
history is not cached between launches. No console content or credentials are included in requests.
The feature adds no configuration keys. See [Release publication](09-releases.md) for maintainers.
