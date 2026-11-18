# zellij-scrollback-completion — Roadmap / Design

A Zellij plugin that pops up a vim-style completion menu of words harvested from
the focused pane's scrollback, and types the chosen word into that pane.

Target: Zellij **0.46.0**, Rust, `zellij-tile` 0.46, compiled to `wasm32-wasip1`.

## User-facing behaviour

- Trigger: keybind `Alt ;` (bound in `shared_except "locked"`).
- A small floating menu opens **directly below the cursor** of the origin pane
  (above it if there is not enough room below), like vim's completion popup.
- The filter is pre-filled with the **partial word before the cursor** (back to
  the last whitespace/separator). On Enter that partial word is replaced by the
  chosen word: only the remainder is typed if the word extends it, otherwise
  the partial word is erased with backspaces (DEL) first. The partial word is
  never offered as a candidate.
- Typing filters the list; the chosen word is inserted into the origin pane
  **verbatim** (no trailing space, no quoting) at the cursor.
- The word always goes to the pane that was focused when `Alt ;` was pressed,
  including when that pane is floating.

## Candidate extraction

- Source: the origin pane's **full scrollback** (viewport + lines above/below),
  capped by `max_lines` (default: unlimited / whole buffer).
- Wrapped rows are re-joined into logical lines (heuristic: a row whose length
  equals the pane's content width continues onto the next row). Toggle:
  `rejoin_wrapped` (default `true`).
- Tokenize on whitespace plus a configurable separator set.
  Default extra separators: `( ) [ ] { } < > " ' , ;`
- Trim a configurable set of trailing characters. Default: `. :`
- Drop tokens shorter than `min_length` (default `3`).
- No exclusions (the current prompt line is NOT excluded; no exclude regexes).
- Ordering: most recent first (closest to the bottom), deduplicated (keep the
  most recent occurrence). Once the user types, ordering is by fuzzy score
  (`nucleo-matcher`), ties broken by recency.

## Popup UI

- About `height` rows (default `10`), as wide as the longest visible
  candidate, capped to the screen.
- No border / title / counter if Zellij allows; otherwise the thinnest frame
  possible. Selected row highlighted; fuzzy-matched characters highlighted.
- **Typing continues in the shell** (vim-style): printable non-separator
  characters and `Backspace` are forwarded to the origin pane and the menu
  re-filters on the word being typed. There is no filter row. The menu is
  anchored at the start of the word. No live preview of the selection.
- Keys:
  - `Tab` / `Down` / `Ctrl-n`: next
  - `Shift-Tab` / `Up` / `Ctrl-p`: previous
  - `Enter`: complete the word with the selection, then hide
  - `Esc`: close without forwarding
  - Separator, `Backspace` past the word start, `Enter` with no matches, any
    other key: close, then forward the key to the shell
- Empty states:
  - Word matches nothing: show a dim `no matches` row; menu stays open.
  - Scrollback has no candidates at all: show `no matches`; the next key
    closes the menu (and is forwarded, except `Esc`).
- The menu keeps Zellij focus (focusing a tiled pane hides the floating
  layer), so the shell cursor isn't drawn while it's open. Future: Zellij
  0.46's unfocused plugin popups (`open_plugin_popup_with_options`) could
  avoid this.

## Lifecycle

- Plugin stays loaded between uses and **hides itself** (`hide_self`) after
  insert/cancel, so reopening is instant and permissions are prompted once.
- Trigger mechanism (implementation detail): the keybind sends a
  `MessagePlugin` so the terminal pane stays focused while the plugin records
  the origin pane id and reads its scrollback; the plugin then shows itself.
  After inserting, focus returns to the origin pane.
- Permissions: `ReadPaneContents`, `WriteToStdin`, `ReadApplicationState`,
  `ChangeApplicationState` (only what's actually needed).

## Configuration

All options passed as key/value pairs in the plugin block of the keybind in
`config.kdl` (no separate config file):

| key              | default                   |
|------------------|---------------------------|
| `separators`     | `()[]{}<>"',;` (+ whitespace always) |
| `trim_trailing`  | `.:`                      |
| `min_length`     | `3`                       |
| `max_lines`      | unlimited                 |
| `height`         | `10`                      |
| `rejoin_wrapped` | `true`                    |

## Code structure & testing

- Pure logic (tokenizing, trimming, wrap re-joining, dedup, recency/fuzzy
  ranking, popup placement math, config parsing) lives in modules with no
  `zellij-tile` dependency, covered by unit tests runnable with plain
  `cargo test` on the host target.
- Zellij glue (events, permissions, rendering, pane I/O) is thin and verified
  manually.
- A `justfile`/`Makefile` builds the release wasm
  (`target/wasm32-wasip1/release/zellij-scrollback-completion.wasm`).

## Setup outside this repo (done manually by the user)

- `rustup target add wasm32-wasip1`
- Add the `Alt ;` keybind to `~/projects/configs/config.kdl`, pointing at the
  built wasm.

## Milestones

1. Scaffold crate, build script, `git init`.
2. Pure logic + unit tests.
3. Zellij glue: trigger, origin pane capture, scrollback read, insert, hide.
4. Popup rendering, positioning at cursor, keys, empty states.
5. README with install + keybind snippet.
