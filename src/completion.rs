//! The word under completion: finding it before the cursor and replacing it.

use unicode_width::UnicodeWidthChar;

use crate::config::Config;

/// The partial word immediately before display column `col` of `line`: the
/// run of non-separator characters ending at the cursor. Empty if the cursor
/// follows a separator (or the line is shorter than `col`).
pub fn word_before_cursor(line: &str, col: usize, config: &Config) -> String {
    let mut width = 0;
    let mut end = 0;
    for (i, c) in line.char_indices() {
        let w = c.width().unwrap_or(0);
        if width + w > col {
            break;
        }
        width += w;
        end = i + c.len_utf8();
    }
    let before = &line[..end];
    let start = before
        .char_indices()
        .rev()
        .find(|&(_, c)| config.is_separator(c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    before[start..].to_string()
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

    #[test]
    fn word_ending_at_cursor() {
        assert_eq!(word_before_cursor("$ git chec", 10, &cfg()), "chec");
    }

    #[test]
    fn cursor_mid_line_ignores_text_after() {
        assert_eq!(word_before_cursor("$ cat foo bar", 8, &cfg()), "fo");
    }

    #[test]
    fn after_space_is_empty() {
        assert_eq!(word_before_cursor("$ ls ", 5, &cfg()), "");
    }

    #[test]
    fn stops_at_configured_separator() {
        assert_eq!(word_before_cursor("echo \"src/ma", 12, &cfg()), "src/ma");
        assert_eq!(word_before_cursor("f(arg", 5, &cfg()), "arg");
    }

    #[test]
    fn whole_line_is_word() {
        assert_eq!(word_before_cursor("abc", 3, &cfg()), "abc");
    }

    #[test]
    fn cursor_past_end_takes_last_word() {
        assert_eq!(word_before_cursor("$ vim REA", 40, &cfg()), "REA");
    }

    #[test]
    fn wide_chars_count_by_display_width() {
        // "日本" is 4 columns wide.
        assert_eq!(word_before_cursor("日本 ab", 7, &cfg()), "ab");
        assert_eq!(word_before_cursor("x 日本", 4, &cfg()), "日");
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
