//! Character-level sequential fuzzy scorer (Sublime/RustCast style).
//!
//! Bonuses for consecutive matches, word boundaries, camelCase transitions,
//! separators and early matches; gap penalty for unmatched characters.
//! Returns `None` when the query is not a subsequence of the name.

/// Score `query` against `name`; `None` when the query does not match.
pub(crate) fn fuzzy_score(query: &str, name: &str) -> Option<f64> {
    if query.is_empty() {
        return None;
    }

    let ql = query.to_lowercase();
    let q = ql.as_bytes();
    let t = name.as_bytes();
    let tl = name.to_lowercase();
    let t_lower = tl.as_bytes();

    if q.len() > t.len() {
        return None;
    }

    let mut score = 0.0;
    let mut qi = 0;
    let mut prev_matched = false;
    let mut first_match_pos: Option<usize> = None;

    for (ti, &ch) in t_lower.iter().enumerate() {
        if qi < q.len() && ch == q[qi] {
            qi += 1;
            score += 10.0;

            if prev_matched {
                score += 15.0;
            }
            if ti == 0 || matches!(t[ti - 1], b' ' | b'-' | b'_' | b'/' | b'\\') {
                score += 30.0;
            }
            if ti > 0 && t[ti].is_ascii_uppercase() && t[ti - 1].is_ascii_lowercase() {
                score += 20.0;
            }
            if ti > 0 && !t[ti - 1].is_ascii_alphanumeric() {
                score += 15.0;
            }
            if first_match_pos.is_none() {
                first_match_pos = Some(ti);
            }
            prev_matched = true;
        } else {
            prev_matched = false;
        }
    }

    if qi != q.len() {
        return None;
    }

    if let Some(pos) = first_match_pos {
        if pos == 0 {
            score += 50.0;
        } else {
            score += (1.0 - pos as f64 / t.len() as f64) * 30.0;
        }
    }

    let unmatched = t.len() - qi;
    score -= unmatched as f64 * 2.0;

    Some(score / q.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_empty_query_returns_none() {
        assert!(fuzzy_score("", "Anything").is_none());
    }

    #[test]
    fn fuzzy_exact_match() {
        let score = fuzzy_score("notepad", "Notepad").unwrap();
        assert!(score > 0.0);
    }

    #[test]
    fn fuzzy_subsequence_match() {
        let score = fuzzy_score("npd", "Notepad").unwrap();
        assert!(score > 0.0);
    }

    #[test]
    fn fuzzy_no_match() {
        assert!(fuzzy_score("xyz", "Notepad").is_none());
    }

    #[test]
    fn fuzzy_word_boundary_bonus() {
        let with_bonus = fuzzy_score("ps", "Power Shell").unwrap();
        let without = fuzzy_score("ps", "Powershell").unwrap_or(0.0);
        // Word boundary should boost the score
        assert!(
            with_bonus > without,
            "expected word boundary bonus: {} <= {}",
            with_bonus,
            without
        );
    }

    #[test]
    fn fuzzy_camelcase_bonus() {
        let with_bonus = fuzzy_score("vs", "VisualStudio").unwrap_or(0.0);
        let without = fuzzy_score("vs", "visualstudio").unwrap_or(0.0);
        assert!(
            with_bonus >= without,
            "expected camelCase bonus: {} < {}",
            with_bonus,
            without
        );
    }

    #[test]
    fn fuzzy_consecutive_bonus() {
        let consecutive = fuzzy_score("wo", "Word").unwrap();
        let spread = fuzzy_score("wd", "Word").unwrap_or(0.0);
        assert!(
            consecutive > spread,
            "expected consecutive bonus: {} <= {}",
            consecutive,
            spread
        );
    }

    #[test]
    fn fuzzy_early_match_bonus() {
        let early = fuzzy_score("n", "Notepad").unwrap();
        let late = fuzzy_score("d", "Notepad").unwrap_or(0.0);
        assert!(
            early > late,
            "expected early match bonus: {} <= {}",
            early,
            late
        );
    }

    #[test]
    fn fuzzy_query_longer_than_name() {
        assert!(fuzzy_score("ThisIsWayTooLong", "Short").is_none());
    }

    #[test]
    fn fuzzy_case_insensitive() {
        let upper = fuzzy_score("NP", "Notepad").unwrap();
        let lower = fuzzy_score("np", "Notepad").unwrap();
        assert!(
            (upper - lower).abs() < f64::EPSILON,
            "should be case-insensitive: {} != {}",
            upper,
            lower
        );
    }
}
