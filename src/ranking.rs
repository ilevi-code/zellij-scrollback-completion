//! Filtering and ordering candidates against the typed filter.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str};

/// One visible entry of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Index into the candidate list (which is ordered by recency, so a lower
    /// index means more recent).
    pub index: usize,
    pub score: u32,
    /// Char indices of the matched characters in the candidate, sorted.
    pub positions: Vec<u32>,
}

pub struct Ranker {
    matcher: Matcher,
}

impl Default for Ranker {
    fn default() -> Self {
        Ranker {
            matcher: Matcher::new(MatcherConfig::DEFAULT),
        }
    }
}

impl Ranker {
    /// With an empty filter, every candidate is returned in recency order.
    /// Otherwise only fuzzy matches are returned, best score first, ties
    /// broken by recency. The filter is matched literally (no `^`/`$`/`!`
    /// operators); smart case: an uppercase letter in the filter makes the
    /// match case-sensitive.
    pub fn rank(&mut self, candidates: &[String], filter: &str) -> Vec<Match> {
        if filter.is_empty() {
            return (0..candidates.len())
                .map(|index| Match {
                    index,
                    score: 0,
                    positions: Vec::new(),
                })
                .collect();
        }
        let pattern = Pattern::new(
            filter,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut buf = Vec::new();
        let mut matches: Vec<Match> = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let mut positions = Vec::new();
                let score = pattern.indices(
                    Utf32Str::new(candidate, &mut buf),
                    &mut self.matcher,
                    &mut positions,
                )?;
                positions.sort_unstable();
                positions.dedup();
                Some(Match {
                    index,
                    score,
                    positions,
                })
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn words(cands: &[String], ms: &[Match]) -> Vec<String> {
        ms.iter().map(|m| cands[m.index].clone()).collect()
    }

    #[test]
    fn empty_filter_keeps_recency_order() {
        let cands = c(&["newest", "middle", "oldest"]);
        let ms = Ranker::default().rank(&cands, "");
        assert_eq!(words(&cands, &ms), cands);
        assert!(ms.iter().all(|m| m.positions.is_empty()));
    }

    #[test]
    fn filters_out_non_matches() {
        let cands = c(&["apple", "banana", "cherry"]);
        let ms = Ranker::default().rank(&cands, "an");
        assert_eq!(words(&cands, &ms), c(&["banana"]));
    }

    #[test]
    fn fuzzy_subsequence_matches() {
        let cands = c(&["zellij-scrollback", "other"]);
        let ms = Ranker::default().rank(&cands, "zsb");
        assert_eq!(words(&cands, &ms), c(&["zellij-scrollback"]));
    }

    #[test]
    fn better_score_wins_over_recency() {
        // "config" (prefix, contiguous) should beat the more recent scattered match.
        let cands = c(&["cxoxnxfxixg", "config"]);
        let ms = Ranker::default().rank(&cands, "config");
        assert_eq!(words(&cands, &ms), c(&["config", "cxoxnxfxixg"]));
    }

    #[test]
    fn ties_broken_by_recency() {
        let cands = c(&["foo1", "foo2", "foo3"]);
        let ms = Ranker::default().rank(&cands, "foo");
        assert_eq!(
            ms.iter().map(|m| m.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn positions_are_char_indices() {
        let cands = c(&["éabc"]);
        let ms = Ranker::default().rank(&cands, "bc");
        assert_eq!(ms[0].positions, vec![2, 3]);
    }

    #[test]
    fn smart_case() {
        let cands = c(&["README", "readme"]);
        let ms = Ranker::default().rank(&cands, "readme");
        assert_eq!(ms.len(), 2);
        let ms = Ranker::default().rank(&cands, "READ");
        assert_eq!(words(&cands, &ms), c(&["README"]));
    }

    #[test]
    fn special_chars_are_literal() {
        let cands = c(&["$HOME", "HOME", "!bang", "^caret"]);
        let mut r = Ranker::default();
        assert_eq!(words(&cands, &r.rank(&cands, "$HO")), c(&["$HOME"]));
        assert_eq!(words(&cands, &r.rank(&cands, "!ba")), c(&["!bang"]));
        assert_eq!(words(&cands, &r.rank(&cands, "^ca")), c(&["^caret"]));
    }

    #[test]
    fn no_candidates() {
        assert!(Ranker::default().rank(&[], "x").is_empty());
        assert!(Ranker::default().rank(&[], "").is_empty());
    }
}
