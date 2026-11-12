//! What the renderer gives back when iOS warns about memory.
use super::*;

impl Renderer {
    /// Lets the arenas' shrinking start now: its waits (`SHRINK_SETTLE`, `SHRINK_RETRY`) keep
    /// frames smooth, and a memory warning outranks them. The copies stay capped per frame
    /// (`COMPACT_BUDGET`), so this costs no hitch of its own.
    pub fn trim_memory(&mut self) {
        self.shrink_gate = ShrinkGate::default();
    }
}
