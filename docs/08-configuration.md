# The settings file

Where it goes: **`$XDG_CONFIG_HOME/agent-terminal/config`** — or `~/.config/agent-terminal/config` if
you do not have that variable set. Next to `layout.json`, which is where the terminal saves only how
you left the windows; this one you write yourself.

**If it does not exist, the terminal opens exactly as always.** You do not have to create it.

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
| `theme` | a built-in set of all twenty-two colours: `tokyonight` or `gruvbox-dark` |
| `size` | the font size, in points |
| `font` | the font family it paints with, by its name |
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

`tokyonight` uses the Night variant; `gruvbox-dark` uses the dark, medium-contrast palette. Names
ignore letter case. Each theme supplies all twenty-two colours. A colour you write yourself replaces
that one colour in the theme, whether its line comes before or after `theme`.

If you write `theme` more than once, the last valid name wins and the terminal reports the repeated
key. An unknown name is reported with its line number and the two available names; other valid lines
still apply. With no `theme` line, the current built-in appearance remains the base.

The catalog is part of the binary. It reads no theme files and does not reload a changed config while
the terminal is open.

## The shortcuts

The terminal's twelve shortcuts are changed with `key.` and the name of what they do:

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
