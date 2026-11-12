//! iOS keyboard for screens whose text boxes the game cannot see.
use super::{Game, Screen};

impl Game<'_> {
    /// A native mod's own screen draws and reads its text boxes itself, through the screen
    /// hooks, and the mod API has no "a text box has focus" signal, so `text_field_focused`
    /// cannot tell. On iOS the keyboard is offered while such a screen is open (its committed
    /// text reaches the mod through `screen_text_typed`); elsewhere a physical keyboard is
    /// always there and nothing is asked of the OS.
    pub(super) fn opaque_screen_wants_keyboard(&self) -> bool {
        cfg!(target_os = "ios") && self.screen == Screen::Mod
    }
}
