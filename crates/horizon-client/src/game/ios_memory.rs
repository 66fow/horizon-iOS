//! iOS memory pressure. UIKit warns before it ends a process for its memory (jetsam); winit
//! hands the warning to `ApplicationHandler::memory_warning`, which the game left empty.
use super::Game;

impl Game<'_> {
    /// Gives back what can be given back without losing anything the game still draws:
    /// the renderer's arena shrinking (`Renderer::trim_memory`), then the pages mimalloc and
    /// the system heap hold free (`horizon_threads::release_free_memory`).
    pub(super) fn on_memory_warning(&mut self) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.trim_memory();
        }
        horizon_threads::release_free_memory();
    }
}
