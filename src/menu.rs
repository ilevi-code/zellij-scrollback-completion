//! Menu state machine: filter text, selection, scrolling and key handling.

use crate::keys::{Bare, KeyPress};
use crate::ranking::{Match, Ranker};

/// What the plugin should do after a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Menu-only change (selection moved); re-render.
    Redraw,
    /// The key edited the word: forward it to the shell, then re-render.
    ForwardAndRedraw,
    /// Nothing to do.
    Ignore,
    /// Complete the word with this candidate, then close.
    Insert(String),
    /// Close without forwarding (`Esc`).
    Close,
    /// Close, then forward the key to the shell as if the menu weren't there.
    CloseAndForward,
}

/// One row of the list as it should be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow<'a> {
    pub text: &'a str,
    pub positions: &'a [u32],
    pub selected: bool,
}

pub struct Menu {
    candidates: Vec<String>,
    filter: String,
    matches: Vec<Match>,
    selected: usize,
    scroll: usize,
    max_list_rows: usize,
    ranker: Ranker,
}

impl Menu {
    #[cfg(test)]
    pub(crate) fn new(candidates: Vec<String>, height: usize) -> Menu {
        Menu::with_prefix(candidates, height, "")
    }

    /// A menu over `candidates` (most recent first) showing at most `height`
    /// rows, with the filter starting as `prefix` (the partial word before the
    /// cursor). A candidate equal to `prefix` is dropped: it is usually the
    /// partial word itself, harvested from the prompt line.
    pub fn with_prefix(mut candidates: Vec<String>, height: usize, prefix: &str) -> Menu {
        if !prefix.is_empty() {
            candidates.retain(|c| c != prefix);
        }
        let mut menu = Menu {
            candidates,
            filter: prefix.to_string(),
            matches: Vec::new(),
            selected: 0,
            scroll: 0,
            max_list_rows: height.max(1),
            ranker: Ranker::default(),
        };
        menu.refilter();
        menu
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// True if the scrollback produced no candidates at all.
    pub fn has_no_candidates(&self) -> bool {
        self.candidates.is_empty()
    }

    #[cfg(test)]
    fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    pub fn selected_word(&self) -> Option<&str> {
        self.matches
            .get(self.selected)
            .map(|m| self.candidates[m.index].as_str())
    }

    /// Number of list rows to show (at least 1, for the `no matches` row).
    pub fn list_rows(&self) -> usize {
        self.matches.len().clamp(1, self.max_list_rows)
    }

    /// Limits the number of list rows actually available (e.g. the popup had
    /// to be shrunk to fit the screen), keeping the selection visible.
    pub fn set_available_list_rows(&mut self, rows: usize) {
        self.ensure_visible(rows.max(1));
    }

    pub fn visible_rows(&self, rows: usize) -> Vec<VisibleRow<'_>> {
        self.matches
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(rows)
            .map(|(i, m)| VisibleRow {
                text: &self.candidates[m.index],
                positions: &m.positions,
                selected: i == self.selected,
            })
            .collect()
    }

    /// Handles a key typed while the menu is open. The filter mirrors the
    /// word being typed in the shell: printable non-separator characters and
    /// `Backspace` edit it (and are forwarded); navigation keys stay in the
    /// menu; `Enter` completes; `Esc` closes; anything else closes the menu
    /// and goes to the shell.
    pub fn handle_key(&mut self, key: &KeyPress, is_separator: impl Fn(char) -> bool) -> Outcome {
        let nav = |bare: Bare| key.bare == bare && key.no_mods();
        let ctrl = |c: char| key.bare == Bare::Char(c) && key.ctrl && !key.alt && !key.shift;
        if nav(Bare::Esc) {
            return Outcome::Close;
        }
        if self.has_no_candidates() {
            return Outcome::CloseAndForward;
        }
        if nav(Bare::Tab) || nav(Bare::Down) || ctrl('n') {
            return self.step(true);
        }
        if (key.bare == Bare::Tab && key.shift && !key.ctrl && !key.alt)
            || nav(Bare::Up)
            || ctrl('p')
        {
            return self.step(false);
        }
        if nav(Bare::Enter) {
            return match self.selected_word() {
                Some(word) => Outcome::Insert(word.to_string()),
                None => Outcome::CloseAndForward,
            };
        }
        if nav(Bare::Backspace) {
            return if self.filter.pop().is_some() {
                self.refilter();
                Outcome::ForwardAndRedraw
            } else {
                Outcome::CloseAndForward
            };
        }
        match key.typed_char() {
            Some(c) if !is_separator(c) => {
                self.filter.push(c);
                self.refilter();
                Outcome::ForwardAndRedraw
            }
            _ => Outcome::CloseAndForward,
        }
    }

    fn step(&mut self, forward: bool) -> Outcome {
        let n = self.matches.len();
        if n <= 1 {
            return Outcome::Ignore;
        }
        self.selected = if forward {
            (self.selected + 1) % n
        } else {
            (self.selected + n - 1) % n
        };
        self.ensure_visible(self.list_rows());
        Outcome::Redraw
    }

    fn ensure_visible(&mut self, rows: usize) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + rows {
            self.scroll = self.selected + 1 - rows;
        }
    }

    fn refilter(&mut self) {
        self.matches = self.ranker.rank(&self.candidates, &self.filter);
        self.selected = 0;
        self.scroll = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(words: &[&str], height: usize) -> Menu {
        Menu::new(words.iter().map(|s| s.to_string()).collect(), height)
    }

    fn texts(m: &Menu, rows: usize) -> Vec<&str> {
        m.visible_rows(rows).iter().map(|r| r.text).collect()
    }

    fn selected_text(m: &Menu, rows: usize) -> Option<String> {
        m.visible_rows(rows)
            .iter()
            .find(|r| r.selected)
            .map(|r| r.text.to_string())
    }

    fn sep(c: char) -> bool {
        c.is_whitespace() || c == '/'
    }

    fn press(m: &mut Menu, key: KeyPress) -> Outcome {
        m.handle_key(&key, sep)
    }

    fn ch(c: char) -> KeyPress {
        KeyPress::plain(Bare::Char(c))
    }

    fn key(b: Bare) -> KeyPress {
        KeyPress::plain(b)
    }

    #[test]
    fn prefix_prefills_filter_and_drops_itself() {
        let words = ["chec", "checkout", "cherry-pick", "status"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let m = Menu::with_prefix(words, 10, "chec");
        assert_eq!(m.filter(), "chec");
        let shown = texts(&m, 10);
        assert_eq!(shown[0], "checkout");
        assert!(!shown.contains(&"chec"));
        assert!(!shown.contains(&"status"));
        assert_eq!(m.selected_word(), Some("checkout"));
    }

    #[test]
    fn starts_with_first_selected() {
        let m = menu(&["one", "two", "three"], 10);
        assert_eq!(m.selected_word(), Some("one"));
        assert_eq!(m.list_rows(), 3);
        assert_eq!(texts(&m, 3), vec!["one", "two", "three"]);
    }

    #[test]
    fn navigation_keys_wrap_and_stay_in_menu() {
        let mut m = menu(&["a1", "a2", "a3"], 10);
        assert_eq!(press(&mut m, key(Bare::Up)), Outcome::Redraw);
        assert_eq!(m.selected_word(), Some("a3"));
        assert_eq!(press(&mut m, key(Bare::Tab)), Outcome::Redraw);
        assert_eq!(m.selected_word(), Some("a1"));
        press(&mut m, key(Bare::Down));
        assert_eq!(m.selected_word(), Some("a2"));
        press(&mut m, KeyPress::ctrl('n'));
        assert_eq!(m.selected_word(), Some("a3"));
        press(&mut m, KeyPress::ctrl('p'));
        press(&mut m, KeyPress::shift(Bare::Tab));
        assert_eq!(m.selected_word(), Some("a1"));
    }

    #[test]
    fn scrolls_to_keep_selection_visible() {
        let mut m = menu(&["w0", "w1", "w2", "w3", "w4"], 2);
        assert_eq!(m.list_rows(), 2);
        press(&mut m, key(Bare::Down));
        press(&mut m, key(Bare::Down));
        assert_eq!(texts(&m, 2), vec!["w1", "w2"]);
        assert_eq!(selected_text(&m, 2).as_deref(), Some("w2"));
        press(&mut m, key(Bare::Up));
        press(&mut m, key(Bare::Up));
        assert_eq!(texts(&m, 2), vec!["w0", "w1"]);
        press(&mut m, key(Bare::Up)); // wrap to last
        assert_eq!(texts(&m, 2), vec!["w3", "w4"]);
        assert_eq!(selected_text(&m, 2).as_deref(), Some("w4"));
    }

    #[test]
    fn shrinking_available_rows_keeps_selection_visible() {
        let mut m = menu(&["w0", "w1", "w2", "w3", "w4"], 10);
        for _ in 0..4 {
            press(&mut m, key(Bare::Down));
        }
        m.set_available_list_rows(2);
        assert_eq!(texts(&m, 2), vec!["w3", "w4"]);
    }

    #[test]
    fn typing_is_forwarded_and_filters() {
        let mut m = menu(&["foobar", "bazqux", "foobaz"], 10);
        press(&mut m, key(Bare::Down));
        assert_eq!(press(&mut m, ch('q')), Outcome::ForwardAndRedraw);
        assert_eq!(m.filter(), "q");
        assert_eq!(texts(&m, 10), vec!["bazqux"]);
        assert_eq!(m.selected_word(), Some("bazqux"));
        assert_eq!(
            press(&mut m, KeyPress::shift(Bare::Char('u'))),
            Outcome::ForwardAndRedraw
        );
        assert_eq!(m.filter(), "qU");
    }

    #[test]
    fn backspace_is_forwarded_until_word_is_empty() {
        let mut m = Menu::with_prefix(vec!["foobar".into(), "bazqux".into()], 10, "q");
        assert_eq!(
            press(&mut m, key(Bare::Backspace)),
            Outcome::ForwardAndRedraw
        );
        assert_eq!(m.filter(), "");
        assert_eq!(texts(&m, 10), vec!["foobar", "bazqux"]);
        assert_eq!(
            press(&mut m, key(Bare::Backspace)),
            Outcome::CloseAndForward
        );
    }

    #[test]
    fn separator_closes_and_forwards() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(press(&mut m, ch(' ')), Outcome::CloseAndForward);
        assert_eq!(press(&mut m, ch('/')), Outcome::CloseAndForward);
        assert_eq!(m.filter(), "");
    }

    #[test]
    fn other_keys_close_and_forward() {
        for k in [
            key(Bare::Other),
            KeyPress::ctrl('c'),
            KeyPress::ctrl('a'),
            KeyPress {
                alt: true,
                ..ch('b')
            },
        ] {
            let mut m = menu(&["abc"], 10);
            assert_eq!(press(&mut m, k), Outcome::CloseAndForward, "{k:?}");
        }
    }

    #[test]
    fn enter_inserts_selected_verbatim() {
        let mut m = menu(&["it's", "--flag=\"x\""], 10);
        press(&mut m, key(Bare::Down));
        assert_eq!(
            press(&mut m, key(Bare::Enter)),
            Outcome::Insert("--flag=\"x\"".into())
        );
    }

    #[test]
    fn esc_closes_without_forwarding() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(press(&mut m, key(Bare::Esc)), Outcome::Close);
        let mut empty = menu(&[], 10);
        assert_eq!(press(&mut empty, key(Bare::Esc)), Outcome::Close);
    }

    #[test]
    fn no_matches_stays_open_while_typing() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(press(&mut m, ch('z')), Outcome::ForwardAndRedraw);
        assert!(!m.has_matches());
        assert_eq!(m.list_rows(), 1);
        assert!(m.visible_rows(1).is_empty());
        assert_eq!(press(&mut m, key(Bare::Down)), Outcome::Ignore);
        assert_eq!(
            press(&mut m, key(Bare::Backspace)),
            Outcome::ForwardAndRedraw
        );
        assert!(m.has_matches());
    }

    #[test]
    fn enter_with_no_matches_goes_to_shell() {
        let mut m = menu(&["abc"], 10);
        press(&mut m, ch('z'));
        assert_eq!(press(&mut m, key(Bare::Enter)), Outcome::CloseAndForward);
    }

    #[test]
    fn empty_scrollback_any_key_closes_and_forwards() {
        for k in [
            ch('a'),
            key(Bare::Down),
            key(Bare::Enter),
            key(Bare::Backspace),
        ] {
            let mut m = menu(&[], 10);
            assert!(m.has_no_candidates());
            assert_eq!(m.list_rows(), 1);
            assert_eq!(press(&mut m, k), Outcome::CloseAndForward, "{k:?}");
        }
    }

    #[test]
    fn single_match_navigation_is_noop() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(press(&mut m, key(Bare::Down)), Outcome::Ignore);
        assert_eq!(m.selected_word(), Some("abc"));
    }

    #[test]
    fn list_rows_capped_by_height() {
        let words: Vec<String> = (0..50).map(|i| format!("word{i}")).collect();
        let m = Menu::new(words, 10);
        assert_eq!(m.list_rows(), 10);
    }
}
