//! Splits pages into overlapping chunks (tech doc 4.1: ~500 tokens, 50 overlap).
//!
//! Tokens are estimated as characters / 4, which is close for English and good
//! enough for sizing; exact counts don't matter here.

use crate::parse::Page;

pub const TARGET_TOKENS: usize = 500;
pub const OVERLAP_TOKENS: usize = 50;
const CHARS_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub ord: usize,
    pub page: Option<u32>,
    pub text: String,
    pub token_count: usize,
}

#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(CHARS_PER_TOKEN)
}

/// Chunks every page in order. Empty pages produce nothing.
#[must_use]
pub fn chunk_pages(pages: &[Page]) -> Vec<Chunk> {
    let mut out = Vec::new();
    for page in pages {
        for text in split(
            &page.text,
            TARGET_TOKENS * CHARS_PER_TOKEN,
            OVERLAP_TOKENS * CHARS_PER_TOKEN,
        ) {
            out.push(Chunk {
                ord: out.len(),
                page: page.number,
                token_count: estimate_tokens(&text),
                text,
            });
        }
    }
    out
}

/// Word-boundary windows of about `size` chars, each starting `size - overlap` after the last.
fn split(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let mut end = start;
        let mut len = 0;
        while end < words.len() && (len == 0 || len + 1 + words[end].len() <= size) {
            len += words[end].len() + usize::from(len > 0);
            end += 1;
        }
        chunks.push(words[start..end].join(" "));
        if end >= words.len() {
            break;
        }
        // Step back to keep about `overlap` chars of context.
        let mut back = end;
        let mut kept = 0;
        while back > start + 1 && kept + words[back - 1].len() < overlap {
            kept += words[back - 1].len() + 1;
            back -= 1;
        }
        start = back.max(start + 1);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: Option<u32>, text: &str) -> Page {
        Page {
            number: n,
            text: text.into(),
        }
    }

    #[test]
    fn short_pages_become_one_chunk_each_with_their_page() {
        let chunks = chunk_pages(&[
            page(Some(1), "Rate limits."),
            page(Some(2), ""),
            page(Some(3), "Pricing  tiers\n\nand  more"),
        ]);
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            (chunks[0].page, chunks[0].ord, chunks[0].text.as_str()),
            (Some(1), 0, "Rate limits.")
        );
        assert_eq!(
            (chunks[1].page, chunks[1].ord, chunks[1].text.as_str()),
            (Some(3), 1, "Pricing tiers and more")
        );
    }

    #[test]
    fn long_pages_split_near_500_tokens_with_overlap() {
        let words: Vec<String> = (0..2000).map(|i| format!("word{i:04}")).collect(); // 9 chars + space each
        let chunks = chunk_pages(&[page(Some(7), &words.join(" "))]);
        assert!(chunks.len() >= 9, "{}", chunks.len());
        for c in &chunks[..chunks.len() - 1] {
            assert!((450..=500).contains(&c.token_count), "{}", c.token_count);
            assert_eq!(c.page, Some(7));
        }
        // Consecutive chunks share about 50 tokens (~200 chars) of text.
        let tail: Vec<&str> = chunks[0].text.split(' ').rev().take(5).collect();
        for w in tail {
            assert!(chunks[1].text.contains(w), "overlap missing {w}");
        }
        // Nothing is lost: the last word is in the last chunk.
        assert!(chunks.last().unwrap().text.ends_with("word1999"));
    }

    #[test]
    fn a_single_huge_word_still_makes_progress() {
        let chunks = chunk_pages(&[page(None, &"x".repeat(5000))]);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn estimates_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }
}
