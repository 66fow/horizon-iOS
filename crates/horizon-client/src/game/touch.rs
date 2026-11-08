//! The touch screen (iOS, which sends touches and no mouse). Over menus, the
//! first finger is the mouse; while playing, the on-screen controls
//! (`horizon_client_input::touch`) press the actions the controller settings
//! bind to their buttons and feed the left stick, but they are not a
//! controller: they never make the game think a pad is in use (no pad cursor
//! or glyphs in the menus they open). Sneak goes straight to the game's
//! Sneak option (Toggle or Hold). Dragging the view turns it like the mouse
//! does, at the Mouse Sensitivity option. The Edit button opens the
//! layout editor; its layout is kept in `touch-controls.txt` beside
//! `options.txt`.
use super::*;
use horizon_client::gamepad::{Button, PadState};
use horizon_client_input::touch::{
    Area, Control, EditorButton, Layout, TouchButton, TouchSettings, icons,
};
use winit::event::{Touch, TouchPhase};

/// How far the view turns per GUI unit dragged, in mouse-movement units
/// (`look_turn`): a drag across a phone's screen turns about half a circle.
const LOOK_SPEED: f64 = 2.0;

/// Colours, as the HUD's `0xAARRGGBB`, at full opacity.
const FACE: u32 = 0x6000_0000;
const FACE_PRESSED: u32 = 0xa0ff_ffff;
const EDGE: u32 = 0xa0ff_ffff;
const INK: u32 = 0xffff_ffff;
const INK_PRESSED: u32 = 0xff20_2020;
const SELECTED: u32 = 0xffff_e040;
const HIDDEN_EDGE: u32 = 0xc0ff_5050;
const SHADE: u32 = 0x5000_0000;

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl Game<'_> {
    /// The part of the window clear of the notch and the home bar, in GUI
    /// units at `scale`: iOS reports it as the window's inner rectangle.
    pub(super) fn touch_safe(&self, width: f32, height: f32, scale: f32) -> Area {
        let whole = Area {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        };
        let Some(window) = self.window.as_ref() else {
            return whole;
        };
        let (Ok(inner), Ok(outer)) = (window.inner_position(), window.outer_position()) else {
            return whole;
        };
        let size = window.inner_size();
        let safe = Area {
            x: (inner.x - outer.x) as f32 / scale,
            y: (inner.y - outer.y) as f32 / scale,
            w: size.width as f32 / scale,
            h: size.height as f32 / scale,
        };
        // A safe area that doesn't fit the window is no safe area.
        if safe.w <= 0.0
            || safe.h <= 0.0
            || safe.x + safe.w > width + 1.0
            || safe.y + safe.h > height + 1.0
        {
            return whole;
        }
        safe
    }

    /// The controls' layout on this window, in GUI units, and the GUI scale.
    fn touch_layout(&self) -> Option<(Layout, f32)> {
        let batch = self.gui_scale()?;
        let safe = self.touch_safe(batch.width, batch.height, batch.scale);
        Some((
            self.pad.touch.layout(batch.width, batch.height, safe),
            batch.scale,
        ))
    }

    /// The layout for a finger that is already down (it moves or lifts): the
    /// one of the last frame, so a drag costs no window queries and no layout
    /// per event. A finger coming down always gets a fresh one.
    fn touch_layout_for(&mut self, phase: TouchPhase) -> Option<(Layout, f32)> {
        if phase != TouchPhase::Started
            && let Some(cached) = self.pad.touch_layout.clone()
        {
            return Some(cached);
        }
        let fresh = self.touch_layout()?;
        self.pad.touch_layout = Some(fresh.clone());
        Some(fresh)
    }

    /// The saved layout, read once.
    fn touch_load(&mut self) {
        if self.pad.touch_loaded {
            return;
        }
        self.pad.touch_loaded = true;
        let path = touch_settings_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => self.pad.touch.settings = TouchSettings::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!(
                "Horizon: touch controls: can't read {}: {e}",
                path.display()
            ),
        }
    }

    /// The Esc button: always top left in the safe area, on every screen, as
    /// the keyboard's Escape (a touch screen has no key for it). Playing, it
    /// opens the pause menu (where Save and Quit leaves the world); over a
    /// menu (an inventory, a crafting table, chat, the options) it closes it
    /// or goes back. Not on the main screen (the title and the menus over it
    /// have their own Back and Done buttons), and not while the layout editor
    /// or the mods screen is open (their Done and Close close them).
    fn touch_close_button(&self, batch: &HudBatch) -> Option<Area> {
        if !TOUCH_CONTROLS
            || self.title.is_some()
            || self.pad.mod_browser.is_some()
            || (self.pad.touch.editing() && !self.pad_in_menu())
        {
            return None;
        }
        let safe = self.touch_safe(batch.width, batch.height, batch.scale);
        Some(Area {
            x: safe.x + 4.0,
            y: safe.y + 4.0,
            w: 28.0,
            h: 20.0,
        })
    }

    /// The Esc button (`touch_close_button`).
    pub(super) fn draw_touch_close(&self, batch: &mut HudBatch) {
        if let Some(a) = self.touch_close_button(batch) {
            let atlas = &self.gui.atlas;
            batch.fill(atlas, a.x, a.y, a.w, a.h, 0xc020_2020);
            outline(batch, atlas, a, EDGE);
            let label = horizon_client::lang::get("horizon.touch.escape");
            let label = fit(atlas, &label, a.w - 4.0);
            let width = HudBatch::text_width(atlas, &label);
            batch.text(
                atlas,
                &label,
                a.x + (a.w - width) / 2.0,
                a.y + (a.h - 8.0) / 2.0,
                INK,
                true,
            );
        }
    }

    /// The world is being played, with no screen over it: the controls are up.
    fn touch_playing(&self) -> bool {
        !self.pad_in_menu() && self.player.is_some() && !self.ui.hide_gui
    }

    /// The mouse events `touch` stands for: a finger over a menu, or one that
    /// came down while a menu was open. A finger on the controls gives none.
    pub(super) fn touch_event(&mut self, touch: Touch) -> Vec<WindowEvent> {
        self.touch_load();
        // The mods screen and the title screen's button for it.
        if self.mods_touch(&touch) {
            return Vec::new();
        }
        let mouse = self.pad.touch_mouse.finger() == Some(touch.id);
        // The Esc button, top left in a world: Escape's job.
        if !mouse
            && touch.phase == TouchPhase::Started
            && let Some(batch) = self.gui_scale()
            && let Some(close) = self.touch_close_button(&batch)
            && close.contains((
                touch.location.x as f32 / batch.scale,
                touch.location.y as f32 / batch.scale,
            ))
        {
            log::info!("Horizon: touch Esc");
            if let Err(e) = self.pad_back() {
                log::warn!("Horizon: closing the menu: {e}");
            }
            return Vec::new();
        }
        // The on-screen keyboard takes every finger while it is open.
        if !mouse && self.pad.menu.osk.is_some() {
            if touch.phase == TouchPhase::Ended
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
            && let Some((layout, scale)) = self.touch_layout_for(touch.phase)
        {
            let at = (
                touch.location.x as f32 / scale,
                touch.location.y as f32 / scale,
            );
            if touch.phase == TouchPhase::Started
                && !self.pad.touch.editing()
                && let Some(button) = layout.button_at(at)
            {
                log::info!(
                    "Horizon: touch button {button:?} at ({:.0}, {:.0})",
                    at.0,
                    at.1
                );
            }
            if touch.phase == TouchPhase::Started {
                // A finger on the controls is not a controller: a menu it opens
                // (the inventory, chat) is the finger's, not a pad's focus.
                self.pad.in_use = false;
            }
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
        self.mods_poll();
        if let Some(settings) = self.pad.touch.take_saved() {
            let path = touch_settings_path();
            match std::fs::write(&path, settings.to_text()) {
                Ok(()) => log::info!("Horizon: touch controls saved to {}", path.display()),
                Err(e) => log::warn!(
                    "Horizon: touch controls: can't save {}: {e}",
                    path.display()
                ),
            }
        }
        let layout = self
            .touch_layout()
            .filter(|_| self.touch_playing())
            .map(|(layout, scale)| {
                self.pad.touch_layout = Some((layout.clone(), scale));
                layout
            });
        let Some(layout) = layout else {
            if self.pad.touch.active() {
                self.pad.touch.release_all();
            }
            self.pad.touch_layout = None;
            self.pad.touch_state = PadState::default();
            self.touch_sneak(false);
            return;
        };
        // Sneak follows the game's own Sneak option (Toggle or Hold), not the
        // controller's toggle sneak: the touch button is a key, not a pad.
        self.pad.touch.sneak_taps = self.settings.toggle_crouch;
        let frame = self.pad.touch.frame(&layout, Instant::now());
        self.touch_sneak(frame.held.contains(&TouchButton::Sneak));
        let mut state = PadState {
            left: frame.stick,
            ..PadState::default()
        };
        // F3 is the keyboard's: pressed and released as the key.
        let debug = frame.held.contains(&TouchButton::Debug);
        if debug != self.pad.touch_debug {
            self.pad.touch_debug = debug;
            // KeyboardHandler.keyPress: F3 is a debug key first (its release toggles the
            // overlay), the way the keyboard's F3 goes in window_event.
            use horizon_client::debug_keys::KeyAction;
            let action = if debug {
                KeyAction::Press
            } else {
                KeyAction::Release
            };
            if debug {
                self.physical_keys.insert(KeyCode::F3);
            } else {
                self.physical_keys.remove(&KeyCode::F3);
            }
            if !self.debug_key(KeyCode::F3, action)
                && let Err(e) = self.mapped_key_event(KeyCode::F3, debug, false)
            {
                log::warn!("Horizon: touch F3: {e}");
            }
        }
        for held in frame.held {
            // Sneak went to the game above. With Toggle sprint the game flips
            // sprinting on each press, so a stick pushed forward again must
            // not press it while it is already on (that would switch it off).
            if held == TouchButton::Sneak
                || (held == TouchButton::Sprint
                    && self.settings.toggle_sprint
                    && self.keys.contains(&KeyCode::ControlLeft))
            {
                continue;
            }
            let button = match held.action() {
                Some(action) => self.settings.controller.binding(action),
                None if held == TouchButton::Pause => Some(Button::Start),
                None => None,
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
            // Like the mouse: Mouse Sensitivity (and the spyglass's slower
            // turn) scale the drag, and Invert Mouse flips it.
            let look = self.look_options();
            let scale = horizon_client::mouse::degrees_per_count(look) / 0.15;
            let flip = |inverted: bool| if inverted { -1.0 } else { 1.0 };
            self.look_turn(
                f64::from(frame.look.0) * LOOK_SPEED * scale * flip(look.invert_x),
                f64::from(frame.look.1) * LOOK_SPEED * scale * flip(look.invert_y),
            );
        }
        if let Some(slot) = frame.slot
            && !self.spectator()
        {
            self.hotbar_presses.push(slot);
        }
    }

    /// The Sneak button as the sneak key, pressed and released on its edges:
    /// the game's own Sneak option makes it a toggle or a hold
    /// (`ToggleKeyMapping`), as for the keyboard's Shift.
    fn touch_sneak(&mut self, down: bool) {
        if down == self.pad.touch_sneak {
            return;
        }
        self.pad.touch_sneak = down;
        if let Err(e) = self.mapped_key(
            KeyCode::ShiftLeft,
            down,
            false,
            KeyCode::ShiftLeft,
            Source::Pad,
        ) {
            log::warn!("Horizon: touch sneak: {e}");
        }
    }

    /// Moves what was drawn into `batch` since `from` (a vertex count) by the
    /// hotbar's offset in the touch layout: the hotbar, hearts, food and
    /// experience bar follow the hotbar wherever the player put it.
    pub(super) fn touch_follow_hotbar(&self, batch: &mut HudBatch, from: usize) {
        if !TOUCH_CONTROLS {
            return;
        }
        let safe = self.touch_safe(batch.width, batch.height, batch.scale);
        let layout = self.pad.touch.layout(batch.width, batch.height, safe);
        let (dx, dy) = layout.hotbar_offset();
        if (dx, dy) == (0.0, 0.0) {
            return;
        }
        for v in batch.vertices.iter_mut().skip(from) {
            v.pos[0] += dx;
            v.pos[1] += dy;
        }
    }

    /// The controls over the HUD while playing, or the layout editor.
    pub(super) fn draw_touch_controls(&self, batch: &mut HudBatch) {
        if !TOUCH_CONTROLS || !self.touch_playing() {
            return;
        }
        let atlas = &self.gui.atlas;
        let safe = self.touch_safe(batch.width, batch.height, batch.scale);
        let layout = self.pad.touch.layout(batch.width, batch.height, safe);
        if let Some(view) = self.pad.touch.editor() {
            self.draw_touch_editor(batch, &layout, view);
            return;
        }
        for item in layout.items.iter().filter(|i| !i.hidden) {
            match item.control {
                Control::Button(button) => {
                    let pressed = if button == TouchButton::Sneak {
                        // The game's state: lit while sneaking, with Toggle after
                        // the finger lifted too.
                        self.keys.contains(&KeyCode::ShiftLeft)
                    } else {
                        self.pad.touch.pressed(button)
                    };
                    draw_button(batch, atlas, button, item.area, item.opacity, pressed);
                }
                Control::Stick => {
                    let r = layout.stick_radius;
                    let (origin, knob, k) = match self.pad.touch.stick(&layout) {
                        Some((origin, knob)) => (origin, knob, item.opacity),
                        // At rest it shows faintly where it sits.
                        None => (layout.stick_home, layout.stick_home, item.opacity * 0.6),
                    };
                    disc(batch, atlas, origin, r, faded(FACE, k));
                    ring(batch, atlas, origin, r, faded(EDGE, k));
                    disc(batch, atlas, knob, r * 0.4, faded(FACE_PRESSED, k));
                }
                Control::Hotbar => {}
            }
        }
    }

    /// The editor: every control (hidden ones dimmed with a red edge), the
    /// picked one outlined, the toolbar, and what is picked.
    fn draw_touch_editor(
        &self,
        batch: &mut HudBatch,
        layout: &Layout,
        view: horizon_client_input::touch::EditorView,
    ) {
        let atlas = &self.gui.atlas;
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, SHADE);
        for item in &layout.items {
            let k = if item.hidden { 0.35 } else { item.opacity };
            let a = item.area;
            match item.control {
                Control::Button(button) => draw_button(batch, atlas, button, a, k, false),
                Control::Stick => {
                    outline(batch, atlas, a, faded(EDGE, 0.5));
                    let home = layout.stick_home;
                    disc(batch, atlas, home, layout.stick_radius, faded(FACE, k));
                    ring(batch, atlas, home, layout.stick_radius, faded(EDGE, k));
                }
                Control::Hotbar => outline(batch, atlas, a, faded(EDGE, 0.8)),
            }
            if item.hidden {
                outline(batch, atlas, a, HIDDEN_EDGE);
            }
            if view.selected == Some(item.control) {
                let grown = Area {
                    x: a.x - 2.0,
                    y: a.y - 2.0,
                    w: a.w + 4.0,
                    h: a.h + 4.0,
                };
                outline(batch, atlas, grown, SELECTED);
                outline(
                    batch,
                    atlas,
                    Area {
                        x: grown.x + 1.0,
                        y: grown.y + 1.0,
                        w: grown.w - 2.0,
                        h: grown.h - 2.0,
                    },
                    SELECTED,
                );
            }
        }
        let mut bottom = layout.safe.y;
        for (button, a) in EditorButton::toolbar(layout) {
            let face = if button == EditorButton::Done {
                0xd030_8040
            } else {
                0xc020_2020
            };
            batch.fill(atlas, a.x, a.y, a.w, a.h, face);
            outline(batch, atlas, a, EDGE);
            let label = horizon_client::lang::get(button.label(view.placement.hidden));
            let label = fit(atlas, &label, a.w - 4.0);
            let width = HudBatch::text_width(atlas, &label);
            batch.text(
                atlas,
                &label,
                a.x + (a.w - width) / 2.0,
                a.y + (a.h - 8.0) / 2.0,
                INK,
                true,
            );
            bottom = bottom.max(a.y + a.h);
        }
        let caption = match view.selected {
            Some(control) => {
                let name = horizon_client::lang::get(control.label());
                let p = view.placement;
                if p.hidden {
                    horizon_client::lang::get("horizon.touch.edit.hidden").replace("{name}", &name)
                } else {
                    horizon_client::lang::get("horizon.touch.edit.caption")
                        .replace("{name}", &name)
                        .replace("{size}", &format!("{:.0}", p.size * 100.0))
                        .replace("{opacity}", &format!("{:.0}", p.opacity * 100.0))
                }
            }
            None => horizon_client::lang::get("horizon.touch.edit.hint"),
        };
        let caption = fit(atlas, &caption, layout.safe.w - 8.0);
        let width = HudBatch::text_width(atlas, &caption);
        batch.text(
            atlas,
            &caption,
            layout.safe.x + (layout.safe.w - width) / 2.0,
            bottom + 4.0,
            INK,
            true,
        );
    }
}

/// On iOS the touch controls are drawn and played.
pub(super) const TOUCH_CONTROLS: bool = cfg!(target_os = "ios");

/// Where the layout is kept: beside `options.txt`.
fn touch_settings_path() -> std::path::PathBuf {
    horizon_client::logging::player_dir().join("touch-controls.txt")
}

/// `colour` with its alpha scaled by `k`.
fn faded(colour: u32, k: f32) -> u32 {
    let alpha = ((colour >> 24) as f32 * k.clamp(0.0, 1.0)).round() as u32;
    (alpha << 24) | (colour & 0x00ff_ffff)
}

/// A button: its face, edge and icon, at opacity `k`.
fn draw_button(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    button: TouchButton,
    a: Area,
    k: f32,
    pressed: bool,
) {
    let (face, ink) = if pressed {
        (FACE_PRESSED, INK_PRESSED)
    } else {
        (FACE, INK)
    };
    batch.fill(atlas, a.x, a.y, a.w, a.h, faded(face, k));
    outline(batch, atlas, a, faded(EDGE, k));
    draw_icon(batch, atlas, icons::icon(button), a, faded(ink, k));
}

/// `icon` centred in `a`, in `ink`.
fn draw_icon(batch: &mut HudBatch, atlas: &HudAtlas, icon: &icons::Icon, a: Area, ink: u32) {
    let px = (a.w.min(a.h) * 0.62 / icons::SIZE as f32).max(0.5);
    let side = px * icons::SIZE as f32;
    let (x0, y0) = (a.x + (a.w - side) / 2.0, a.y + (a.h - side) / 2.0);
    for (row, col, len) in icons::runs(icon) {
        batch.fill(
            atlas,
            x0 + col as f32 * px,
            y0 + row as f32 * px,
            len as f32 * px,
            px,
            ink,
        );
    }
}

/// A one-unit frame round `a`.
fn outline(batch: &mut HudBatch, atlas: &HudAtlas, a: Area, c: u32) {
    batch.fill(atlas, a.x, a.y, a.w, 1.0, c);
    batch.fill(atlas, a.x, a.y + a.h - 1.0, a.w, 1.0, c);
    batch.fill(atlas, a.x, a.y + 1.0, 1.0, a.h - 2.0, c);
    batch.fill(atlas, a.x + a.w - 1.0, a.y + 1.0, 1.0, a.h - 2.0, c);
}

/// A filled circle, as one span per unit-high row.
fn disc(batch: &mut HudBatch, atlas: &HudAtlas, (cx, cy): (f32, f32), r: f32, c: u32) {
    let rows = r.ceil() as i32;
    for i in -rows..rows {
        let y = i as f32 + 0.5;
        let half = (r * r - y * y).max(0.0).sqrt();
        if half > 0.0 {
            batch.fill(atlas, cx - half, cy + i as f32, half * 2.0, 1.0, c);
        }
    }
}

/// A circle's one-unit edge.
fn ring(batch: &mut HudBatch, atlas: &HudAtlas, (cx, cy): (f32, f32), r: f32, c: u32) {
    let rows = r.ceil() as i32;
    let inner = (r - 1.0).max(0.0);
    for i in -rows..rows {
        let y = i as f32 + 0.5;
        let outer = (r * r - y * y).max(0.0).sqrt();
        let within = (inner * inner - y * y).max(0.0).sqrt();
        if outer <= 0.0 {
            continue;
        }
        let w = (outer - within).max(1.0);
        batch.fill(atlas, cx - outer, cy + i as f32, w, 1.0, c);
        batch.fill(atlas, cx + outer - w, cy + i as f32, w, 1.0, c);
    }
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
