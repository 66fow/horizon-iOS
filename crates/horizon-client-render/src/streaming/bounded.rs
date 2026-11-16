//! Bounded submission: the light and mesh stages hand the workers only the
//! nearest of what is ready.

/// The nearest `room` of the `candidates`, ordered: the light and mesh
/// stages submit bounded work, chosen from everything that is ready each
/// `update`, so what runs is always the nearest of what is left — chosen
/// again, from the current centre, as capacity frees, the way vanilla's
/// chunk task queues follow the current ticket levels. The selection is
/// O(n); the sort orders only what is submitted.
pub(super) fn nearest_first<T: Ord>(candidates: &mut Vec<T>, room: usize) {
    if candidates.len() > room {
        candidates.select_nth_unstable(room);
        candidates.truncate(room);
    }
    candidates.sort_unstable();
}

/// Budget sections, not columns: a column can contain hundreds of jobs.
/// Rebuilds get a small reserve when uploads of new terrain fill the queue.
/// A dirty section already in flight waits for that job instead of repeatedly
/// replacing it and retaining another snapshot on every frame.
pub(super) fn mesh_sections(
    candidates: &mut Vec<(i64, i32, i32, crate::section_mask::SectionMask)>,
    mut room: usize,
    mut rebuild_room: usize,
    in_flight: impl Fn((i32, i32, i32)) -> bool,
    compiled: impl Fn((i32, i32, i32)) -> bool,
) {
    use crate::section_mask::SectionMask;

    if room == 0 && rebuild_room == 0 {
        candidates.clear();
        return;
    }

    candidates.sort_unstable();
    // Admit rebuilds before first meshes, retaining distance order within each
    // group. Keep the chosen bits separate from the still-dirty remainder.
    for rebuild in [true, false] {
        for (_, x, z, sections) in candidates.iter_mut() {
            let mut selected = SectionMask::EMPTY;
            for sy in sections.iter() {
                let key = (*x, sy as i32, *z);
                if compiled(key) != rebuild {
                    selected.insert(sy);
                    continue;
                }
                if in_flight(key) {
                    continue;
                }
                if room > 0 {
                    room -= 1;
                } else if rebuild && rebuild_room > 0 {
                    rebuild_room -= 1;
                } else {
                    continue;
                }
                selected.insert(sy);
            }
            *sections = selected;
        }
    }
    candidates.retain(|(_, _, _, sections)| !sections.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_mask::SectionMask;

    #[test]
    fn mesh_capacity_counts_sections_including_a_partial_column() {
        let mut candidates = vec![
            (9, 3, 0, SectionMask::below(24)),
            (0, 0, 0, SectionMask::below(24)),
        ];
        mesh_sections(&mut candidates, 26, 0, |_| false, |_| false);
        assert_eq!(candidates[0].3, SectionMask::below(24));
        assert_eq!(candidates[1].3, SectionMask::below(2));
    }

    #[test]
    fn rebuilds_progress_with_upload_backlog_without_duplicate_jobs() {
        let mut candidates = vec![(0, 0, 0, SectionMask::below(24))];
        mesh_sections(&mut candidates, 0, 2, |k| k.1 == 4, |k| k.1 >= 4);
        assert_eq!(candidates[0].3.iter().collect::<Vec<_>>(), [5, 6]);
    }

    #[test]
    fn rebuilds_take_capacity_before_new_terrain_and_zero_room_stops() {
        let original = vec![(0, 0, 0, SectionMask::below(24))];
        let mut candidates = original.clone();
        mesh_sections(&mut candidates, 2, 0, |_| false, |k| k.1 == 12);
        assert_eq!(candidates[0].3.iter().collect::<Vec<_>>(), [0, 12]);
        let mut candidates = original;
        mesh_sections(&mut candidates, 0, 0, |_| false, |_| true);
        assert!(candidates.is_empty());
    }

    #[test]
    fn unscheduled_sections_survive_until_the_next_frame() {
        let original: SectionMask = [0, 1, 64, 130, 255].into_iter().collect();
        let mut candidates = vec![(0, 0, 0, original)];
        mesh_sections(&mut candidates, 2, 0, |k| k.1 == 1, |_| false);
        let remaining = original & !candidates[0].3;
        assert_eq!(remaining.iter().collect::<Vec<_>>(), [1, 130, 255]);
        let mut candidates = vec![(0, 0, 0, remaining)];
        mesh_sections(&mut candidates, 3, 0, |_| false, |_| false);
        assert_eq!(candidates[0].3, remaining);
    }

    /// The light and mesh stages submit bounded work, the nearest first:
    /// what exceeds the room stays dirty, to be chosen again — from the
    /// current centre — as capacity frees. Submitted whole, the queue once
    /// held every ready column's jobs and grew with the render distance
    /// (#3629); ties break by position, so the choice is deterministic.
    #[test]
    fn bounded_submission_takes_the_nearest_first() {
        let candidates: Vec<(i64, i32, i32, u32)> = (0..40i32)
            .map(|i| {
                let (x, z) = (i / 8 - 2, i % 8 - 3);
                (
                    i64::from((x - 3).pow(2) + (z - 2).pow(2)),
                    x,
                    z,
                    1 << (i % 24),
                )
            })
            .collect();
        let mut expected = candidates.clone();
        expected.sort_unstable();
        let mut none = candidates.clone();
        nearest_first(&mut none, 0);
        assert!(none.is_empty());
        let mut nearest = candidates;
        nearest_first(&mut nearest, 5);
        assert_eq!(nearest, expected[..5]);
        let mut past = expected.clone();
        nearest_first(&mut past, expected.len() + 10);
        assert_eq!(past, expected);
    }
}
