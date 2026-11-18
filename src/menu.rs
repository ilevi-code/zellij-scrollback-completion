//! Menu state machine: filter text, selection, scrolling and key handling.

use crate::ranking::{Match, Ranker};

/// Keys the menu understands, already decoded from Zellij's key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// `Tab`, `Down`, `Ctrl-n`
    Next,
    /// `Shift-Tab`, `Up`, `Ctrl-p`
    Prev,
    /// `Enter`
    Accept,
    /// `Esc`, `Ctrl-c`
    Cancel,
    Backspace,
    /// A printable character to append to the filter.
    Char(char),
    /// Anything else.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// State changed; re-render (and maybe resize).
    Redraw,
    /// Nothing changed.
    Ignore,
    /// Insert this word into the origin pane, then hide.
    Insert(String),
    /// Hide without inserting.
    Close,
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
    /// `candidates` must be in recency order; `height` is the total popup
    /// height including the filter row.
    pub fn new(candidates: Vec<String>, height: usize) -> Menu {
        Menu::with_prefix(candidates, height, "")
    }

    /// Like [`Menu::new`], but the filter starts as `prefix` (the partial word
    /// before the cursor). A candidate equal to `prefix` is dropped: it is
    /// usually the partial word itself, harvested from the prompt line.
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
            max_list_rows: height.saturating_sub(1).max(1),
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

    pub fn has_matches(&self) -> bool {
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

    pub fn handle_key(&mut self, key: Key) -> Outcome {
        if self.has_no_candidates() {
            return Outcome::Close;
        }
        match key {
            Key::Cancel => Outcome::Close,
            Key::Accept => match self.selected_word() {
                Some(word) => Outcome::Insert(word.to_string()),
                None => Outcome::Ignore,
            },
            Key::Next => self.step(true),
            Key::Prev => self.step(false),
            Key::Backspace => {
                if self.filter.pop().is_some() {
                    self.refilter();
                    Outcome::Redraw
                } else {
                    Outcome::Ignore
                }
            }
            Key::Char(c) if !c.is_control() => {
                self.filter.push(c);
                self.refilter();
                Outcome::Redraw
            }
            Key::Char(_) | Key::Other => Outcome::Ignore,
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
    fn prefix_can_be_backspaced() {
        let words = ["checkout", "cherry-pick"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut m = Menu::with_prefix(words, 10, "chec");
        m.handle_key(Key::Backspace);
        m.handle_key(Key::Backspace);
        assert_eq!(m.filter(), "ch");
        assert_eq!(texts(&m, 10).len(), 2);
    }

    #[test]
    fn starts_with_first_selected() {
        let m = menu(&["one", "two", "three"], 10);
        assert_eq!(m.selected_word(), Some("one"));
        assert_eq!(m.list_rows(), 3);
        assert_eq!(texts(&m, 3), vec!["one", "two", "three"]);
    }

    #[test]
    fn next_prev_wrap_around() {
        let mut m = menu(&["a1", "a2", "a3"], 10);
        assert_eq!(m.handle_key(Key::Prev), Outcome::Redraw);
        assert_eq!(m.selected_word(), Some("a3"));
        assert_eq!(m.handle_key(Key::Next), Outcome::Redraw);
        assert_eq!(m.selected_word(), Some("a1"));
        m.handle_key(Key::Next);
        assert_eq!(m.selected_word(), Some("a2"));
    }

    #[test]
    fn scrolls_to_keep_selection_visible() {
        let mut m = menu(&["w0", "w1", "w2", "w3", "w4"], 3); // 2 list rows
        assert_eq!(m.list_rows(), 2);
        m.handle_key(Key::Next);
        m.handle_key(Key::Next);
        assert_eq!(texts(&m, 2), vec!["w1", "w2"]);
        assert_eq!(selected_text(&m, 2).as_deref(), Some("w2"));
        m.handle_key(Key::Prev);
        m.handle_key(Key::Prev);
        assert_eq!(texts(&m, 2), vec!["w0", "w1"]);
        m.handle_key(Key::Prev); // wrap to last
        assert_eq!(texts(&m, 2), vec!["w3", "w4"]);
        assert_eq!(selected_text(&m, 2).as_deref(), Some("w4"));
    }

    #[test]
    fn shrinking_available_rows_keeps_selection_visible() {
        let mut m = menu(&["w0", "w1", "w2", "w3", "w4"], 10);
        for _ in 0..4 {
            m.handle_key(Key::Next);
        }
        m.set_available_list_rows(2);
        assert_eq!(texts(&m, 2), vec!["w3", "w4"]);
    }

    #[test]
    fn typing_filters_and_resets_selection() {
        let mut m = menu(&["foobar", "bazqux", "foobaz"], 10);
        m.handle_key(Key::Next);
        assert_eq!(m.handle_key(Key::Char('q')), Outcome::Redraw);
        assert_eq!(m.filter(), "q");
        assert_eq!(texts(&m, 10), vec!["bazqux"]);
        assert_eq!(m.selected_word(), Some("bazqux"));
    }

    #[test]
    fn backspace_edits_filter() {
        let mut m = menu(&["foobar", "bazqux"], 10);
        m.handle_key(Key::Char('q'));
        assert_eq!(m.handle_key(Key::Backspace), Outcome::Redraw);
        assert_eq!(m.filter(), "");
        assert_eq!(texts(&m, 10), vec!["foobar", "bazqux"]);
        assert_eq!(m.handle_key(Key::Backspace), Outcome::Ignore);
    }

    #[test]
    fn enter_inserts_selected_verbatim() {
        let mut m = menu(&["it's", "--flag=\"x\""], 10);
        m.handle_key(Key::Next);
        assert_eq!(
            m.handle_key(Key::Accept),
            Outcome::Insert("--flag=\"x\"".into())
        );
    }

    #[test]
    fn cancel_closes() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(m.handle_key(Key::Cancel), Outcome::Close);
    }

    #[test]
    fn no_matches_stays_open() {
        let mut m = menu(&["abc"], 10);
        m.handle_key(Key::Char('z'));
        assert!(!m.has_matches());
        assert_eq!(m.list_rows(), 1);
        assert!(m.visible_rows(1).is_empty());
        assert_eq!(m.handle_key(Key::Accept), Outcome::Ignore);
        assert_eq!(m.handle_key(Key::Next), Outcome::Ignore);
        assert_eq!(m.handle_key(Key::Backspace), Outcome::Redraw);
        assert!(m.has_matches());
    }

    #[test]
    fn empty_scrollback_any_key_closes() {
        for key in [
            Key::Char('a'),
            Key::Next,
            Key::Accept,
            Key::Backspace,
            Key::Other,
        ] {
            let mut m = menu(&[], 10);
            assert!(m.has_no_candidates());
            assert_eq!(m.list_rows(), 1);
            assert_eq!(m.handle_key(key), Outcome::Close, "{key:?}");
        }
    }

    #[test]
    fn control_chars_and_other_keys_are_ignored() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(m.handle_key(Key::Char('\u{1b}')), Outcome::Ignore);
        assert_eq!(m.handle_key(Key::Other), Outcome::Ignore);
        assert_eq!(m.filter(), "");
    }

    #[test]
    fn single_match_navigation_is_noop() {
        let mut m = menu(&["abc"], 10);
        assert_eq!(m.handle_key(Key::Next), Outcome::Ignore);
        assert_eq!(m.selected_word(), Some("abc"));
    }

    #[test]
    fn list_rows_capped_by_height() {
        let words: Vec<String> = (0..50).map(|i| format!("word{i}")).collect();
        let m = Menu::new(words, 10);
        assert_eq!(m.list_rows(), 9);
    }
}
