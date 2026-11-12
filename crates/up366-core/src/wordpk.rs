//! Word-PK round selection.
//!
//! The client decides `score` and `total`; the server only sees the resulting
//! word list. Scoring is `score = rightWordList.length`, so the first `score`
//! drawn words are marked correct and the rest wrong.
//!
//! Selection lives here, not in the command layer, because it is pure: the
//! caller supplies the pool size and a seed, and the result is deterministic.
//! That keeps it unit-testable and makes a reprinted round reproducible.

use crate::error::{DomainError, Result};

/// A requested round: `score` correct words out of `total` drawn words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundRequest {
    pub score: usize,
    pub total: usize,
}

impl RoundRequest {
    /// Validate the request against the number of usable words in the bucket.
    pub fn validate(self, available: usize) -> Result<Self> {
        if self.total == 0 {
            return Err(DomainError::Invalid("total 必须大于 0".into()));
        }
        if self.score > self.total {
            return Err(DomainError::Invalid("分数不能大于 total".into()));
        }
        if self.total > available {
            return Err(DomainError::Invalid(format!(
                "词库只有 {available} 个可用单词，不足以抽取 {} 个",
                self.total
            )));
        }
        Ok(self)
    }
}

/// One splitmix64 step: a fast, dependency-free source of shuffle decisions.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fisher-Yates shuffle of `0..len`, driven by `seed`.
pub fn shuffled_indices(len: usize, seed: u64) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..len).collect();
    let mut state = seed;
    for index in (1..len).rev() {
        let pick = (splitmix64(&mut state) % (index as u64 + 1)) as usize;
        indices.swap(index, pick);
    }
    indices
}

/// Pick `total` distinct indices; the first `score` are the correct words.
///
/// The caller must have validated the request with [`RoundRequest::validate`].
pub fn draw(request: RoundRequest, available: usize, seed: u64) -> Vec<usize> {
    let mut indices = shuffled_indices(available, seed);
    indices.truncate(request.total);
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_is_deterministic_for_the_same_seed() {
        let request = RoundRequest { score: 2, total: 5 };
        let first = draw(request, 20, 42);
        let second = draw(request, 20, 42);
        assert_eq!(first, second);
        assert_eq!(first.len(), 5);
    }

    #[test]
    fn drawn_indices_are_distinct_and_in_range() {
        let request = RoundRequest { score: 3, total: 8 };
        let picked = draw(request, 9, 7);
        assert_eq!(picked.len(), 8);
        let mut sorted = picked.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8);
        assert!(picked.iter().all(|&index| index < 9));
    }

    #[test]
    fn different_seeds_select_different_rounds() {
        let request = RoundRequest { score: 1, total: 4 };
        assert_ne!(draw(request, 30, 1), draw(request, 30, 2));
    }

    #[test]
    fn the_whole_pool_is_returned_when_total_covers_it() {
        let request = RoundRequest { score: 0, total: 6 };
        let mut picked = draw(request, 6, 99);
        picked.sort_unstable();
        assert_eq!(picked, (0..6).collect::<Vec<_>>());
    }

    #[test]
    fn validation_rejects_impossible_rounds() {
        assert!(RoundRequest { score: 0, total: 0 }.validate(10).is_err());
        assert!(RoundRequest { score: 3, total: 2 }.validate(10).is_err());
        assert!(RoundRequest {
            score: 1,
            total: 11
        }
        .validate(10)
        .is_err());
        assert!(RoundRequest { score: 0, total: 1 }.validate(10).is_ok());
        // score == total means every drawn word is correct.
        assert!(RoundRequest { score: 4, total: 4 }.validate(4).is_ok());
    }
}
