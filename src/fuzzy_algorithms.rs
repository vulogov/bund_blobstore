use std::cmp;
use std::collections::HashSet;

/// Jaro-Winkler similarity algorithm
pub struct JaroWinkler {
    prefix_scale: f64,
    prefix_length: usize,
}

impl Default for JaroWinkler {
    fn default() -> Self {
        Self {
            prefix_scale: 0.1,
            prefix_length: 4,
        }
    }
}

impl JaroWinkler {
    pub fn new(prefix_scale: f64, prefix_length: usize) -> Self {
        Self {
            prefix_scale,
            prefix_length,
        }
    }

    pub fn similarity(&self, s1: &str, s2: &str) -> f64 {
        let jaro = self.jaro_similarity(s1, s2);
        let prefix = self.common_prefix_length(s1, s2);
        let p = prefix.min(self.prefix_length) as f64;

        jaro + p * self.prefix_scale * (1.0 - jaro)
    }

    fn jaro_similarity(&self, s1: &str, s2: &str) -> f64 {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let len1 = s1_chars.len();
        let len2 = s2_chars.len();

        if len1 == 0 && len2 == 0 {
            return 1.0;
        }
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        let match_distance = cmp::max(len1, len2) / 2 - 1;

        let mut s1_matches = vec![false; len1];
        let mut s2_matches = vec![false; len2];
        let mut matches = 0;

        for i in 0..len1 {
            let start = if i > match_distance {
                i - match_distance
            } else {
                0
            };
            let end = cmp::min(i + match_distance + 1, len2);

            for j in start..end {
                if !s2_matches[j] && s1_chars[i] == s2_chars[j] {
                    s1_matches[i] = true;
                    s2_matches[j] = true;
                    matches += 1;
                    break;
                }
            }
        }

        if matches == 0 {
            return 0.0;
        }

        let mut transpositions = 0;
        let mut k = 0;
        for i in 0..len1 {
            if s1_matches[i] {
                while !s2_matches[k] {
                    k += 1;
                }
                if s1_chars[i] != s2_chars[k] {
                    transpositions += 1;
                }
                k += 1;
            }
        }

        (matches as f64 / len1 as f64
            + matches as f64 / len2 as f64
            + (matches as f64 - transpositions as f64 / 2.0) / matches as f64)
            / 3.0
    }

    fn common_prefix_length(&self, s1: &str, s2: &str) -> usize {
        s1.chars()
            .zip(s2.chars())
            .take_while(|(c1, c2)| c1 == c2)
            .count()
    }
}

/// Sørensen-Dice coefficient for string similarity
pub struct SorensenDice;

impl SorensenDice {
    pub fn similarity(s1: &str, s2: &str) -> f64 {
        let bigrams1 = Self::get_bigrams(s1);
        let bigrams2 = Self::get_bigrams(s2);

        let intersection: HashSet<_> = bigrams1.intersection(&bigrams2).collect();

        if bigrams1.is_empty() && bigrams2.is_empty() {
            return 1.0;
        }

        (2.0 * intersection.len() as f64) / (bigrams1.len() + bigrams2.len()) as f64
    }

    fn get_bigrams(s: &str) -> HashSet<String> {
        let chars: Vec<char> = s.chars().collect();
        let mut bigrams = HashSet::new();

        for i in 0..chars.len().saturating_sub(1) {
            bigrams.insert(format!("{}{}", chars[i], chars[i + 1]));
        }

        bigrams
    }
}

/// Combined fuzzy search result with multiple algorithms
#[derive(Debug, Clone)]
pub struct AdvancedFuzzyResult {
    pub key: String,
    pub term: String,
    pub levenshtein_distance: usize,
    pub jaro_winkler_score: f64,
    pub sorensen_dice_score: f64,
    pub combined_score: f64,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
}

impl AdvancedFuzzyResult {
    pub fn calculate_combined_score(&mut self) {
        self.combined_score = (self.jaro_winkler_score + self.sorensen_dice_score) / 2.0;
    }
}
