use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JointTradeRow {
    pub logical_mask: Vec<u64>,
}

/// Returns the lexicographically first pairwise-disjoint index tuple.
///
/// Each list must contain the complete row universe for one room under the
/// same response signature and must already be sorted by that room's final
/// comparator. The traversal changes search order only; it does not prune.
pub(crate) fn first_mask_disjoint_tuple(
    sorted_lists: &[Vec<JointTradeRow>],
    prefix_mask: &[u64],
) -> Option<Vec<usize>> {
    if sorted_lists.is_empty() {
        return Some(Vec::new());
    }
    if sorted_lists.iter().any(Vec::is_empty) {
        return None;
    }

    let initial = vec![0; sorted_lists.len()];
    let mut heap = BinaryHeap::from([Reverse(initial.clone())]);
    let mut visited = HashSet::from([initial]);

    while let Some(Reverse(indices)) = heap.pop() {
        if tuple_masks_are_disjoint(sorted_lists, &indices, prefix_mask) {
            return Some(indices);
        }
        for dimension in 0..indices.len() {
            let mut next = indices.clone();
            next[dimension] += 1;
            if next[dimension] < sorted_lists[dimension].len() && visited.insert(next.clone()) {
                heap.push(Reverse(next));
            }
        }
    }
    None
}

fn tuple_masks_are_disjoint(
    lists: &[Vec<JointTradeRow>],
    indices: &[usize],
    prefix_mask: &[u64],
) -> bool {
    let width = prefix_mask.len().max(
        lists
            .iter()
            .flat_map(|list| list.iter())
            .map(|row| row.logical_mask.len())
            .max()
            .unwrap_or(0),
    );
    let mut occupied = vec![0; width];
    occupied[..prefix_mask.len()].copy_from_slice(prefix_mask);
    for (list, index) in lists.iter().zip(indices) {
        let row = &list[*index];
        for (word, bits) in row.logical_mask.iter().copied().enumerate() {
            if occupied[word] & bits != 0 {
                return false;
            }
            occupied[word] |= bits;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(_stable_id: u64, bits: u64) -> JointTradeRow {
        JointTradeRow {
            logical_mask: vec![bits],
        }
    }

    fn brute_force_first(lists: &[Vec<JointTradeRow>], prefix_mask: &[u64]) -> Option<Vec<usize>> {
        fn visit(
            lists: &[Vec<JointTradeRow>],
            prefix_mask: &[u64],
            indices: &mut Vec<usize>,
        ) -> Option<Vec<usize>> {
            if indices.len() == lists.len() {
                return tuple_masks_are_disjoint(lists, indices, prefix_mask)
                    .then(|| indices.clone());
            }
            for index in 0..lists[indices.len()].len() {
                indices.push(index);
                if let Some(found) = visit(lists, prefix_mask, indices) {
                    return Some(found);
                }
                indices.pop();
            }
            None
        }
        visit(lists, prefix_mask, &mut Vec::new())
    }

    #[test]
    fn indexed_join_matches_brute_force_for_two_and_three_rooms() {
        let two_rooms = vec![
            vec![row(0, 0b0001), row(1, 0b0010), row(2, 0b0100)],
            vec![row(3, 0b0001), row(4, 0b1000)],
        ];
        assert_eq!(
            first_mask_disjoint_tuple(&two_rooms, &[]),
            brute_force_first(&two_rooms, &[])
        );

        let three_rooms = vec![
            vec![row(0, 0b0001), row(1, 0b0010)],
            vec![row(2, 0b0001), row(3, 0b0100)],
            vec![row(4, 0b0100), row(5, 0b1000)],
        ];
        assert_eq!(
            first_mask_disjoint_tuple(&three_rooms, &[0b0010]),
            brute_force_first(&three_rooms, &[0b0010])
        );
    }

    #[test]
    fn indexed_join_matches_brute_force_at_dimension_and_width_boundaries() {
        assert_eq!(
            first_mask_disjoint_tuple(&[], &[u64::MAX]),
            Some(Vec::new())
        );
        assert_eq!(first_mask_disjoint_tuple(&[Vec::new()], &[]), None);

        let one_room = vec![vec![
            JointTradeRow {
                logical_mask: vec![0, 0b1],
            },
            JointTradeRow {
                logical_mask: vec![0b1],
            },
        ]];
        assert_eq!(
            first_mask_disjoint_tuple(&one_room, &[0, 0b1]),
            brute_force_first(&one_room, &[0, 0b1])
        );

        let four_rooms = vec![
            vec![row(0, 0b0001), row(1, 0b0010)],
            vec![row(2, 0b0001), row(3, 0b0100)],
            vec![row(4, 0b0100), row(5, 0b1000)],
            vec![row(6, 0b1000), row(7, 0b1_0000)],
        ];
        assert_eq!(
            first_mask_disjoint_tuple(&four_rooms, &[]),
            brute_force_first(&four_rooms, &[])
        );
    }

    #[test]
    fn complete_lists_find_winner_that_local_top_one_loses() {
        let complete = vec![
            vec![row(0, 0b0001), row(1, 0b0010)],
            vec![row(2, 0b0011), row(3, 0b0100)],
        ];
        assert_eq!(first_mask_disjoint_tuple(&complete, &[]), Some(vec![0, 1]));

        let top_one = complete
            .iter()
            .map(|list| vec![list[0].clone()])
            .collect::<Vec<_>>();
        assert_eq!(first_mask_disjoint_tuple(&top_one, &[]), None);
    }

    #[test]
    fn prefix_mask_is_a_hard_constraint() {
        let lists = vec![vec![row(0, 0b0001), row(1, 0b0010)]];
        assert_eq!(first_mask_disjoint_tuple(&lists, &[0b0001]), Some(vec![1]));
    }
}
