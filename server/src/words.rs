//! Deterministic test text generation.
//!
//! The text is a pure function of a 64-bit seed. The server never stores the text it hands
//! out; it regenerates it from the seed inside the signed token when a result comes back.
//! The generator is implemented here, not taken from `rand`, so the sequence for a seed can
//! never change under a dependency upgrade while tokens are in flight.

use std::sync::LazyLock;

/// The embedded word list, one lowercase ASCII word per line.
///
/// `LazyLock` is like a module-level constant computed on first access, similar to a cached
/// global in Python. It splits the embedded file once and reuses the slice for every request.
static ENGLISH: LazyLock<Box<[&'static str]>> =
    LazyLock::new(|| include_str!("../words/english.txt").lines().collect());

/// SplitMix64: a tiny, fast, statistically solid PRNG with a fixed published algorithm.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Index in `0..bound` by Lemire's multiply-shift, which avoids a slow modulo. Without a
    /// rejection step it is uniform up to one draw in 2^64: each index is hit by
    /// `floor(2^64 / bound)` or `ceil(2^64 / bound)` of the draws.
    fn below(&mut self, bound: usize) -> usize {
        ((u128::from(self.next_u64()) * bound as u128) >> 64) as usize
    }

    /// Index in `0..bound` that is none of the `excluded` indices, as uniform as [`Self::below`].
    ///
    /// It draws from the allowed indices only, then shifts the draw past each excluded index
    /// in ascending order, so no retry loop is needed. The `Some` entries must be distinct and
    /// below `bound`, and fewer than `bound`, so some index is left. `None` entries exclude
    /// nothing.
    fn below_excluding(&mut self, bound: usize, mut excluded: [Option<usize>; 2]) -> usize {
        // `Option` orders `None` before every `Some`, so sorting puts the indices to skip last,
        // in ascending order. `.flatten()` then iterates over only the `Some` values.
        excluded.sort_unstable();
        let allowed = bound - excluded.iter().flatten().count();
        excluded
            .iter()
            .flatten()
            .fold(self.below(allowed), |index, &skip| {
                index + usize::from(index >= skip)
            })
    }
}

/// Generates the words for a test.
///
/// The same `seed` and `count` always produce the same words. A word never repeats either of
/// the two words before it, so the text has no "the the" or "of the of" stutters.
///
/// # Arguments
///
/// * `seed` - Seed taken from the test token
/// * `count` - Number of words to produce
///
/// # Returns
///
/// Borrowed slices of the embedded word list, so no word is allocated.
///
/// # Examples
///
/// ```ignore
/// let words = generate(42, 10);
/// assert_eq!(words, generate(42, 10));
/// ```
pub fn generate(seed: u64, count: usize) -> Vec<&'static str> {
    let list = &**ENGLISH;
    let mut rng = SplitMix64(seed);
    let mut words = Vec::with_capacity(count);
    // List indices of the last word and the one before it; `None` until they exist.
    let mut recent: [Option<usize>; 2] = [None, None];
    for _ in 0..count {
        let index = rng.below_excluding(list.len(), recent);
        words.push(list[index]);
        recent = [recent[1], Some(index)];
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_list_is_lowercase_ascii_and_unique() {
        let mut sorted = ENGLISH.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 200);
        assert!(
            ENGLISH
                .iter()
                .all(|word| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase()))
        );
    }

    #[test]
    fn word_list_holds_only_short_words() {
        assert!(ENGLISH.iter().all(|word| word.len() <= 8));
    }

    #[test]
    fn same_seed_gives_same_words() {
        assert_eq!(generate(7, 50), generate(7, 50));
    }

    #[test]
    fn different_seeds_give_different_words() {
        assert_ne!(generate(7, 50), generate(8, 50));
    }

    #[test]
    fn sequence_is_pinned_across_releases() {
        assert_eq!(generate(0, 5), ["without", "any", "in", "second", "by"]);
    }

    #[test]
    fn never_repeats_either_of_the_two_previous_words() {
        let words = generate(123, 10_000);
        assert!(
            words
                .windows(3)
                .all(|run| run[0] != run[1] && run[0] != run[2] && run[1] != run[2])
        );
    }

    #[test]
    fn uses_every_word_in_the_list() {
        let mut seen = generate(5, 10_000);
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), ENGLISH.len());
    }

    #[test]
    fn below_excluding_skips_excluded_indices_uniformly() {
        let mut rng = SplitMix64(9);
        let mut hits = [0_u32; 5];
        for _ in 0..30_000 {
            hits[rng.below_excluding(5, [Some(3), Some(1)])] += 1;
        }
        assert_eq!((hits[1], hits[3]), (0, 0));
        // Each of the three allowed indices expects 10,000 hits.
        assert!(
            [hits[0], hits[2], hits[4]]
                .iter()
                .all(|&count| (9_500..10_500).contains(&count))
        );
    }

    #[test]
    fn returns_requested_count() {
        assert_eq!(generate(1, 0).len(), 0);
        assert_eq!(generate(1, 350).len(), 350);
    }
}
