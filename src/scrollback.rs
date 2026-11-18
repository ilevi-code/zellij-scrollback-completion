//! Turning the visual rows Zellij hands us into logical lines.

use unicode_width::UnicodeWidthStr;

/// Concatenates the three scrollback regions in screen order (oldest first).
pub fn ordered_rows<'a>(
    lines_above_viewport: &'a [String],
    viewport: &'a [String],
    lines_below_viewport: &'a [String],
) -> impl Iterator<Item = &'a str> {
    lines_above_viewport
        .iter()
        .chain(viewport)
        .chain(lines_below_viewport)
        .map(String::as_str)
}

/// Display width of a row ignoring trailing whitespace. Zellij pads some rows
/// with blanks, so trailing whitespace must not count towards "full".
fn visible_width(row: &str) -> usize {
    row.trim_end().width()
}

/// Re-joins soft-wrapped rows into logical lines.
///
/// Heuristic: a row whose (trailing-whitespace-trimmed) display width reaches
/// the pane's content width continues onto the next row. With
/// `content_columns == 0` (unknown width) or `rejoin == false`, rows are
/// returned unchanged.
pub fn logical_lines<'a, I>(rows: I, content_columns: usize, rejoin: bool) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let rejoin = rejoin && content_columns > 0;
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut continuing = false;
    for row in rows {
        if continuing {
            current.push_str(row);
        } else {
            current = row.to_string();
        }
        continuing = rejoin && visible_width(row) >= content_columns;
        if !continuing {
            lines.push(std::mem::take(&mut current));
        }
    }
    if continuing {
        lines.push(current);
    }
    lines
}

/// Keeps only the last `max_lines` lines (the ones closest to the bottom).
pub fn cap_lines(mut lines: Vec<String>, max_lines: Option<usize>) -> Vec<String> {
    if let Some(max) = max_lines {
        if lines.len() > max {
            lines.drain(..lines.len() - max);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn orders_above_viewport_below() {
        let above = rows(&["a1", "a2"]);
        let view = rows(&["v1"]);
        let below = rows(&["b1"]);
        let all: Vec<&str> = ordered_rows(&above, &view, &below).collect();
        assert_eq!(all, vec!["a1", "a2", "v1", "b1"]);
    }

    #[test]
    fn short_rows_are_separate_lines() {
        let l = logical_lines(["abc", "def"], 10, true);
        assert_eq!(l, vec!["abc", "def"]);
    }

    #[test]
    fn full_rows_are_joined() {
        // width 5: "hello" is full, so "world" continues it.
        let l = logical_lines(["hello", "wor", "next"], 5, true);
        assert_eq!(l, vec!["hellowor", "next"]);
    }

    #[test]
    fn multiple_wraps_join_into_one_line() {
        let l = logical_lines(["aaaa", "bbbb", "cc", "d"], 4, true);
        assert_eq!(l, vec!["aaaabbbbcc", "d"]);
    }

    #[test]
    fn trailing_blanks_do_not_count_as_full() {
        let l = logical_lines(["ab   ", "cd"], 5, true);
        assert_eq!(l, vec!["ab   ", "cd"]);
    }

    #[test]
    fn last_full_row_is_kept() {
        let l = logical_lines(["abcd", "efgh"], 4, true);
        assert_eq!(l, vec!["abcdefgh"]);
    }

    #[test]
    fn wide_chars_count_by_display_width() {
        // two CJK chars = 4 columns.
        let l = logical_lines(["日本", "語"], 4, true);
        assert_eq!(l, vec!["日本語"]);
        let l = logical_lines(["日本", "語"], 5, true);
        assert_eq!(l, vec!["日本", "語"]);
    }

    #[test]
    fn rejoin_disabled_or_unknown_width() {
        assert_eq!(
            logical_lines(["abcd", "efgh"], 4, false),
            vec!["abcd", "efgh"]
        );
        assert_eq!(
            logical_lines(["abcd", "efgh"], 0, true),
            vec!["abcd", "efgh"]
        );
    }

    #[test]
    fn empty_rows() {
        assert!(logical_lines(Vec::<&str>::new(), 4, true).is_empty());
        assert_eq!(logical_lines(["", ""], 4, true), vec!["", ""]);
    }

    #[test]
    fn cap_keeps_most_recent() {
        let l = rows(&["1", "2", "3", "4"]);
        assert_eq!(cap_lines(l.clone(), Some(2)), rows(&["3", "4"]));
        assert_eq!(cap_lines(l.clone(), Some(10)), l);
        assert_eq!(cap_lines(l.clone(), None), l);
        assert!(cap_lines(l, Some(0)).is_empty());
    }
}
