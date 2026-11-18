//! Rendering the menu to ANSI text, and computing the popup's desired size.

use crate::menu::{Menu, VisibleRow};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const NO_MATCHES: &str = "no matches";
/// Never narrower than this, so the popup stays usable.
pub const MIN_WIDTH: usize = 12;

const RESET: &str = "\u{1b}[0m";
const REVERSE: &str = "\u{1b}[7m";
const DIM: &str = "\u{1b}[2m";
const MATCH_ON: &str = "\u{1b}[1;33m";
const MATCH_OFF: &str = "\u{1b}[22;39m";

/// Desired popup size `(width, height)` before clamping to the screen: as
/// wide as the widest visible candidate, one row per visible candidate (or
/// one `no matches` row). There is no filter row: the word being completed
/// is visible on the shell prompt itself.
pub fn desired_size(menu: &Menu) -> (usize, usize) {
    let rows = menu.list_rows();
    let widest = menu
        .visible_rows(rows)
        .iter()
        .map(|r| r.text.width())
        .max()
        .unwrap_or(0);
    let width = widest.max(NO_MATCHES.width()).max(MIN_WIDTH);
    (width, rows)
}

/// Renders the popup into exactly `rows` lines (fewer if `rows` is 0), each
/// at most `cols` columns wide.
pub fn render_lines(menu: &Menu, rows: usize, cols: usize) -> Vec<String> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity(rows);
    let visible = menu.visible_rows(rows);
    if visible.is_empty() {
        lines.push(format!(
            "{DIM}{}{RESET}",
            pad(&truncate(NO_MATCHES, cols), cols)
        ));
    }
    for row in &visible {
        lines.push(candidate_line(row, cols));
    }
    while lines.len() < rows {
        lines.push(" ".repeat(cols));
    }
    lines
}

fn candidate_line(row: &VisibleRow<'_>, cols: usize) -> String {
    let full_width = row.text.width();
    let (limit, ellipsis) = if full_width > cols {
        (cols.saturating_sub(1), true)
    } else {
        (cols, false)
    };
    let mut line = String::new();
    if row.selected {
        line.push_str(REVERSE);
    }
    let mut used = 0;
    for (i, ch) in row.text.chars().enumerate() {
        let w = ch.width().unwrap_or(0);
        if used + w > limit {
            break;
        }
        used += w;
        if row.positions.binary_search(&(i as u32)).is_ok() {
            line.push_str(MATCH_ON);
            line.push(ch);
            line.push_str(MATCH_OFF);
        } else {
            line.push(ch);
        }
    }
    if ellipsis && used < cols {
        line.push('…');
        used += 1;
    }
    line.push_str(&" ".repeat(cols.saturating_sub(used)));
    line.push_str(RESET);
    line
}

/// Cuts `s` to at most `cols` display columns (from the left).
fn truncate(s: &str, cols: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > cols {
            break;
        }
        used += w;
        out.push(ch);
    }
    out
}

fn pad(s: &str, cols: usize) -> String {
    let w = s.width();
    format!("{s}{}", " ".repeat(cols.saturating_sub(w)))
}

/// Removes ANSI CSI sequences (used by tests, handy for debugging).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{Bare, KeyPress};

    fn menu(words: &[&str], height: usize) -> Menu {
        Menu::new(words.iter().map(|s| s.to_string()).collect(), height)
    }

    fn typed(words: &[&str], filter: &str) -> Menu {
        let mut m = menu(words, 10);
        for c in filter.chars() {
            m.handle_key(&KeyPress::plain(Bare::Char(c)), char::is_whitespace);
        }
        m
    }

    fn plain(lines: &[String]) -> Vec<String> {
        lines.iter().map(|l| strip_ansi(l)).collect()
    }

    #[test]
    fn size_fits_longest_visible_candidate() {
        let m = menu(&["short", "a-much-longer-candidate"], 10);
        assert_eq!(desired_size(&m), (23, 2));
    }

    #[test]
    fn size_has_minimum_width() {
        let m = menu(&["abc"], 10);
        assert_eq!(desired_size(&m), (MIN_WIDTH, 1));
    }

    #[test]
    fn size_counts_only_visible_rows() {
        let m = menu(&["a1x", "a2x", "a-very-very-long-hidden-one"], 2);
        assert_eq!(desired_size(&m), (MIN_WIDTH, 2));
    }

    #[test]
    fn size_for_empty_and_no_matches() {
        assert_eq!(desired_size(&menu(&[], 10)), (MIN_WIDTH, 1));
        assert_eq!(desired_size(&typed(&["abc"], "z")), (MIN_WIDTH, 1));
    }

    #[test]
    fn renders_rows_with_selection() {
        let mut m = menu(&["alpha", "beta"], 10);
        m.handle_key(&KeyPress::plain(Bare::Down), char::is_whitespace);
        let lines = render_lines(&m, 2, 12);
        assert_eq!(plain(&lines), vec!["alpha       ", "beta        "]);
        assert!(!lines[0].contains(REVERSE));
        assert!(lines[1].starts_with(REVERSE));
    }

    #[test]
    fn every_line_is_exactly_cols_wide() {
        let m = typed(
            &["alpha", "日本語テキスト", "a-very-long-candidate-name"],
            "a",
        );
        for cols in [3, 8, 12, 40] {
            for line in plain(&render_lines(&m, 5, cols)) {
                assert_eq!(line.width(), cols, "{line:?} @ {cols}");
            }
        }
    }

    #[test]
    fn highlights_matched_chars() {
        let m = typed(&["xaxb"], "ab");
        let lines = render_lines(&m, 1, 12);
        let expected = format!("{REVERSE}x{MATCH_ON}a{MATCH_OFF}x{MATCH_ON}b{MATCH_OFF}");
        assert!(lines[0].starts_with(&expected), "{:?}", lines[0]);
    }

    #[test]
    fn truncates_long_candidates_with_ellipsis() {
        let m = menu(&["abcdefghijkl"], 10);
        assert_eq!(plain(&render_lines(&m, 1, 6)), vec!["abcde…"]);
    }

    #[test]
    fn no_matches_row_is_dim() {
        let m = typed(&["abc"], "z");
        let lines = render_lines(&m, 1, 12);
        assert!(lines[0].starts_with(DIM));
        assert_eq!(strip_ansi(&lines[0]), "no matches  ");
    }

    #[test]
    fn empty_scrollback_shows_no_matches() {
        let m = menu(&[], 10);
        assert_eq!(plain(&render_lines(&m, 1, 12)), vec!["no matches  "]);
    }

    #[test]
    fn pads_extra_rows_and_handles_zero() {
        let m = menu(&["abc"], 10);
        assert_eq!(render_lines(&m, 4, 5).len(), 4);
        assert!(render_lines(&m, 0, 5).is_empty());
        assert!(render_lines(&m, 3, 0).is_empty());
    }

    #[test]
    fn strip_ansi_works() {
        assert_eq!(strip_ansi("\u{1b}[1;33mab\u{1b}[0mc"), "abc");
    }
}
