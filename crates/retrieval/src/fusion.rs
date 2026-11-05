//! Reciprocal rank fusion: score = Σ 1 / (K + rank). Robust to the two lists
//! having scores on different scales, which vector distance and BM25 do.

use std::collections::HashMap;

pub const K: f64 = 60.0;

/// Fuses ranked id lists (best first) into one, best first. Ties keep the
/// order of first appearance, so results are deterministic.
#[must_use]
pub fn fuse(lists: &[Vec<i64>]) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, (f64, usize)> = HashMap::new();
    let mut seen = 0;
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let s = 1.0 / (K + rank as f64 + 1.0);
            let entry = scores.entry(*id).or_insert_with(|| {
                seen += 1;
                (0.0, seen)
            });
            entry.0 += s;
        }
    }
    let mut out: Vec<(i64, f64, usize)> = scores
        .into_iter()
        .map(|(id, (s, order))| (id, s, order))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
    out.into_iter().map(|(id, s, _)| (id, s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_in_both_lists_rank_first() {
        let fused = fuse(&[vec![1, 2, 3], vec![3, 4, 1]]);
        let ids: Vec<i64> = fused.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids[..2], [1, 3], "{fused:?}");
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn one_list_keeps_its_order() {
        let ids: Vec<i64> = fuse(&[vec![9, 7, 8]])
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [9, 7, 8]);
    }

    #[test]
    fn empty_input() {
        assert!(fuse(&[vec![], vec![]]).is_empty());
    }
}
