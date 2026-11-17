//! Feedback for optional terrain work on memory-constrained devices. No GPU waits or
//! allocations: presentation cadence and already available GPU timestamps are enough.
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(super) struct Budget {
    pub upload_bytes: u64,
    pub upload_time: Duration,
    pub mesh_sections: usize,
    pub mesh_bytes: usize,
}

pub(super) struct Governor {
    level: usize,
    smooth: f64,
    stable: Duration,
    memory_hold: Duration,
    target: Duration,
}

impl Default for Governor {
    fn default() -> Self {
        Self {
            level: 1,
            smooth: 1.0,
            stable: Duration::ZERO,
            memory_hold: Duration::ZERO,
            target: Duration::ZERO,
        }
    }
}

impl Governor {
    pub fn memory_warning(&mut self) {
        self.level = 0;
        self.stable = Duration::ZERO;
        self.memory_hold = Duration::from_secs(5);
    }

    /// `cadence` includes normal pacing; only a missed slot is overload. `work`
    /// excludes presentation waits and the FPS limiter. GPU timing is optional
    /// and asynchronous; never request a readback to drive this controller.
    pub fn observe(
        &mut self,
        cadence: Duration,
        work: Duration,
        gpu_ms: Option<f32>,
        target: Duration,
    ) {
        if target.is_zero() {
            return;
        }
        if self.target != target || cadence > Duration::from_millis(250) {
            // Resume/teleport/changed FPS cap: discard stale timing, keeping
            // any memory warning's hold until actual playing time passes.
            self.target = target;
            self.smooth = 1.0;
            self.stable = Duration::ZERO;
            return;
        }
        self.memory_hold = self.memory_hold.saturating_sub(cadence);
        let target_ms = target.as_secs_f64() * 1000.0;
        let gpu = gpu_ms.filter(|ms| ms.is_finite() && *ms >= 0.0);
        let busy = (work.as_secs_f64() * 1000.0).max(f64::from(gpu.unwrap_or(0.0)));
        let ratio = (cadence.as_secs_f64() / target.as_secs_f64()).max(busy / target_ms);
        self.smooth += (ratio.min(4.0) - self.smooth) * 0.125;
        if !self.memory_hold.is_zero() {
            self.level = 0;
            self.stable = Duration::ZERO;
        } else if ratio > 1.35 || self.smooth > 1.10 {
            self.level = self.level.saturating_sub(1);
            self.stable = Duration::ZERO;
        } else if self.smooth < 1.05 && busy < target_ms * 0.80 {
            self.stable += cadence;
            if self.stable >= Duration::from_secs(2) {
                self.level = (self.level + 1).min(3);
                self.stable = Duration::ZERO;
            }
        } else {
            self.stable = Duration::ZERO;
        }
    }

    pub fn budget(&self) -> Budget {
        let scale = [1, 2, 3, 4][self.level];
        // Reserve at most 6% of a slot for upload, capped at 1 ms at 60 Hz.
        let slot = if self.target.is_zero() {
            Duration::from_micros(16_667)
        } else {
            self.target
        };
        Budget {
            upload_bytes: scale as u64 * 256 * 1024,
            upload_time: (slot.mul_f64(0.06).min(Duration::from_millis(1)))
                .mul_f64(scale as f64 / 4.0),
            mesh_sections: scale * 6,
            mesh_bytes: (if self.memory_hold.is_zero() { 16 } else { 4 }) * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SLOT: Duration = Duration::from_nanos(16_666_667);
    const WORK: Duration = Duration::from_millis(8);

    fn stable(g: &mut Governor, frames: usize) {
        for _ in 0..frames {
            g.observe(SLOT, WORK, None, SLOT);
        }
    }

    #[test]
    fn vsync_wait_does_not_throttle_and_recovery_is_slow() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        assert_eq!(g.budget().upload_bytes, 1024 * 1024);
        g.observe(SLOT * 2, WORK, None, SLOT);
        assert!(g.budget().upload_bytes < 1024 * 1024);
        let reduced = g.budget().upload_bytes;
        stable(&mut g, 60);
        assert!(g.budget().upload_bytes <= reduced);
        stable(&mut g, 500);
        assert_eq!(g.budget().upload_bytes, 1024 * 1024);
    }

    #[test]
    fn gpu_pressure_throttles_even_when_cpu_is_fast() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        for _ in 0..4 {
            g.observe(SLOT, WORK, Some(30.0), SLOT);
        }
        assert_eq!(g.budget().mesh_sections, 6);
        assert_eq!(g.budget().upload_bytes, 256 * 1024);
    }

    #[test]
    fn memory_warning_holds_low_budget_across_suspension() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        g.memory_warning();
        g.observe(Duration::from_secs(20), WORK, None, SLOT);
        stable(&mut g, 240);
        assert_eq!(g.budget().mesh_bytes, 4 * 1024 * 1024);
        assert_eq!(g.budget().mesh_sections, 6);
        stable(&mut g, 500);
        assert_eq!(g.budget().mesh_bytes, 16 * 1024 * 1024);
        assert_eq!(g.budget().mesh_sections, 24);
    }

    #[test]
    fn high_refresh_reserves_less_upload_time() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        let at_60 = g.budget().upload_time;
        g.observe(SLOT / 2, WORK, None, SLOT / 2);
        assert!(g.budget().upload_time < at_60);
        assert!(g.budget().upload_time <= Duration::from_micros(500));
    }

    #[test]
    fn changed_target_and_invalid_gpu_samples_do_not_trigger_overload() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        let slow_slot = Duration::from_millis(33);
        g.observe(slow_slot, WORK, Some(f32::NAN), slow_slot);
        g.observe(slow_slot, WORK, Some(f32::INFINITY), slow_slot);
        assert_eq!(g.budget().upload_bytes, 1024 * 1024);
        g.observe(SLOT, WORK, None, Duration::ZERO);
        assert_eq!(g.budget().upload_bytes, 1024 * 1024);
    }
}
