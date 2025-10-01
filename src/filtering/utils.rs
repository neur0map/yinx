// Utility functions for filtering operations
use std::collections::{HashMap, HashSet};

/// Calculate Shannon entropy of a string
/// Higher entropy = more random/information-dense
/// Returns value typically between 0.0 (all same character) and ~5.0 (uniform distribution)
pub fn shannon_entropy(s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }

    let mut freq: HashMap<char, u32> = HashMap::new();
    for c in s.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }

    let len = s.len() as f32;
    -freq
        .values()
        .map(|&count| {
            let p = count as f32 / len;
            p * p.log2()
        })
        .sum::<f32>()
}

/// Calculate change score between two strings
/// Returns 1.0 for completely different, 0.0 for identical
/// Uses character set overlap ratio (fast approximation, not Levenshtein)
pub fn change_score(line: &str, prev: &str) -> f32 {
    if line == prev {
        return 0.0;
    }

    // Character set based similarity (fast approximation)
    let chars1: HashSet<char> = line.chars().collect();
    let chars2: HashSet<char> = prev.chars().collect();

    let intersection = chars1.intersection(&chars2).count();
    let union = chars1.union(&chars2).count();

    if union == 0 {
        return 1.0;
    }

    1.0 - (intersection as f32 / union as f32)
}

/// Calculate percentile value from sorted or unsorted scores
/// p should be between 0.0 (min) and 1.0 (max)
/// Returns the value at the specified percentile
pub fn percentile(scores: &[f32], p: f32) -> f32 {
    if scores.is_empty() {
        return 0.0;
    }

    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let index = ((sorted.len() as f32) * p) as usize;
    let index = index.min(sorted.len() - 1);

    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy_uniform() {
        // Uniform distribution has high entropy
        let high = shannon_entropy("abcdefghijklmnop");
        // Repetitive string has low entropy
        let low = shannon_entropy("aaaaaaaaaaaaaaaa");

        assert!(high > low);
        assert!(high > 3.0); // Uniform should be around 4.0
        assert!(low < 1.0); // All same char should be 0.0
    }

    #[test]
    fn test_shannon_entropy_empty() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn test_change_score_identical() {
        assert_eq!(change_score("hello", "hello"), 0.0);
    }

    #[test]
    fn test_change_score_different() {
        let score = change_score("hello", "world");
        assert!(score > 0.5);
        assert!(score < 1.0);
    }

    #[test]
    fn test_change_score_completely_different() {
        let score = change_score("abc", "xyz");
        assert!(score > 0.9);
    }

    #[test]
    fn test_percentile_basic() {
        let scores = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(percentile(&scores, 0.0), 1.0); // Min
        assert_eq!(percentile(&scores, 0.5), 3.0); // Median
        assert_eq!(percentile(&scores, 1.0), 5.0); // Max
    }

    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[test]
    fn test_percentile_80th() {
        let scores = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p80 = percentile(&scores, 0.8);
        assert!((8.0..=9.0).contains(&p80));
    }
}
