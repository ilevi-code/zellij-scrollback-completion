//! Tokenizing logical lines into completion candidates.

use crate::config::Config;

/// Splits `line` into raw tokens on whitespace and the configured separators.
pub fn tokens<'a>(line: &'a str, config: &'a Config) -> impl DoubleEndedIterator<Item = &'a str> {
    line.split(move |c: char| config.is_separator(c))
        .filter(|t| !t.is_empty())
}

/// Strips any run of the configured trailing characters from `token`.
pub fn trim_token<'a>(token: &'a str, config: &Config) -> &'a str {
    token.trim_end_matches(|c: char| config.trim_trailing.contains(&c))
}

/// Extracts candidates from `lines` (oldest first), returning them most
/// recent first (closest to the bottom, and rightmost within a line), with
/// duplicates removed (the most recent occurrence wins).
pub fn extract(lines: &[String], config: &Config) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in lines.iter().rev() {
        for raw in tokens(line, config).rev() {
            let token = trim_token(raw, config);
            if token.chars().count() < config.min_length.max(1) {
                continue;
            }
            if seen.insert(token) {
                out.push(token.to_string());
            }
        }
    }
    out
}

/// Full pipeline: visual rows (oldest first) -> logical lines -> capped ->
/// candidates (most recent first).
pub fn from_rows<'a, I>(rows: I, content_columns: usize, config: &Config) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let lines = crate::scrollback::logical_lines(rows, content_columns, config.rejoin_wrapped);
    let lines = crate::scrollback::cap_lines(lines, config.max_lines);
    extract(&lines, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn cfg(pairs: &[(&str, &str)]) -> Config {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_map(&map).0
    }

    #[test]
    fn splits_on_whitespace_and_default_separators() {
        let c = Config::default();
        let t: Vec<&str> = tokens("foo(bar) [baz]{qux}<a>\"b\" 'c',d;e\tf", &c).collect();
        assert_eq!(
            t,
            vec!["foo", "bar", "baz", "qux", "a", "b", "c", "d", "e", "f"]
        );
    }

    #[test]
    fn keeps_paths_flags_and_urls_intact() {
        let c = Config::default();
        let t: Vec<&str> =
            tokens("ls --color=auto /usr/local/bin https://x.io/a?b=1", &c).collect();
        assert_eq!(
            t,
            vec!["ls", "--color=auto", "/usr/local/bin", "https://x.io/a?b=1"]
        );
    }

    #[test]
    fn trims_trailing_chars_only() {
        let c = Config::default();
        assert_eq!(trim_token("file.txt.", &c), "file.txt");
        assert_eq!(trim_token("error::", &c), "error");
        assert_eq!(trim_token("a.:.:", &c), "a");
        assert_eq!(trim_token(".hidden", &c), ".hidden");
        assert_eq!(trim_token("...", &c), "");
    }

    #[test]
    fn most_recent_first_and_dedup_keeps_latest() {
        let c = Config::default();
        let l = lines(&["alpha beta", "gamma alpha", "delta"]);
        assert_eq!(extract(&l, &c), vec!["delta", "alpha", "gamma", "beta"]);
    }

    #[test]
    fn rightmost_token_in_line_is_most_recent() {
        let c = Config::default();
        assert_eq!(
            extract(&lines(&["one two three"]), &c),
            vec!["three", "two", "one"]
        );
    }

    #[test]
    fn min_length_applies_after_trimming() {
        let c = Config::default();
        let l = lines(&["ab ab. abc abc: a.b"]);
        assert_eq!(extract(&l, &c), vec!["a.b", "abc"]);
    }

    #[test]
    fn min_length_counts_chars_not_bytes() {
        let c = Config::default();
        assert_eq!(extract(&lines(&["éé ééé"]), &c), vec!["ééé"]);
    }

    #[test]
    fn dedup_after_trimming() {
        let c = Config::default();
        assert_eq!(extract(&lines(&["main.rs main.rs."]), &c), vec!["main.rs"]);
    }

    #[test]
    fn custom_config() {
        let c = cfg(&[
            ("separators", "="),
            ("trim_trailing", "!"),
            ("min_length", "1"),
        ]);
        let l = lines(&["x=(hello)!"]);
        assert_eq!(extract(&l, &c), vec!["(hello)", "x"]);
    }

    #[test]
    fn min_length_zero_still_drops_empty() {
        let c = cfg(&[("min_length", "0")]);
        assert_eq!(extract(&lines(&["a ... b"]), &c), vec!["b", "a"]);
    }

    #[test]
    fn prompt_line_is_not_excluded() {
        let c = Config::default();
        let l = lines(&["output word", "$ git chec"]);
        assert_eq!(extract(&l, &c)[0], "chec");
    }

    #[test]
    fn pipeline_rejoins_wrapped_words() {
        let c = Config::default();
        // width 10: "see /usr/l" is full and continues with "ocal/bin".
        let rows = ["see /usr/l", "ocal/bin", "done"];
        assert_eq!(
            from_rows(rows, 10, &c),
            vec!["done", "/usr/local/bin", "see"]
        );
    }

    #[test]
    fn pipeline_without_rejoin_splits_words() {
        let c = cfg(&[("rejoin_wrapped", "false")]);
        let rows = ["see /usr/l", "ocal/bin"];
        assert_eq!(from_rows(rows, 10, &c), vec!["ocal/bin", "/usr/l", "see"]);
    }

    #[test]
    fn pipeline_max_lines_counts_logical_lines() {
        let c = cfg(&[("max_lines", "1")]);
        let rows = ["oldword", "abcdefghij", "klm"];
        assert_eq!(from_rows(rows, 10, &c), vec!["abcdefghijklm"]);
    }

    #[test]
    fn empty_input() {
        let c = Config::default();
        assert!(extract(&[], &c).is_empty());
        assert!(extract(&lines(&["", "  ", "a b"]), &c).is_empty());
    }
}
