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

    /// Uniform index in `0..bound` by Lemire's multiply-shift, which avoids a slow modulo.
    fn below(&mut self, bound: usize) -> usize {
        ((u128::from(self.next_u64()) * bound as u128) >> 64) as usize
    }
}

/// Generates the words for a test.
///
/// The same `seed` and `count` always produce the same words. No word appears twice in a row.
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
    let mut previous = rng.below(list.len());
    for _ in 0..count {
        words.push(list[previous]);
        // Draw from the other `len - 1` words and skip over the previous index, so a repeat is
        // impossible without a retry loop.
        let next = rng.below(list.len() - 1);
        previous = next + usize::from(next >= previous);
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
    fn same_seed_gives_same_words() {
        assert_eq!(generate(7, 50), generate(7, 50));
    }

    #[test]
    fn different_seeds_give_different_words() {
        assert_ne!(generate(7, 50), generate(8, 50));
    }

    #[test]
    fn sequence_is_pinned_across_releases() {
        assert_eq!(generate(0, 5), ["word", "look", "to", "early", "we"]);
    }

    #[test]
    fn never_repeats_a_word_back_to_back() {
        let words = generate(123, 10_000);
        assert!(words.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn returns_requested_count() {
        assert_eq!(generate(1, 0).len(), 0);
        assert_eq!(generate(1, 350).len(), 350);
    }
}
