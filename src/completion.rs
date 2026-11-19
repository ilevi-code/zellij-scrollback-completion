//! The word under completion: finding it before the cursor and replacing it.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::Config;
use crate::scrollback;

/// The partial word being completed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CursorWord {
    /// The word's text, including any part that wrapped from earlier rows.
    pub text: String,
    /// Content column on the cursor row where the visible part of the word
    /// starts (`0` when the word continues from the previous row).
    pub start_col: usize,
}

/// Finds the partial word ending at `cursor` (content-relative `(col, row)`)
/// in `viewport`. If the word starts at the beginning of the cursor row and
/// the row above soft-wrapped into it, the word is followed back across the
/// wrapped rows (when `config.rejoin_wrapped` is on).
pub fn cursor_word(
    viewport: &[String],
    cursor: (usize, usize),
    content_columns: usize,
    config: &Config,
) -> CursorWord {
    let (col, row) = cursor;
    let Some(line) = viewport.get(row) else {
        return CursorWord::default();
    };
    let before = cols_prefix(line, col);
    let visible = trailing_word(before, config);
    let start_col = before.width() - visible.width();

    let mut text = visible.to_string();
    if start_col == 0 && config.rejoin_wrapped {
        for above in viewport[..row].iter().rev() {
            if !scrollback::wraps(above, content_columns) {
                break;
            }
            let word = trailing_word(above, config);
            text.insert_str(0, word);
            if word.len() != above.len() {
                break; // the word starts inside this row
            }
        }
    }
    CursorWord { text, start_col }
}

/// The part of `line` that lies before display column `col`.
fn cols_prefix(line: &str, col: usize) -> &str {
    let mut width = 0;
    for (i, c) in line.char_indices() {
        width += c.width().unwrap_or(0);
        if width > col {
            return &line[..i];
        }
    }
    line
}

/// The run of non-separator characters at the end of `s`.
fn trailing_word<'a>(s: &'a str, config: &Config) -> &'a str {
    let start = s
        .char_indices()
        .rev()
        .find(|&(_, c)| config.is_separator(c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    &s[start..]
}

/// The characters to type so that `prefix` (already in the shell, right
/// before the cursor) becomes `word`. If `word` extends `prefix`, only the
/// rest is typed; otherwise `prefix` is erased with backspaces (DEL) first.
pub fn replacement(prefix: &str, word: &str) -> String {
    if let Some(rest) = word.strip_prefix(prefix) {
        return rest.to_string();
    }
    let mut out = "\u{7f}".repeat(prefix.chars().count());
    out.push_str(word);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn cfg() -> Config {
        Config::from_map(&BTreeMap::new()).0
    }

    fn rows(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn word(line: &str, col: usize) -> CursorWord {
        cursor_word(&rows(&[line]), (col, 0), 80, &cfg())
    }

    fn cw(text: &str, start_col: usize) -> CursorWord {
        CursorWord {
            text: text.into(),
            start_col,
        }
    }

    #[test]
    fn word_ending_at_cursor() {
        assert_eq!(word("$ git chec", 10), cw("chec", 6));
    }

    #[test]
    fn cursor_mid_line_ignores_text_after() {
        assert_eq!(word("$ cat foo bar", 8), cw("fo", 6));
    }

    #[test]
    fn after_space_is_empty() {
        assert_eq!(word("$ ls ", 5), cw("", 5));
    }

    #[test]
    fn stops_at_configured_separator() {
        assert_eq!(word("echo \"src/ma", 12).text, "src/ma");
        assert_eq!(word("f(arg", 5).text, "arg");
    }

    #[test]
    fn whole_line_is_word() {
        assert_eq!(word("abc", 3), cw("abc", 0));
    }

    #[test]
    fn cursor_past_end_takes_last_word() {
        assert_eq!(word("$ vim REA", 40), cw("REA", 6));
    }

    #[test]
    fn wide_chars_count_by_display_width() {
        // "日本" is 4 columns wide.
        assert_eq!(word("日本 ab", 7), cw("ab", 5));
        assert_eq!(word("x 日本", 4), cw("日", 2));
    }

    #[test]
    fn missing_row_is_empty() {
        assert_eq!(cursor_word(&rows(&["abc"]), (1, 5), 80, &cfg()), cw("", 0));
    }

    #[test]
    fn word_straddling_a_wrap_is_rejoined() {
        // 10-column pane: "$ ls /usr/lo" wrapped after column 10.
        let viewport = rows(&["$ ls /usr/", "local/b"]);
        assert_eq!(
            cursor_word(&viewport, (7, 1), 10, &cfg()),
            cw("/usr/local/b", 0)
        );
    }

    #[test]
    fn word_spanning_several_full_rows() {
        let viewport = rows(&["abcd", "efgh", "ij"]);
        assert_eq!(
            cursor_word(&viewport, (2, 2), 4, &cfg()),
            cw("abcdefghij", 0)
        );
    }

    #[test]
    fn short_previous_row_is_not_joined() {
        let viewport = rows(&["$ ls x", "abc"]);
        assert_eq!(cursor_word(&viewport, (3, 1), 10, &cfg()), cw("abc", 0));
    }

    #[test]
    fn wrap_rejoin_can_be_disabled() {
        let map: BTreeMap<String, String> =
            [("rejoin_wrapped".to_string(), "false".to_string())].into();
        let config = Config::from_map(&map).0;
        let viewport = rows(&["$ ls /usr/", "local/b"]);
        assert_eq!(
            cursor_word(&viewport, (7, 1), 10, &config),
            cw("local/b", 0)
        );
    }

    #[test]
    fn replacement_extends_prefix() {
        assert_eq!(replacement("chec", "checkout"), "kout");
        assert_eq!(replacement("", "word"), "word");
    }

    #[test]
    fn replacement_erases_non_prefix_match() {
        assert_eq!(
            replacement("rdm", "README.md"),
            "\u{7f}\u{7f}\u{7f}README.md"
        );
    }
}
