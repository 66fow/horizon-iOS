//! Admission budget for mesh jobs, including the storage still waiting for upload.
#[derive(Clone, Copy)]
pub struct MeshBudget {
    pub sections: usize,
    pub bytes: usize,
    pub pending_bytes: usize,
}

impl MeshBudget {
    /// Reserve a conservative observed mesh size for each unfinished job. This
    /// is backpressure, not a hard allocator limit: a new section can be larger
    /// than any seen before. Already running work is allowed to finish.
    pub(super) fn rooms(
        self,
        occupied: usize,
        in_flight: usize,
        ready_bytes: usize,
        estimate: usize,
        limits: (usize, usize),
    ) -> (usize, usize) {
        let (capacity, reserve) = limits;
        let retained = self.pending_bytes.saturating_add(ready_bytes);
        // Even a formerly oversized section must not prevent future work
        // once the queue drains. One job can exceed the estimate.
        let estimate = estimate.clamp(1, self.bytes.max(1));
        let memory_room =
            (self.bytes.saturating_sub(retained) / estimate).saturating_sub(in_flight);
        let total = self.sections.saturating_sub(occupied).min(memory_room);
        let normal = capacity.saturating_sub(occupied).min(total);
        let rebuild = reserve
            .saturating_sub(occupied.saturating_sub(capacity))
            .min(total.saturating_sub(normal));
        (normal, rebuild)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIB: usize = 1024 * 1024;

    #[test]
    fn dense_meshes_and_running_jobs_reduce_admission() {
        let budget = MeshBudget {
            sections: 24,
            bytes: 16 * MIB,
            pending_bytes: 8 * MIB,
        };
        assert_eq!(budget.rooms(6, 2, 0, 2 * MIB, (128, 16)), (2, 0));
        assert_eq!(budget.rooms(6, 2, 4 * MIB, 2 * MIB, (128, 16)), (0, 0));
    }

    #[test]
    fn oversized_history_still_allows_one_job_after_draining() {
        let budget = MeshBudget {
            sections: 8,
            bytes: 4 * MIB,
            pending_bytes: 0,
        };
        assert_eq!(budget.rooms(0, 0, 0, 32 * MIB, (128, 16)), (1, 0));
        assert_eq!(budget.rooms(1, 1, 0, 32 * MIB, (128, 16)), (0, 0));
        assert_eq!(budget.rooms(1, 0, 8 * MIB, 32 * MIB, (128, 16)), (0, 0));
    }

    #[test]
    fn rebuild_reserve_cannot_bypass_memory_or_section_limit() {
        let budget = MeshBudget {
            sections: 8,
            bytes: 16 * MIB,
            pending_bytes: 0,
        };
        assert_eq!(budget.rooms(6, 0, 0, MIB, (4, 16)), (0, 2));
        assert_eq!(budget.rooms(8, 0, 0, MIB, (4, 16)), (0, 0));
        assert_eq!(budget.rooms(6, 0, 16 * MIB, MIB, (4, 16)), (0, 0));
        assert_eq!(budget.rooms(6, 0, 0, MIB, (4, 16)), (0, 2));
    }
}
