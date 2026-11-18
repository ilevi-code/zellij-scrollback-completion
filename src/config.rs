//! Plugin configuration, parsed from the key/value pairs of the plugin block
//! in `config.kdl`.

use std::collections::BTreeMap;

pub const DEFAULT_SEPARATORS: &str = "()[]{}<>\"',;";
pub const DEFAULT_TRIM_TRAILING: &str = ".:";
pub const DEFAULT_MIN_LENGTH: usize = 3;
pub const DEFAULT_HEIGHT: usize = 10;
/// The popup needs at least one row.
pub const MIN_HEIGHT: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Extra token separators; whitespace always separates as well.
    pub separators: Vec<char>,
    /// Characters stripped from the end of every token.
    pub trim_trailing: Vec<char>,
    /// Tokens shorter than this (in chars, after trimming) are dropped.
    pub min_length: usize,
    /// Only the last `max_lines` logical lines are scanned; `None` = all.
    pub max_lines: Option<usize>,
    /// Maximum popup height in rows (one row per candidate).
    pub height: usize,
    /// Re-join soft-wrapped rows into logical lines.
    pub rejoin_wrapped: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            separators: DEFAULT_SEPARATORS.chars().collect(),
            trim_trailing: DEFAULT_TRIM_TRAILING.chars().collect(),
            min_length: DEFAULT_MIN_LENGTH,
            max_lines: None,
            height: DEFAULT_HEIGHT,
            rejoin_wrapped: true,
        }
    }
}

impl Config {
    /// Parses the plugin configuration. Unknown keys are ignored; invalid
    /// values fall back to the default and produce a warning (returned so the
    /// glue can log them).
    pub fn from_map(map: &BTreeMap<String, String>) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut warnings = Vec::new();

        if let Some(v) = map.get("separators") {
            config.separators = unique_chars(v);
        }
        if let Some(v) = map.get("trim_trailing") {
            config.trim_trailing = unique_chars(v);
        }
        if let Some(v) = map.get("min_length") {
            match v.trim().parse::<usize>() {
                Ok(n) => config.min_length = n,
                Err(_) => warnings.push(invalid("min_length", v)),
            }
        }
        if let Some(v) = map.get("max_lines") {
            match parse_max_lines(v) {
                Some(n) => config.max_lines = n,
                None => warnings.push(invalid("max_lines", v)),
            }
        }
        if let Some(v) = map.get("height") {
            match v.trim().parse::<usize>() {
                Ok(n) if n >= MIN_HEIGHT => config.height = n,
                _ => warnings.push(invalid("height", v)),
            }
        }
        if let Some(v) = map.get("rejoin_wrapped") {
            match parse_bool(v) {
                Some(b) => config.rejoin_wrapped = b,
                None => warnings.push(invalid("rejoin_wrapped", v)),
            }
        }
        (config, warnings)
    }

    pub fn is_separator(&self, c: char) -> bool {
        c.is_whitespace() || self.separators.contains(&c)
    }
}

fn invalid(key: &str, value: &str) -> String {
    format!("invalid value {value:?} for `{key}`, using the default")
}

fn unique_chars(s: &str) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    for c in s.chars() {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// `Some(None)` = unlimited, `Some(Some(n))` = cap, `None` = invalid.
fn parse_max_lines(v: &str) -> Option<Option<usize>> {
    let v = v.trim();
    match v.to_ascii_lowercase().as_str() {
        "" | "unlimited" | "none" | "0" => Some(None),
        _ => v.parse::<usize>().ok().map(Some),
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn defaults_match_roadmap() {
        let (c, w) = Config::from_map(&BTreeMap::new());
        assert!(w.is_empty());
        assert_eq!(c.separators.iter().collect::<String>(), "()[]{}<>\"',;");
        assert_eq!(c.trim_trailing, vec!['.', ':']);
        assert_eq!(c.min_length, 3);
        assert_eq!(c.max_lines, None);
        assert_eq!(c.height, 10);
        assert!(c.rejoin_wrapped);
    }

    #[test]
    fn parses_all_keys() {
        let (c, w) = Config::from_map(&map(&[
            ("separators", "|="),
            ("trim_trailing", ".,!"),
            ("min_length", "5"),
            ("max_lines", "2000"),
            ("height", "6"),
            ("rejoin_wrapped", "false"),
        ]));
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.separators, vec!['|', '=']);
        assert_eq!(c.trim_trailing, vec!['.', ',', '!']);
        assert_eq!(c.min_length, 5);
        assert_eq!(c.max_lines, Some(2000));
        assert_eq!(c.height, 6);
        assert!(!c.rejoin_wrapped);
    }

    #[test]
    fn empty_sets_are_allowed() {
        let (c, w) = Config::from_map(&map(&[("separators", ""), ("trim_trailing", "")]));
        assert!(w.is_empty());
        assert!(c.separators.is_empty());
        assert!(c.trim_trailing.is_empty());
        assert!(c.is_separator(' '));
        assert!(c.is_separator('\t'));
        assert!(!c.is_separator('('));
    }

    #[test]
    fn duplicate_chars_are_collapsed() {
        let (c, _) = Config::from_map(&map(&[("separators", "((,,")]));
        assert_eq!(c.separators, vec!['(', ',']);
    }

    #[test]
    fn max_lines_unlimited_spellings() {
        for v in ["unlimited", "none", "0", "", " Unlimited "] {
            let (c, w) = Config::from_map(&map(&[("max_lines", v)]));
            assert!(w.is_empty(), "{v:?}");
            assert_eq!(c.max_lines, None, "{v:?}");
        }
    }

    #[test]
    fn bool_spellings() {
        for (v, expected) in [
            ("true", true),
            ("YES", true),
            ("1", true),
            ("off", false),
            ("0", false),
        ] {
            let (c, w) = Config::from_map(&map(&[("rejoin_wrapped", v)]));
            assert!(w.is_empty());
            assert_eq!(c.rejoin_wrapped, expected, "{v:?}");
        }
    }

    #[test]
    fn invalid_values_fall_back_with_warnings() {
        let (c, w) = Config::from_map(&map(&[
            ("min_length", "-1"),
            ("max_lines", "lots"),
            ("height", "0"),
            ("rejoin_wrapped", "maybe"),
        ]));
        assert_eq!(w.len(), 4, "{w:?}");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (c, w) = Config::from_map(&map(&[("whatever", "x")]));
        assert!(w.is_empty());
        assert_eq!(c, Config::default());
    }

    #[test]
    fn default_separators_and_whitespace() {
        let c = Config::default();
        for ch in "()[]{}<>\"',; \t".chars() {
            assert!(c.is_separator(ch), "{ch:?}");
        }
        for ch in "abc-_/.:=|$~".chars() {
            assert!(!c.is_separator(ch), "{ch:?}");
        }
    }
}
