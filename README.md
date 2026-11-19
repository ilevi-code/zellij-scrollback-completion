# zellij-scrollback-completion

A [Zellij](https://zellij.dev) plugin that completes the word you're typing
from the **scrollback of the current pane**, with a small vim-style completion
menu.

Type part of a word that appeared earlier in the pane (a path, a branch name,
a hash, a hostname), press `Alt ;`, pick a candidate, and press `Enter`. The
word is completed in place.

Requires Zellij **0.46** or later. See [ROADMAP.md](ROADMAP.md) for the design.

## Install

1. Add the wasm target (once):

   ```sh
   rustup target add wasm32-wasip1
   ```

2. Build the plugin:

   ```sh
   make build
   # same as: cargo build --release --target wasm32-wasip1
   ```

   Output: `target/wasm32-wasip1/release/zellij-scrollback-completion.wasm`

3. Add a keybind. Put the `bind` block inside the existing
   `shared_except "locked" { ... }` block of `keybinds` in your `config.kdl`,
   replacing `/path/to/zellij-scrollback-completion` with the absolute path of
   this repository:

   ```kdl
   keybinds {
       shared_except "locked" {
           bind "Alt ;" {
               MessagePlugin "file:/path/to/zellij-scrollback-completion/target/wasm32-wasip1/release/zellij-scrollback-completion.wasm" {
                   floating true
                   // Optional settings; the values shown are the defaults.
                   // separators "()[]{}<>\"',;"
                   // trim_trailing ".:"
                   // min_length "3"
                   // max_lines "unlimited"
                   // height "10"
                   // rejoin_wrapped "true"
               }
           }
       }
   }
   ```

4. Restart Zellij, or start a new session, so it picks up the keybind.

5. On the first press of `Alt ;`, Zellij opens the plugin and asks for
   permissions. Focus the plugin pane (for example by clicking it) and press
   `y`. The plugin then hides itself; press `Alt ;` again to use it. Zellij
   remembers the grant, so it asks only once.

After rebuilding the wasm, restart the Zellij session: Zellij keeps the
already-loaded plugin until then.

## Usage

Press `Alt ;` in a terminal pane. A borderless menu opens under the word
you're typing (or above it when there's no room below), listing words from
the pane's scrollback.

* **The current word is the filter.** The text before the cursor, back to the
  last whitespace or separator, pre-fills the filter, so `git chec` + `Alt ;`
  offers `checkout`. The partial word itself is never offered.
* **Keep typing.** Characters and `Backspace` go straight to your shell prompt,
  and the menu re-filters as the word changes.
* **Ranking.** Fuzzy matching (smart case) orders the candidates; equally
  good matches are ordered most recent first (closest to the bottom of the
  scrollback). Duplicates are removed.
* **Completion.** `Enter` completes the word. If the chosen candidate starts
  with what you typed, only the rest is typed; otherwise your partial word is
  erased with backspaces and replaced. The word is inserted exactly as shown,
  with no trailing space and no quoting.

| Key                                         | Action                                           |
|---------------------------------------------|--------------------------------------------------|
| characters, `Backspace`                     | typed into the shell; the menu follows the word  |
| `Tab` / `Down` / `Ctrl-n`                   | next candidate                                   |
| `Shift-Tab` / `Up` / `Ctrl-p`               | previous candidate                               |
| `Enter`                                     | complete the word with the selection and close   |
| `Esc`                                       | close; what you typed stays                      |
| separator (space, `(`, ...), `Backspace` past the start of the word, `Enter` with no matches, any other key | close the menu, then the key goes to the shell |

If nothing matches, the menu shows a dim `no matches` row and stays open so
you can keep typing or backspace. If the scrollback has no candidates at all,
it shows `no matches` and the next key closes it. If Zellij can't hand over
the scrollback, it shows a red `cannot read scrollback` row instead (the
reason is in the Zellij log), and the next key closes it.

**Limitation:** the menu holds Zellij's focus while it's open (giving focus
back to the shell pane would hide the floating menu), so the shell's cursor
isn't drawn until the menu closes.

## Options

Options go inside the `MessagePlugin { ... }` block of the keybind, one per
line, as `name "value"`. Values are always quoted strings, even numbers and
booleans. Unknown options are ignored. An invalid value falls back to the
default and a warning is written to the Zellij log.

| Option           | Default         | Accepted values |
|------------------|-----------------|-----------------|
| `separators`     | `()[]{}<>"',;`  | any characters  |
| `trim_trailing`  | `.:`            | any characters  |
| `min_length`     | `3`             | integer ≥ 0     |
| `max_lines`      | `unlimited`     | integer; `0`, `none`, `unlimited` or empty for no limit |
| `height`         | `10`            | integer ≥ 1     |
| `rejoin_wrapped` | `true`          | `true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0` |

### `separators`

Characters that split the scrollback into words, **in addition to
whitespace** (which always splits). Each character in the string is one
separator; order doesn't matter. The same set also decides where the
current word starts, and typing a separator while the menu is open closes it.

To use a `"` in the value, escape it as `\"` (KDL string syntax). Examples:

* `separators "()[]{}<>\"',;"`: the default.
* `separators "()[]{}<>\"',;=:"`: also split `key=value` and `host:port`.
* `separators ""`: split on whitespace only.

### `trim_trailing`

Characters stripped from the **end** of each word, so `see foo.txt.` yields
`foo.txt` and `error:` yields `error`. Every trailing run of these characters
is removed. Use `trim_trailing ""` to keep words as they are.

### `min_length`

Words shorter than this, counted in characters after trimming, are dropped.
`min_length "1"` keeps every word. `0` behaves like `1`.

### `max_lines`

Scan only the last N lines of the scrollback (after wrapped rows are
re-joined). The default scans the whole scrollback, which is bounded by
Zellij's `scroll_buffer_size` (10000 lines by default). Lower it if opening
the menu feels slow on very large scrollbacks.

### `height`

Maximum number of candidate rows shown at once. The menu shrinks when there
are fewer matches, and further if the screen doesn't have room. The list
scrolls to keep the selection visible.

### `rejoin_wrapped`

Zellij reports the scrollback as screen rows, so a long line (a URL, a long
path) that wraps is split across rows. With this on, a row that fills the
pane's full width is joined with the next one, so the long word is offered
whole. Turn it off if you see words glued together from lines that happened
to end exactly at the pane edge.

### Changing options

Zellij treats a different set of options as a different plugin instance.
After changing them, restart the session; the new instance may ask for
permissions again.

## Permissions

* `ReadPaneContents`: read the pane's scrollback.
* `WriteToStdin`: type into the pane (forwarded keys and the completion).
* `ReadApplicationState`: find the focused pane, its cursor and geometry.
* `ChangeApplicationState`: show, position and hide the menu, and return focus.

## Development

```sh
make test    # cargo test: unit tests for all pure logic (host target)
make check   # cargo fmt --check + cargo clippy -D warnings (host target)
make build   # release wasm
```

Layout:

* `src/config.rs`: parses the options.
* `src/scrollback.rs`: orders the rows, re-joins wrapped rows, applies `max_lines`.
* `src/candidates.rs`: splits into words, trims, applies `min_length`, orders by recency, removes duplicates.
* `src/ranking.rs`: fuzzy ranking (`nucleo-matcher`), ties broken by recency.
* `src/completion.rs`: the partial word before the cursor, and the keystrokes that replace it.
* `src/keys.rs`: key presses and their terminal byte encoding, for forwarding.
* `src/menu.rs`: the menu state machine (filter, selection, scrolling, which keys are consumed or forwarded).
* `src/placement.rs`: menu placement below or above the word, clamped to the screen.
* `src/render.rs`: ANSI rendering and the menu's size.
* `src/main.rs`: the Zellij glue (events, permissions, pane I/O). It is registered as a plugin only when built for wasm.

`tests/readme_keybind.rs` parses the first `kdl` block of this README with
Zellij's own config parser, so keep the keybind snippet first.
