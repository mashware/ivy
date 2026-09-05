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
| `cursor` | the cursor |
| `divider` | the line that separates two consoles |
| `focus` | what marks out the console that has the keyboard |
| `panel` | the status panel's background |
| `size` | the font size, in points |

Colours are written **`#rrggbb`**, in hexadecimal, and it makes no difference whether in upper or
lower case.

**The size goes from 4 to 96 points**, with decimals if you want. The usual one is **14**. Outside
that range you are told and the usual one is used: at two points nothing is legible and at two
hundred three columns fit.

Changing the size **changes how many rows and columns** fit in your window, so the programs running
inside notice.

**What you do not set stays as it was.** You can change a single colour and not write the rest.

## The shortcuts

The terminal's eleven shortcuts are changed with `key.` and the name of what they do:

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

**A chord is written with `+`**, in any order and regardless of case: `ctrl+shift+e`, `shift+prior`,
`alt+f4`. `ctrl`, `shift` and `alt` are valid, and **at least one is required**: without one you
would be taking a letter away from yourself.

Keys are written the way the keyboard calls them: `e`, `f4`, `space`, `delete`, `home`, `end`,
`pageup`, `pagedown`, `left`, `right`, `up`, `down`. The usual abbreviations work too (`prior`,
`next`, `pgup`, `pgdn`, `esc`), and the arrows with their symbol (`←`, `↑`, `→`, `↓`).

**`Ctrl+C` and `Ctrl+V` cannot be bound to anything.** The desktop takes them before they get here,
so a shortcut with those chords would never fire and you would have no way of knowing why.

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

The font comes in the next delivery of this same milestone. Reloading the file without closing the
terminal is left for later, and the reason is written down.
