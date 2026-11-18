//! Feedback for optional terrain work on memory-constrained devices. No GPU waits or
//! allocations: presentation cadence and already available GPU timestamps are enough.
use std::time::Duration;

const MAX_LEVEL: usize = 4;
const PRESSURE_WINDOW: Duration = Duration::from_millis(200);
const STEP_INTERVAL: Duration = Duration::from_millis(750);
const RECOVERY_WINDOW: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug)]
pub(super) struct Budget {
    pub upload_bytes: u64,
    pub upload_time: Duration,
    pub mesh_sections: usize,
    pub mesh_bytes: usize,
}

pub(super) struct Governor {
    level: usize,
    pressure: Duration,
    stable: Duration,
    cooldown: Duration,
    memory_hold: Duration,
    target: Duration,
}

impl Default for Governor {
    fn default() -> Self {
        Self {
            level: 1,
            pressure: Duration::ZERO,
            stable: Duration::ZERO,
            cooldown: Duration::ZERO,
            memory_hold: Duration::ZERO,
            target: Duration::ZERO,
        }
    }
}

impl Governor {
    pub fn memory_warning(&mut self) {
        self.level = 0;
        self.pressure = Duration::ZERO;
        self.stable = Duration::ZERO;
        self.cooldown = Duration::ZERO;
        self.memory_hold = Duration::from_secs(5);
    }

    /// `cadence` includes normal pacing. `work`
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
            self.pressure = Duration::ZERO;
            self.stable = Duration::ZERO;
            return;
        }
        self.memory_hold = self.memory_hold.saturating_sub(cadence);
        if !self.memory_hold.is_zero() {
            return;
        }
        self.cooldown = self.cooldown.saturating_sub(cadence);

        let target_ms = target.as_secs_f64() * 1000.0;
        let gpu = gpu_ms.filter(|ms| ms.is_finite() && *ms >= 0.0);
        let busy = (work.as_secs_f64() * 1000.0).max(f64::from(gpu.unwrap_or(0.0)));
        let cadence_ratio = cadence.as_secs_f64() / target.as_secs_f64();
        let busy_ratio = busy / target_ms;
        // A long frame earns at most one slot of pressure. A short frame can
        // offset a late wake-up; repeated misses or high work still accumulate.
        let sample = cadence.min(target).min(PRESSURE_WINDOW / 4);
        if cadence_ratio > 1.15 || busy_ratio > 0.92 {
            self.pressure = (self.pressure + sample).min(PRESSURE_WINDOW);
            self.stable = Duration::ZERO;
            if self.pressure >= PRESSURE_WINDOW && self.cooldown.is_zero() && self.level > 0 {
                self.level -= 1;
                self.pressure = Duration::ZERO;
                self.cooldown = STEP_INTERVAL;
            }
        } else {
            let decay = if cadence_ratio < 0.90 { target } else { sample / 2 };
            self.pressure = self.pressure.saturating_sub(decay);
            if self.pressure.is_zero() && cadence_ratio <= 1.08 && busy_ratio < 0.80 {
                self.stable = (self.stable + cadence).min(RECOVERY_WINDOW);
            } else {
                self.stable = Duration::ZERO;
            }
            if self.stable >= RECOVERY_WINDOW && self.cooldown.is_zero() && self.level < MAX_LEVEL {
                self.level += 1;
                self.stable = Duration::ZERO;
                self.cooldown = STEP_INTERVAL;
            }
        }
    }

    pub fn budget(&self) -> Budget {
        let scale = self.level + 2;
        // Reserve 1.5–4.5% of the frame slot, capped at 0.75 ms.
        let slot = if self.target.is_zero() {
            Duration::from_micros(16_667)
        } else {
            self.target
        };
        Budget {
            upload_bytes: scale as u64 * 128 * 1024,
            upload_time: slot
                .mul_f64(0.0075 * scale as f64)
                .min(Duration::from_micros(750)),
            mesh_sections: 6 + self.level * 2,
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
    fn isolated_late_frame_does_not_change_budget() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        let baseline = g.budget().upload_bytes;
        g.observe(Duration::from_millis(25), WORK, None, SLOT);
        stable(&mut g, 4);
        assert_eq!(g.budget().upload_bytes, baseline);
    }

    #[test]
    fn repeated_late_frames_reduce_one_step_and_respect_cooldown() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        let baseline = g.budget().upload_bytes;
        for _ in 0..16 {
            g.observe(Duration::from_millis(25), WORK, None, SLOT);
        }
        let reduced = g.budget().upload_bytes;
        assert!(reduced < baseline);
        for _ in 0..10 {
            g.observe(Duration::from_millis(25), WORK, None, SLOT);
        }
        assert_eq!(g.budget().upload_bytes, reduced);
        stable(&mut g, 400);
        assert_eq!(g.budget().upload_bytes, baseline);
    }

    #[test]
    fn gpu_pressure_throttles_even_when_cpu_is_fast() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        let initial_mesh_budget = g.budget().mesh_sections;
        let initial_upload_budget = g.budget().upload_bytes;
        for _ in 0..100 {
            g.observe(SLOT, WORK, Some(30.0), SLOT);
        }
        assert!(g.budget().mesh_sections < initial_mesh_budget);
        assert!(g.budget().upload_bytes < initial_upload_budget);
    }

    #[test]
    fn memory_warning_holds_low_budget_across_suspension() {
        let mut g = Governor::default();
        stable(&mut g, 400);
        g.memory_warning();
        g.observe(Duration::from_secs(20), WORK, None, SLOT);
        stable(&mut g, 100);
        assert_eq!(g.budget().mesh_bytes, 4 * 1024 * 1024);
        assert_eq!(g.budget().mesh_sections, 6);
        stable(&mut g, 1_000);
        assert_eq!(g.budget().mesh_bytes, 16 * 1024 * 1024);
        assert_eq!(g.budget().mesh_sections, 14);
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
        let baseline = g.budget().upload_bytes;
        let slow_slot = Duration::from_millis(33);
        g.observe(slow_slot, WORK, Some(f32::NAN), slow_slot);
        g.observe(slow_slot, WORK, Some(f32::INFINITY), slow_slot);
        assert_eq!(g.budget().upload_bytes, baseline);
        g.observe(SLOT, WORK, None, Duration::ZERO);
        assert_eq!(g.budget().upload_bytes, baseline);
    }
}
