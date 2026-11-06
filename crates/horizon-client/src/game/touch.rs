//! The touch screen (iOS, which sends touches and no mouse). Over menus, the
//! first finger is the mouse; while playing, the on-screen controls
//! (`horizon_client_input::touch`) act as a virtual controller: its buttons
//! are the ones bound in the controller settings, its stick the left stick.
//! Dragging the view turns it like the mouse does.
use super::*;
use horizon_client::gamepad::{Button, PadState};
use horizon_client_input::touch::{Layout, STICK_RADIUS};
use winit::event::Touch;

/// How far the view turns per GUI unit dragged, in mouse-movement units
/// (`look_turn`): a drag across a phone's screen turns about half a circle.
const LOOK_SPEED: f64 = 2.0;

/// Colours, as the HUD's `0xAARRGGBB`.
const FACE: u32 = 0x5a00_0000;
const FACE_PRESSED: u32 = 0x9aff_ffff;
const EDGE: u32 = 0x80ff_ffff;
const LABEL: u32 = 0xffff_ffff;

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl Game<'_> {
    /// The controls' layout on this window, in GUI units, and the GUI scale.
    fn touch_layout(&self) -> Option<(Layout, f32)> {
        let batch = self.gui_scale()?;
        Some((Layout::new(batch.width, batch.height), batch.scale))
    }

    /// The world is being played, with no screen over it: the controls are up.
    fn touch_playing(&self) -> bool {
        !self.pad_in_menu() && self.player.is_some() && !self.ui.hide_gui
    }

    /// The mouse events `touch` stands for: a finger over a menu, or one that
    /// came down while a menu was open. A finger on the controls gives none.
    pub(super) fn touch_event(&mut self, touch: Touch) -> Vec<WindowEvent> {
        let mouse = self.pad.touch_mouse.finger() == Some(touch.id);
        // The on-screen keyboard takes every finger while it is open.
        if !mouse && self.pad.menu.osk.is_some() {
            if touch.phase == winit::event::TouchPhase::Ended
                && let Some((_, scale)) = self.touch_layout()
            {
                let at = (
                    touch.location.x as f32 / scale,
                    touch.location.y as f32 / scale,
                );
                self.touch_keyboard(at);
            }
            return Vec::new();
        }
        if !mouse
            && self.touch_playing()
            && let Some((layout, scale)) = self.touch_layout()
        {
            let at = (
                touch.location.x as f32 / scale,
                touch.location.y as f32 / scale,
            );
            self.pad
                .touch
                .touch(&layout, touch.id, touch.phase, at, Instant::now());
            self.last_input = Instant::now();
            return Vec::new();
        }
        self.pad.touch_mouse.translate(WindowEvent::Touch(touch))
    }

    /// Once per loop iteration, before the pads are read: the controls'
    /// buttons and stick as a virtual controller, the view's drag as a turn
    /// and a tapped slot as its number key.
    pub(super) fn touch_frame(&mut self) {
        if !self.touch_playing() {
            if self.pad.touch.active() {
                self.pad.touch.release_all();
            }
            self.pad.touch_state = PadState::default();
            return;
        }
        let frame = self.pad.touch.frame(Instant::now());
        let mut state = PadState {
            left: frame.stick,
            ..PadState::default()
        };
        for held in frame.held {
            let button = match held.action() {
                Some(action) => self.settings.controller.binding(action),
                None => Some(Button::Start),
            };
            match button {
                Some(Button::LeftTrigger) => state.triggers[0] = 1.0,
                Some(Button::RightTrigger) => state.triggers[1] = 1.0,
                Some(button) => state.set(button, true),
                None => {}
            }
        }
        self.pad.touch_state = state;
        if frame.look != (0.0, 0.0) {
            self.look_turn(
                f64::from(frame.look.0) * LOOK_SPEED,
                f64::from(frame.look.1) * LOOK_SPEED,
            );
        }
        if let Some(slot) = frame.slot
            && !self.spectator()
        {
            self.hotbar_presses.push(slot);
        }
    }

    /// The controls over the HUD while playing.
    pub(super) fn draw_touch_controls(&self, batch: &mut HudBatch) {
        if !TOUCH_CONTROLS || !self.touch_playing() {
            return;
        }
        let atlas = &self.gui.atlas;
        let layout = Layout::new(batch.width, batch.height);
        for (button, area) in &layout.buttons {
            let face = if self.pad.touch.pressed(*button) {
                FACE_PRESSED
            } else {
                FACE
            };
            batch.fill(atlas, area.x, area.y, area.w, area.h, face);
            outline(batch, atlas, (area.x, area.y, area.w, area.h), EDGE);
            let label = horizon_client::lang::get(button.label());
            let label = fit(atlas, &label, area.w - 4.0);
            let width = HudBatch::text_width(atlas, &label);
            batch.text(
                atlas,
                &label,
                area.x + (area.w - width) / 2.0,
                area.y + (area.h - 8.0) / 2.0,
                LABEL,
                true,
            );
        }
        if let Some((origin, knob)) = self.pad.touch.stick() {
            let r = STICK_RADIUS;
            batch.fill(atlas, origin.0 - r, origin.1 - r, r * 2.0, r * 2.0, FACE);
            outline(
                batch,
                atlas,
                (origin.0 - r, origin.1 - r, r * 2.0, r * 2.0),
                EDGE,
            );
            let k = r / 2.5;
            batch.fill(
                atlas,
                knob.0 - k,
                knob.1 - k,
                k * 2.0,
                k * 2.0,
                FACE_PRESSED,
            );
        }
    }
}

/// On iOS the touch controls are drawn and played.
pub(super) const TOUCH_CONTROLS: bool = cfg!(target_os = "ios");

/// A one-unit frame round `(x, y, w, h)`.
fn outline(batch: &mut HudBatch, atlas: &HudAtlas, (x, y, w, h): (f32, f32, f32, f32), c: u32) {
    batch.fill(atlas, x, y, w, 1.0, c);
    batch.fill(atlas, x, y + h - 1.0, w, 1.0, c);
    batch.fill(atlas, x, y + 1.0, 1.0, h - 2.0, c);
    batch.fill(atlas, x + w - 1.0, y + 1.0, 1.0, h - 2.0, c);
}

/// `text` cut to `width`, keeping whole characters.
fn fit(atlas: &HudAtlas, text: &str, width: f32) -> String {
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if HudBatch::text_width(atlas, &out) > width {
            out.pop();
            break;
        }
    }
    out
}
