//! Touch screens. Over menus, [`TouchMouse`] makes the first finger the mouse.
//! While playing, [`TouchControls`] are the on-screen controls: a movement
//! stick on the left, dragging anywhere else turns the view, a quick tap uses
//! (places, eats, opens) and holding still attacks (breaks), plus buttons for
//! jump, sneak, attack, use, inventory, chat, drop, perspective and pause, and
//! a tap on a hotbar slot selects it. iOS sends only touches.
//!
//! Positions are in GUI units (physical pixels over the GUI scale), the units
//! the HUD is drawn in, so the controls keep their size on every screen.
use std::collections::HashMap;
use std::time::{Duration, Instant};
use winit::event::{ElementState, MouseButton, Touch, TouchPhase, WindowEvent};

/// A touch screen as a mouse: the first finger down moves the cursor and
/// holds the left button until it lifts; other fingers are ignored. This is
/// what lets a finger press the menus' buttons.
#[derive(Default, Debug)]
pub struct TouchMouse {
    finger: Option<u64>,
}

impl TouchMouse {
    /// The finger acting as the mouse.
    pub fn finger(&self) -> Option<u64> {
        self.finger
    }
    /// The mouse events `event` stands for. A touch by another finger gives
    /// none; any other event passes through unchanged.
    pub fn translate(&mut self, event: WindowEvent) -> Vec<WindowEvent> {
        let WindowEvent::Touch(Touch {
            device_id,
            phase,
            location,
            id,
            ..
        }) = event
        else {
            return vec![event];
        };
        let moved = WindowEvent::CursorMoved {
            device_id,
            position: location,
        };
        let button = |state| WindowEvent::MouseInput {
            device_id,
            state,
            button: MouseButton::Left,
        };
        match phase {
            TouchPhase::Started if self.finger.is_none() => {
                self.finger = Some(id);
                vec![moved, button(ElementState::Pressed)]
            }
            TouchPhase::Moved if self.finger == Some(id) => vec![moved],
            TouchPhase::Ended | TouchPhase::Cancelled if self.finger == Some(id) => {
                self.finger = None;
                vec![moved, button(ElementState::Released)]
            }
            _ => Vec::new(),
        }
    }
}

/// An on-screen button while playing. The game presses the controller
/// button bound to `action()`, so a rebind in the controller settings moves
/// the touch button with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchButton {
    Jump,
    Sneak,
    Attack,
    Use,
    Inventory,
    Chat,
    Drop,
    Perspective,
    Pause,
    /// Held while the stick is pushed fully forward.
    Sprint,
}

impl TouchButton {
    /// The key mapping it stands for; `None` for pause, the controller's Start.
    pub fn action(self) -> Option<&'static str> {
        Some(match self {
            Self::Jump => "key.jump",
            Self::Sneak => "key.sneak",
            Self::Attack => "key.attack",
            Self::Use => "key.use",
            Self::Inventory => "key.inventory",
            Self::Chat => "key.chat",
            Self::Drop => "key.drop",
            Self::Perspective => "key.togglePerspective",
            Self::Sprint => "key.sprint",
            Self::Pause => return None,
        })
    }
    /// The translation key of its label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Jump => "horizon.touch.jump",
            Self::Sneak => "horizon.touch.sneak",
            Self::Attack => "horizon.touch.attack",
            Self::Use => "horizon.touch.use",
            Self::Inventory => "horizon.touch.inventory",
            Self::Chat => "horizon.touch.chat",
            Self::Drop => "horizon.touch.drop",
            Self::Perspective => "horizon.touch.perspective",
            Self::Pause => "horizon.touch.pause",
            Self::Sprint => "key.sprint",
        }
    }
}

/// A rectangle in GUI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Area {
    pub fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Vanilla's hotbar: 182 by 22 units, centred at the bottom, slots 20 apart.
pub const HOTBAR_WIDTH: f32 = 182.0;
pub const HOTBAR_HEIGHT: f32 = 22.0;
/// How far the stick's knob travels from where the finger came down.
pub const STICK_RADIUS: f32 = 26.0;
/// A finger that moved less than this is still a tap or a hold.
const STILL: f32 = 6.0;
/// Shorter than this, a touch on the view is a tap (use).
const TAP: Duration = Duration::from_millis(250);
/// Held still this long, a touch on the view attacks (breaks a block).
const HOLD: Duration = Duration::from_millis(300);
/// A tapped button stays down this long: the game samples keys once a tick (50 ms).
const PULSE: Duration = Duration::from_millis(80);

/// Where the controls are on a screen `width` by `height` GUI units.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub buttons: Vec<(TouchButton, Area)>,
    /// A finger coming down here becomes the stick, centred where it landed.
    pub stick_zone: Area,
    pub hotbar: Area,
}

impl Layout {
    pub fn new(width: f32, height: f32) -> Self {
        let size = 30.0;
        let gap = 4.0;
        let small = 22.0;
        // Columns from the right edge, rows up from the bottom.
        let col = |i: f32| width - gap - size - i * (size + gap);
        let row = |i: f32| height - gap - size - i * (size + gap);
        let big = |x, y| Area {
            x,
            y,
            w: size,
            h: size,
        };
        let top = |i: f32| Area {
            x: width - gap - small - i * (small + gap),
            y: gap,
            w: small,
            h: small,
        };
        let hotbar = Area {
            x: ((width - HOTBAR_WIDTH) / 2.0).floor(),
            y: height - HOTBAR_HEIGHT,
            w: HOTBAR_WIDTH,
            h: HOTBAR_HEIGHT,
        };
        let buttons = vec![
            (TouchButton::Jump, big(col(0.0), row(0.0))),
            (TouchButton::Sneak, big(col(1.0), row(0.0))),
            (TouchButton::Attack, big(col(0.0), row(1.0))),
            (TouchButton::Use, big(col(1.0), row(1.0))),
            (TouchButton::Drop, big(col(0.0), row(2.0))),
            (TouchButton::Pause, top(0.0)),
            (TouchButton::Chat, top(1.0)),
            (TouchButton::Perspective, top(2.0)),
            (
                TouchButton::Inventory,
                Area {
                    x: hotbar.x + hotbar.w + gap,
                    y: hotbar.y,
                    w: small,
                    h: small,
                },
            ),
        ];
        let stick_zone = Area {
            x: 0.0,
            y: height * 0.3,
            w: (width * 0.4).min(hotbar.x),
            h: height * 0.7,
        };
        Self {
            width,
            height,
            buttons,
            stick_zone,
            hotbar,
        }
    }

    /// The button under `at`, if any.
    pub fn button_at(&self, at: (f32, f32)) -> Option<TouchButton> {
        self.buttons
            .iter()
            .find(|(_, area)| area.contains(at))
            .map(|(b, _)| *b)
    }

    /// The hotbar slot (0 to 8) under `at`.
    pub fn slot_at(&self, at: (f32, f32)) -> Option<usize> {
        self.hotbar
            .contains(at)
            .then(|| (((at.0 - self.hotbar.x - 1.0) / 20.0).floor().max(0.0) as usize).min(8))
    }
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Stick {
        origin: (f32, f32),
        at: (f32, f32),
    },
    Look {
        last: (f32, f32),
        start: (f32, f32),
        since: Instant,
        moved: bool,
        attacking: bool,
    },
    Button(TouchButton),
    /// A finger on the hotbar picks its slot as it lands and then does nothing.
    Hotbar,
}

/// What the controls want this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchFrame {
    /// Buttons to hold down now.
    pub held: Vec<TouchButton>,
    /// The movement stick, -1 to 1, up (forward) positive.
    pub stick: [f32; 2],
    /// How far the view was dragged since the last frame, in GUI units.
    pub look: (f32, f32),
    /// A hotbar slot tapped since the last frame.
    pub slot: Option<usize>,
}

/// The on-screen controls while playing.
#[derive(Default, Debug)]
pub struct TouchControls {
    fingers: HashMap<u64, Role>,
    /// Sneak toggles: one tap down, the next back up.
    sneaking: bool,
    /// Buttons tapped and held for a pulse, until then.
    pulses: Vec<(TouchButton, Instant)>,
    look: (f32, f32),
    slot: Option<usize>,
}

impl TouchControls {
    /// A finger came down, moved, lifted or was cancelled at `at`.
    pub fn touch(
        &mut self,
        layout: &Layout,
        id: u64,
        phase: winit::event::TouchPhase,
        at: (f32, f32),
        now: Instant,
    ) {
        use winit::event::TouchPhase::*;
        match phase {
            Started => {
                let role = if let Some(button) = layout.button_at(at) {
                    if button == TouchButton::Sneak {
                        self.sneaking = !self.sneaking;
                    } else {
                        self.pulse(button, now);
                    }
                    Role::Button(button)
                } else if let Some(slot) = layout.slot_at(at) {
                    self.slot = Some(slot);
                    Role::Hotbar
                } else if layout.stick_zone.contains(at)
                    && !self
                        .fingers
                        .values()
                        .any(|r| matches!(r, Role::Stick { .. }))
                {
                    Role::Stick { origin: at, at }
                } else {
                    Role::Look {
                        last: at,
                        start: at,
                        since: now,
                        moved: false,
                        attacking: false,
                    }
                };
                self.fingers.insert(id, role);
            }
            Moved => match self.fingers.get_mut(&id) {
                Some(Role::Stick { at: stick, .. }) => *stick = at,
                Some(Role::Look {
                    last, start, moved, ..
                }) => {
                    self.look.0 += at.0 - last.0;
                    self.look.1 += at.1 - last.1;
                    *last = at;
                    if distance(*start, at) > STILL {
                        *moved = true;
                    }
                }
                _ => {}
            },
            Ended | Cancelled => {
                if let Some(Role::Look {
                    since,
                    moved,
                    attacking,
                    ..
                }) = self.fingers.remove(&id)
                    && phase == Ended
                    && !moved
                    && !attacking
                    && now.duration_since(since) < TAP
                {
                    self.pulse(TouchButton::Use, now);
                }
            }
        }
    }

    fn pulse(&mut self, button: TouchButton, now: Instant) {
        self.pulses.push((button, now + PULSE));
    }

    /// Lets go of everything (a menu opened, the app went to the background).
    pub fn release_all(&mut self) {
        self.fingers.clear();
        self.pulses.clear();
        self.look = (0.0, 0.0);
        self.slot = None;
    }

    /// Whether any finger is on the controls.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }

    /// The stick's centre and its knob, for drawing.
    pub fn stick(&self) -> Option<((f32, f32), (f32, f32))> {
        self.fingers.values().find_map(|r| match *r {
            Role::Stick { origin, at } => Some((origin, knob(origin, at))),
            _ => None,
        })
    }

    /// Whether `button` is down now, for drawing it pressed.
    pub fn pressed(&self, button: TouchButton) -> bool {
        (button == TouchButton::Sneak && self.sneaking)
            || self
                .fingers
                .values()
                .any(|r| matches!(r, Role::Button(b) if *b == button))
    }

    /// The state to apply this frame; the look and the tapped slot are taken.
    pub fn frame(&mut self, now: Instant) -> TouchFrame {
        self.pulses.retain(|(_, until)| *until > now);
        let mut held: Vec<TouchButton> = self.pulses.iter().map(|(b, _)| *b).collect();
        let mut stick = [0.0; 2];
        for role in self.fingers.values_mut() {
            match role {
                Role::Button(b) if *b != TouchButton::Sneak => held.push(*b),
                Role::Stick { origin, at } => {
                    let k = knob(*origin, *at);
                    stick = [
                        (k.0 - origin.0) / STICK_RADIUS,
                        -(k.1 - origin.1) / STICK_RADIUS,
                    ];
                    if stick[1] > 0.9 {
                        held.push(TouchButton::Sprint);
                    }
                }
                Role::Look {
                    since,
                    moved,
                    attacking,
                    ..
                } => {
                    if *attacking || (!*moved && now.duration_since(*since) >= HOLD) {
                        *attacking = true;
                        held.push(TouchButton::Attack);
                    }
                }
                _ => {}
            }
        }
        if self.sneaking {
            held.push(TouchButton::Sneak);
        }
        held.sort_by_key(|b| *b as u8);
        held.dedup();
        TouchFrame {
            held,
            stick,
            look: std::mem::take(&mut self.look),
            slot: self.slot.take(),
        }
    }
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// `at`, held within the stick's radius of `origin`.
fn knob(origin: (f32, f32), at: (f32, f32)) -> (f32, f32) {
    let d = distance(origin, at);
    if d <= STICK_RADIUS {
        return at;
    }
    let k = STICK_RADIUS / d;
    (
        origin.0 + (at.0 - origin.0) * k,
        origin.1 + (at.1 - origin.1) * k,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::DeviceId;

    fn touch(id: u64, phase: TouchPhase, x: f64, y: f64) -> WindowEvent {
        WindowEvent::Touch(Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: PhysicalPosition::new(x, y),
            force: None,
            id,
        })
    }

    /// (x, y) of a cursor move, or the button's state.
    #[derive(Debug, PartialEq)]
    enum Out {
        Move(f64, f64),
        Button(ElementState),
    }

    fn outs(events: Vec<WindowEvent>) -> Vec<Out> {
        events
            .into_iter()
            .map(|e| match e {
                WindowEvent::CursorMoved { position, .. } => Out::Move(position.x, position.y),
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => Out::Button(state),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_tap_is_a_left_click_where_the_finger_is() {
        let mut t = TouchMouse::default();
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Started, 10.0, 20.0))),
            [Out::Move(10.0, 20.0), Out::Button(ElementState::Pressed)]
        );
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Ended, 10.0, 20.0))),
            [Out::Move(10.0, 20.0), Out::Button(ElementState::Released)]
        );
    }

    #[test]
    fn a_drag_moves_the_cursor_with_the_button_held() {
        let mut t = TouchMouse::default();
        t.translate(touch(1, TouchPhase::Started, 0.0, 0.0));
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Moved, 5.0, 7.0))),
            [Out::Move(5.0, 7.0)]
        );
    }

    #[test]
    fn a_second_finger_is_ignored_until_the_first_lifts() {
        let mut t = TouchMouse::default();
        t.translate(touch(1, TouchPhase::Started, 0.0, 0.0));
        assert!(
            t.translate(touch(2, TouchPhase::Started, 9.0, 9.0))
                .is_empty()
        );
        assert!(
            t.translate(touch(2, TouchPhase::Moved, 8.0, 8.0))
                .is_empty()
        );
        assert!(
            t.translate(touch(2, TouchPhase::Ended, 8.0, 8.0))
                .is_empty()
        );
        // A cancelled touch (a system gesture took over) still lets go of the button.
        assert_eq!(
            outs(t.translate(touch(1, TouchPhase::Cancelled, 1.0, 1.0))),
            [Out::Move(1.0, 1.0), Out::Button(ElementState::Released)]
        );
        // The next finger down becomes the mouse.
        assert_eq!(
            outs(t.translate(touch(2, TouchPhase::Started, 3.0, 4.0))),
            [Out::Move(3.0, 4.0), Out::Button(ElementState::Pressed)]
        );
    }

    #[test]
    fn other_events_pass_through() {
        let mut t = TouchMouse::default();
        let out = t.translate(WindowEvent::Focused(true));
        assert!(matches!(out[..], [WindowEvent::Focused(true)]));
    }

    mod controls {
        use super::super::*;
        use std::time::{Duration, Instant};
        use winit::event::TouchPhase::*;

        /// An iPhone in landscape at GUI scale 4: 633 by 292 units.
        fn layout() -> Layout {
            Layout::new(633.0, 292.0)
        }

        fn centre(l: &Layout, button: TouchButton) -> (f32, f32) {
            let (_, a) = l.buttons.iter().find(|(b, _)| *b == button).unwrap();
            (a.x + a.w / 2.0, a.y + a.h / 2.0)
        }

        #[test]
        fn the_buttons_stay_on_screen_and_apart() {
            for (w, h) in [(633.0, 292.0), (512.0, 384.0), (427.0, 240.0)] {
                let l = Layout::new(w, h);
                for (i, (b, a)) in l.buttons.iter().enumerate() {
                    assert!(
                        a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= w && a.y + a.h <= h,
                        "{b:?} off a {w}x{h} screen"
                    );
                    for (other, o) in &l.buttons[i + 1..] {
                        let apart = a.x + a.w <= o.x
                            || o.x + o.w <= a.x
                            || a.y + a.h <= o.y
                            || o.y + o.h <= a.y;
                        assert!(apart, "{b:?} overlaps {other:?} on {w}x{h}");
                    }
                    let bar = l.hotbar;
                    let clear = a.x + a.w <= bar.x || bar.x + bar.w <= a.x || a.y + a.h <= bar.y;
                    assert!(clear, "{b:?} covers the hotbar on {w}x{h}");
                }
                assert!(l.stick_zone.x + l.stick_zone.w <= l.hotbar.x);
            }
        }

        #[test]
        fn a_button_is_held_while_the_finger_is_on_it() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, centre(&l, TouchButton::Jump), now);
            let later = now + Duration::from_secs(1);
            assert_eq!(t.frame(later).held, [TouchButton::Jump]);
            t.touch(&l, 1, Ended, centre(&l, TouchButton::Jump), later);
            assert!(t.frame(later).held.is_empty());
        }

        #[test]
        fn a_quick_button_tap_still_lasts_a_game_tick() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, centre(&l, TouchButton::Pause), now);
            t.touch(&l, 1, Ended, centre(&l, TouchButton::Pause), now);
            assert_eq!(
                t.frame(now + Duration::from_millis(50)).held,
                [TouchButton::Pause]
            );
            assert!(t.frame(now + Duration::from_millis(200)).held.is_empty());
        }

        #[test]
        fn sneak_toggles() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            let at = centre(&l, TouchButton::Sneak);
            t.touch(&l, 1, Started, at, now);
            t.touch(&l, 1, Ended, at, now);
            assert_eq!(
                t.frame(now + Duration::from_secs(5)).held,
                [TouchButton::Sneak]
            );
            t.touch(&l, 2, Started, at, now);
            t.touch(&l, 2, Ended, at, now);
            assert!(t.frame(now + Duration::from_secs(5)).held.is_empty());
        }

        #[test]
        fn the_stick_follows_the_finger_from_where_it_landed() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, (80.0, 200.0), now);
            // Half the radius to the right: half strafe, no forward.
            t.touch(&l, 1, Moved, (80.0 + STICK_RADIUS / 2.0, 200.0), now);
            let f = t.frame(now);
            assert!(
                (f.stick[0] - 0.5).abs() < 1e-5 && f.stick[1] == 0.0,
                "{:?}",
                f.stick
            );
            // Far up: clamped to full forward, which sprints.
            t.touch(&l, 1, Moved, (80.0, 0.0 + 30.0), now);
            let f = t.frame(now);
            assert!((f.stick[1] - 1.0).abs() < 1e-5, "{:?}", f.stick);
            assert_eq!(f.held, [TouchButton::Sprint]);
            t.touch(&l, 1, Ended, (80.0, 30.0), now);
            assert_eq!(t.frame(now).stick, [0.0, 0.0]);
        }

        #[test]
        fn dragging_the_view_turns_it_by_the_distance_dragged() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, (400.0, 100.0), now);
            t.touch(&l, 1, Moved, (410.0, 95.0), now);
            t.touch(&l, 1, Moved, (420.0, 97.0), now);
            assert_eq!(t.frame(now).look, (20.0, -3.0));
            // Taken: the next frame turns no further.
            assert_eq!(t.frame(now).look, (0.0, 0.0));
            // A dragged touch is no tap.
            t.touch(
                &l,
                1,
                Ended,
                (420.0, 97.0),
                now + Duration::from_millis(100),
            );
            assert!(t.frame(now + Duration::from_millis(110)).held.is_empty());
        }

        #[test]
        fn a_quick_tap_on_the_view_uses_and_holding_still_attacks() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, (400.0, 100.0), now);
            t.touch(
                &l,
                1,
                Ended,
                (401.0, 100.0),
                now + Duration::from_millis(100),
            );
            assert_eq!(
                t.frame(now + Duration::from_millis(120)).held,
                [TouchButton::Use]
            );

            let mut t = TouchControls::default();
            t.touch(&l, 2, Started, (400.0, 100.0), now);
            assert!(t.frame(now + Duration::from_millis(100)).held.is_empty());
            let held = now + Duration::from_millis(400);
            assert_eq!(t.frame(held).held, [TouchButton::Attack]);
            // Moving while breaking keeps breaking (and turns the view).
            t.touch(&l, 2, Moved, (450.0, 100.0), held);
            assert_eq!(t.frame(held).held, [TouchButton::Attack]);
            t.touch(&l, 2, Ended, (450.0, 100.0), held);
            assert!(t.frame(held + Duration::from_millis(200)).held.is_empty());
        }

        #[test]
        fn a_tap_on_the_hotbar_picks_that_slot() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            // Slot 4 is the middle one.
            t.touch(&l, 1, Started, (633.0 / 2.0, 292.0 - 10.0), now);
            assert_eq!(t.frame(now).slot, Some(4));
            assert_eq!(t.frame(now).slot, None);
            t.touch(&l, 2, Started, (l.hotbar.x + 2.0, l.hotbar.y + 2.0), now);
            assert_eq!(t.frame(now).slot, Some(0));
            t.touch(
                &l,
                3,
                Started,
                (l.hotbar.x + l.hotbar.w - 2.0, l.hotbar.y + 2.0),
                now,
            );
            assert_eq!(t.frame(now).slot, Some(8));
        }

        #[test]
        fn a_second_finger_in_the_stick_zone_looks() {
            let l = layout();
            let mut t = TouchControls::default();
            let now = Instant::now();
            t.touch(&l, 1, Started, (80.0, 200.0), now);
            t.touch(&l, 2, Started, (120.0, 200.0), now);
            t.touch(&l, 2, Moved, (130.0, 200.0), now);
            let f = t.frame(now);
            assert_eq!(f.stick, [0.0, 0.0]);
            assert_eq!(f.look, (10.0, 0.0));
        }
    }
}
