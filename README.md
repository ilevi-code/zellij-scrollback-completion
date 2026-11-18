# zellij-scrollback-completion

A Zellij (0.46) plugin that pops up a vim-style completion menu of words
taken from the focused pane's scrollback. The word you pick is typed into
that pane at the cursor.

See [ROADMAP.md](ROADMAP.md) for the design.

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

3. Add the keybind. Put the `bind` line inside the existing
   `shared_except "locked" { ... }` block of `keybinds` in your `config.kdl`:

   ```kdl
   keybinds {
       shared_except "locked" {
           bind "Alt ;" {
               MessagePlugin "file:/Users/itayl/projects/zellij-scrollback-completion/target/wasm32-wasip1/release/zellij-scrollback-completion.wasm" {
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

   `floating true` matters only on the first press, when Zellij loads the
   plugin. Without it, Zellij opens the plugin as a tiled pane.

4. Restart Zellij, or start a new session, so it picks up the keybind.

5. On the first press of `Alt ;`, Zellij opens the plugin and asks for
   permissions. Focus the plugin pane, for example by clicking it, and press
   `y`. The plugin then hides itself. Press `Alt ;` again to use it. Zellij
   remembers the permissions, so it only asks once.

After rebuilding the wasm, reload the plugin. Restarting the session is the
simplest way. Zellij caches loaded plugins by URL and configuration.

## Usage

Press `Alt ;` in a terminal pane. A borderless menu opens just below the
cursor, or above it when there is no room below. It lists words from the
pane's scrollback, most recent first.

| Key                              | Action                               |
|----------------------------------|--------------------------------------|
| typing                           | filter (fuzzy; smart case)           |
| `Backspace`                      | delete the last filter character     |
| `Tab` / `Down` / `Ctrl-n`        | next candidate                       |
| `Shift-Tab` / `Up` / `Ctrl-p`    | previous candidate                   |
| `Enter`                          | insert the selected word, then close |
| `Esc` / `Ctrl-c`                 | cancel                               |

The filter starts as the partial word right before the cursor (text back to
the last whitespace or separator), so `git chec` + `Alt ;` offers `checkout`.
On `Enter` that partial word is completed: if the chosen word extends it, only
the remainder is typed; otherwise the partial word is erased with backspaces
and replaced. The partial word itself is never offered as a candidate.

The top row of the menu shows the filter. Nothing reaches the shell until you
press `Enter`. The word is inserted exactly as shown, with no trailing space
and no quoting. If the scrollback has no candidates, the menu shows
`no matches` and any key closes it.

## Configuration

Set these as key/value pairs inside the `MessagePlugin` block. All values are
strings.

| key              | default         | meaning |
|------------------|-----------------|---------|
| `separators`     | `()[]{}<>"',;`  | Extra characters that split tokens. Each character in the string is one separator. Whitespace always splits. |
| `trim_trailing`  | `.:`            | Characters stripped from the end of each token. |
| `min_length`     | `3`             | Drop tokens shorter than this. Counted in characters, after trimming. |
| `max_lines`      | `unlimited`     | Scan only the last N logical lines. `0`, `none` and `unlimited` all mean no limit. |
| `height`         | `10`            | Maximum popup height in rows, filter row included. Minimum `2`. The popup shrinks when there are fewer matches. |
| `rejoin_wrapped` | `true`          | Re-join soft-wrapped rows. A row that fills the pane's full width continues onto the next row. |

Zellij treats different configurations as different plugin instances. If you
change a value, Zellij loads a new instance and may ask for permissions again.

## Permissions

* `ReadPaneContents`: read the scrollback.
* `WriteToStdin`: type the chosen word into the origin pane.
* `ReadApplicationState`: find the focused pane, its cursor and geometry.
* `ChangeApplicationState`: position the popup and return focus.

## Development

```sh
make test    # cargo test: unit tests for all pure logic (host target)
make check   # cargo fmt --check + cargo clippy -D warnings (host target)
```

Layout:

* `src/config.rs`: parses the plugin configuration.
* `src/scrollback.rs`: orders the rows, re-joins wrapped rows, applies the
  `max_lines` cap.
* `src/candidates.rs`: tokenizes, trims, applies the minimum length,
  orders by recency and dedups.
* `src/ranking.rs`: fuzzy ranking (`nucleo-matcher`), ties broken by recency.
* `src/completion.rs`: the partial word before the cursor and the keystrokes that replace it.
* `src/menu.rs`: the menu state machine (filter, selection, scrolling, keys).
* `src/placement.rs`: popup placement below or above the cursor, clamped to
  the screen.
* `src/render.rs`: ANSI rendering and the popup's desired size.
* `src/main.rs`: the Zellij glue (events, permissions, pane I/O). It is only
  registered as a plugin when built for wasm.
