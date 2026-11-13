//! The size a surface is configured with.

/// iOS: the window's outer size (the view, and so the surface, covers the whole screen). Its
/// inner size is only the safe area, clear of the notch and the home bar, so a surface made at
/// that size draws stretched until something resizes it (`follow_window_size`). Elsewhere
/// the inner size, as before.
pub(super) fn surface_size(window: &winit::window::Window) -> winit::dpi::PhysicalSize<u32> {
    #[cfg(target_os = "ios")]
    {
        let outer = window.outer_size();
        if outer.width > 0 && outer.height > 0 {
            return outer;
        }
    }
    window.inner_size()
}
