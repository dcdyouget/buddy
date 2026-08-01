use super::aggregate::SearchHit;
use std::collections::HashSet;

const COMMON_TERMS: &[&str] = &[
    "a", "an", "and", "are", "current", "for", "how", "in", "is", "latest", "of", "the", "to",
    "what", "who", "与", "了", "什么", "以及", "及", "和", "如何", "当前", "怎么", "是", "最新",
    "的", "谁",
];

pub(super) fn filter_relevant_hits(
    query: &str,
    hits: Vec<SearchHit>,
    limit: usize,
) -> Vec<SearchHit> {
    hits.into_iter()
        .filter(|hit| is_relevant(query, hit))
        .take(limit)
        .collect()
}

fn is_relevant(query: &str, hit: &SearchHit) -> bool {
    let candidate = format!("{} {} {}", hit.title, hit.snippet, hit.url);
    let query_compact = compact(query);
    let candidate_compact = compact(&candidate);
    if query_compact.chars().count() >= 2 && candidate_compact.contains(&query_compact) {
        return true;
    }

    let candidate_tokens = tokens(&candidate).into_iter().collect::<HashSet<_>>();
    let candidate_numbers = numeric_terms(&candidate)
        .into_iter()
        .collect::<HashSet<_>>();
    if !numeric_terms(query)
        .iter()
        .all(|number| candidate_numbers.contains(number))
    {
        return false;
    }

    let terms = meaningful_terms(query);
    if terms.is_empty() {
        return query_compact.chars().count() >= 2 && candidate_compact.contains(&query_compact);
    }

    let matched = terms
        .iter()
        .filter(|term| term_matches(term, &candidate_tokens, &candidate_compact))
        .count();
    let required = (terms.len() * 3).div_ceil(5);
    matched >= required.max(1)
}

fn meaningful_terms(value: &str) -> Vec<String> {
    tokens(value)
        .into_iter()
        .filter(|term| !term.chars().all(|character| character.is_ascii_digit()))
        .filter(|term| term.chars().count() >= 2)
        .filter(|term| !COMMON_TERMS.contains(&term.as_str()))
        .collect()
}

fn numeric_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }
    terms
}

fn term_matches(term: &str, candidate_tokens: &HashSet<String>, candidate_compact: &str) -> bool {
    if candidate_tokens.contains(term) {
        return true;
    }
    if contains_cjk(term) {
        return cjk_bigram_coverage(term, candidate_compact) >= 0.4;
    }
    if term.chars().any(|character| character.is_ascii_digit())
        && term
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
        return candidate_compact.contains(term);
    }
    false
}

fn cjk_bigram_coverage(term: &str, candidate: &str) -> f32 {
    let characters = term
        .chars()
        .filter(|character| is_cjk(*character))
        .collect::<Vec<_>>();
    if characters.len() < 2 {
        return 0.0;
    }
    let bigrams = characters
        .windows(2)
        .map(|pair| pair.iter().collect::<String>())
        .collect::<HashSet<_>>();
    let matched = bigrams
        .iter()
        .filter(|bigram| candidate.contains(bigram.as_str()))
        .count();
    matched as f32 / bigrams.len() as f32
}

fn tokens(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect()
}

fn compact(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(is_cjk)
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{3040}'..='\u{30ff}'
            | '\u{ac00}'..='\u{d7af}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(title: &str, snippet: &str) -> SearchHit {
        SearchHit {
            source: "test",
            title: title.to_string(),
            url: "https://example.com/result".to_string(),
            snippet: snippet.to_string(),
        }
    }

    #[test]
    fn removes_results_unrelated_to_query() {
        let hits = vec![
            hit("Rust Programming Language", "Official Rust documentation"),
            hit("今日足球赛果", "联赛排名与比赛集锦"),
        ];

        let filtered = filter_relevant_hits("Rust programming language", hits, 5);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].title.contains("Rust"));
    }

    #[test]
    fn keeps_cjk_partial_matches_but_requires_requested_year() {
        let hits = vec![
            hit("2026年夏季新番表", "7月动画作品一览"),
            hit("2025年夏季新番表", "本季动画作品一览"),
            hit("2026 世界杯赛程", "夏季体育赛事"),
        ];

        let filtered = filter_relevant_hits("2026 夏季新番", hits, 5);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].title.contains("2026"));
        assert!(filtered[0].title.contains("新番"));
    }

    #[test]
    fn matches_natural_language_chinese_query_by_bigrams() {
        let hits = vec![hit("法国总统埃马纽埃尔·马克龙", "法国现任国家元首资料")];

        assert_eq!(filter_relevant_hits("法国现任总统是谁", hits, 5).len(), 1);
    }
}
