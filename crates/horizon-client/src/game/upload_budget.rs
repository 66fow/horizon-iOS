use std::time::Duration;

pub(super) fn allows(
    count: usize,
    bytes: u64,
    next_bytes: u64,
    limit: usize,
    byte_limit: u64,
    time_limit: Option<Duration>,
    elapsed: Duration,
) -> bool {
    if count == 0 {
        // A single oversized section must not block the queue forever.
        return true;
    }
    (limit == 0 || count < limit)
        && (byte_limit == 0 || bytes.saturating_add(next_bytes) <= byte_limit)
        && time_limit.is_none_or(|limit| elapsed < limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn oversized_first_section_always_makes_progress() {
        assert!(allows(0, 0, u64::MAX, 1, 1024, Some(MS), MS));
    }

    #[test]
    fn even_a_small_batch_stops_at_the_deadline() {
        assert!(allows(1, 32, 32, 0, 1024, Some(MS), Duration::ZERO));
        assert!(!allows(1, 32, 32, 0, 1024, Some(MS), MS));
    }

    #[test]
    fn empty_meshes_do_not_bypass_the_byte_limit() {
        assert!(!allows(1, 0, 2048, 0, 1024, None, Duration::ZERO));
        assert!(allows(1, 0, 1024, 0, 1024, None, Duration::ZERO));
    }

    #[test]
    fn count_and_byte_limits_remain_independent() {
        assert!(!allows(2, 32, 32, 2, 1024, None, Duration::ZERO));
        assert!(!allows(1, 768, 512, 8, 1024, None, Duration::ZERO));
        assert!(allows(1, 512, 512, 8, 1024, None, Duration::ZERO));
    }

    #[test]
    fn zero_limits_still_mean_unlimited() {
        assert!(allows(999, 4096, 4096, 0, 0, None, MS));
    }
}
