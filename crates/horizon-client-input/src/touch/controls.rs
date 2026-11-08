//! The on-screen controls while playing: a movement stick fixed on the left,
//! dragging anywhere else turns the view, a quick tap uses (places, eats,
//! opens) and holding still attacks (breaks), plus buttons for jump, sneak,
//! attack, use, drop, inventory, chat, perspective and pause, and a tap on a
//! hotbar slot selects it.
//!
//! Every control can be moved, resized, faded or hidden in the layout editor
//! (the Edit button), like a phone shooter's HUD editor; [`TouchSettings`] is
//! the saved layout. Positions are in GUI units (physical pixels over the GUI
//! scale), the units the HUD is drawn in, so the controls keep their size on
//! every screen, and they stay inside the screen's safe area (clear of the
//! notch and the home bar).
use std::collections::HashMap;
use std::time::{Duration, Instant};
use winit::event::TouchPhase;

/// An on-screen button while playing. The game presses the controller
/// button bound to `action()`, so a rebind in the controller settings moves
/// the touch button with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchButton {
    Jump,
    Sneak,
    Attack,
    Use,
    Drop,
    Inventory,
    Chat,
    Perspective,
    Pause,
    /// Opens the layout editor.
    Edit,
    /// F3: the debug screen.
    Debug,
    /// Held while the stick is pushed forward (kept until it comes back); not on screen.
    Sprint,
}

impl TouchButton {
    /// The buttons on screen, in the order they are laid out.
    pub const PLACED: [TouchButton; 11] = [
        Self::Jump,
        Self::Sneak,
        Self::Attack,
        Self::Use,
        Self::Drop,
        Self::Pause,
        Self::Chat,
        Self::Perspective,
        Self::Edit,
        Self::Debug,
        Self::Inventory,
    ];

    /// The key mapping it stands for; `None` for pause (the controller's
    /// Start), F3 (the keyboard's key, which no controller button has) and
    /// the editor's button.
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
            Self::Pause | Self::Edit | Self::Debug => return None,
        })
    }

    /// Its name in the settings file.
    fn key(self) -> &'static str {
        match self {
            Self::Jump => "jump",
            Self::Sneak => "sneak",
            Self::Attack => "attack",
            Self::Use => "use",
            Self::Drop => "drop",
            Self::Inventory => "inventory",
            Self::Chat => "chat",
            Self::Perspective => "perspective",
            Self::Pause => "pause",
            Self::Edit => "edit",
            Self::Debug => "debug",
            Self::Sprint => "sprint",
        }
    }
}

/// Something the layout editor moves: a button, the movement stick or the hotbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Button(TouchButton),
    Stick,
    Hotbar,
}

impl Control {
    /// Every control, in the settings file's order.
    pub fn all() -> impl Iterator<Item = Control> {
        TouchButton::PLACED
            .into_iter()
            .map(Control::Button)
            .chain([Control::Stick, Control::Hotbar])
    }

    fn key(self) -> &'static str {
        match self {
            Self::Button(b) => b.key(),
            Self::Stick => "stick",
            Self::Hotbar => "hotbar",
        }
    }

    /// The translation key of its name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Button(TouchButton::Jump) => "horizon.touch.jump",
            Self::Button(TouchButton::Sneak) => "horizon.touch.sneak",
            Self::Button(TouchButton::Attack) => "horizon.touch.attack",
            Self::Button(TouchButton::Use) => "horizon.touch.use",
            Self::Button(TouchButton::Drop) => "horizon.touch.drop",
            Self::Button(TouchButton::Inventory) => "horizon.touch.inventory",
            Self::Button(TouchButton::Chat) => "horizon.touch.chat",
            Self::Button(TouchButton::Perspective) => "horizon.touch.perspective",
            Self::Button(TouchButton::Pause) => "horizon.touch.pause",
            Self::Button(TouchButton::Edit) => "horizon.touch.edit",
            Self::Button(TouchButton::Debug) => "horizon.touch.debug",
            Self::Button(TouchButton::Sprint) => "key.sprint",
            Self::Stick => "horizon.touch.stick",
            Self::Hotbar => "horizon.touch.hotbar",
        }
    }

    /// The hotbar keeps vanilla's size.
    pub fn resizable(self) -> bool {
        self != Self::Hotbar
    }

    /// The editor's own button and the hotbar always show.
    pub fn hideable(self) -> bool {
        !matches!(self, Self::Button(TouchButton::Edit) | Self::Hotbar)
    }
}

/// The smallest and largest size, as a multiple of the default.
pub const SIZES: (f32, f32) = (0.5, 2.5);
/// The faintest and fullest opacity.
pub const OPACITIES: (f32, f32) = (0.2, 1.0);
/// One press of a size or opacity button.
const STEP: f32 = 0.1;

/// Where one control is and how it looks; the default is where the layout
/// puts it, at its own size, fully opaque, shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Its centre as a fraction of the safe area (0 to 1 across, 0 to 1
    /// down); `None` is the default position.
    pub centre: Option<(f32, f32)>,
    pub size: f32,
    pub opacity: f32,
    pub hidden: bool,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            centre: None,
            size: 1.0,
            opacity: 1.0,
            hidden: false,
        }
    }
}

/// The saved layout: each control's placement where it differs from the default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchSettings {
    placements: HashMap<Control, Placement>,
}

impl TouchSettings {
    pub fn placement(&self, control: Control) -> Placement {
        self.placements.get(&control).copied().unwrap_or_default()
    }

    pub fn set(&mut self, control: Control, placement: Placement) {
        if placement == Placement::default() {
            self.placements.remove(&control);
        } else {
            self.placements.insert(control, placement);
        }
    }

    /// Read the settings file: one `name=x,y,size,opacity,hidden` line per
    /// changed control (`x,y` as `-,-` for the default position). Lines it
    /// can't read are skipped, so an old or hand-edited file still loads.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((name, value)) = line.trim().split_once('=') else {
                continue;
            };
            let Some(control) = Control::all().find(|c| c.key() == name.trim()) else {
                continue;
            };
            let parts: Vec<&str> = value.split(',').map(str::trim).collect();
            let [x, y, size, opacity, hidden] = parts[..] else {
                continue;
            };
            let number = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite());
            let centre = match (number(x), number(y)) {
                (Some(x), Some(y)) => Some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))),
                _ => None,
            };
            let (Some(size), Some(opacity)) = (number(size), number(opacity)) else {
                continue;
            };
            settings.set(
                control,
                Placement {
                    centre,
                    size: size.clamp(SIZES.0, SIZES.1),
                    opacity: opacity.clamp(OPACITIES.0, OPACITIES.1),
                    hidden: hidden == "1" && control.hideable(),
                },
            );
        }
        settings
    }

    /// The settings file's text ([`parse`](Self::parse) reads it back).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for control in Control::all() {
            let Some(p) = self.placements.get(&control) else {
                continue;
            };
            let (x, y) = match p.centre {
                Some((x, y)) => (format!("{x:.4}"), format!("{y:.4}")),
                None => ("-".to_owned(), "-".to_owned()),
            };
            out.push_str(&format!(
                "{}={x},{y},{:.2},{:.2},{}\n",
                control.key(),
                p.size,
                p.opacity,
                u8::from(p.hidden)
            ));
        }
        out
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

    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    fn around((cx, cy): (f32, f32), w: f32, h: f32) -> Self {
        Self {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
        }
    }

    /// Moved to lie within `bounds` (as far as it fits).
    fn kept_in(self, bounds: Area) -> Self {
        Self {
            x: self.x.min(bounds.x + bounds.w - self.w).max(bounds.x),
            y: self.y.min(bounds.y + bounds.h - self.h).max(bounds.y),
            ..self
        }
    }
}

/// Vanilla's hotbar: 182 by 22 units, centred at the bottom, slots 20 apart.
pub const HOTBAR_WIDTH: f32 = 182.0;
pub const HOTBAR_HEIGHT: f32 = 22.0;
/// How far the stick's knob travels from where the finger came down, at size 1.
pub const STICK_RADIUS: f32 = 26.0;
/// A big button's side and a small one's, at size 1.
const BIG: f32 = 30.0;
const SMALL: f32 = 22.0;
const GAP: f32 = 4.0;
/// A finger that moved less than this is still a tap or a hold.
const STILL: f32 = 6.0;
/// Shorter than this, a touch on the view is a tap (use).
const TAP: Duration = Duration::from_millis(180);
/// Held still this long, a touch on the view attacks (breaks a block).
const HOLD: Duration = Duration::from_millis(220);
/// A tapped button stays down this long: the game samples keys once a tick (50 ms).
const PULSE: Duration = Duration::from_millis(60);
/// Pushed this far forward, the stick starts sprinting; it keeps sprinting until
/// the stick comes back below `SPRINT_OFF` (or the finger lifts), so a thumb
/// that relaxes a little does not stop the run.
const SPRINT_ON: f32 = 0.8;
const SPRINT_OFF: f32 = 0.3;

/// One control as laid out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub control: Control,
    /// A button's face; the stick's area (where a finger becomes the
    /// stick); the hotbar's 182 by 22.
    pub area: Area,
    pub opacity: f32,
    pub hidden: bool,
}

/// Where the controls are on a screen `width` by `height` GUI units.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    /// The part of the screen clear of the notch and the home bar.
    pub safe: Area,
    pub items: Vec<Item>,
    /// How far the stick's knob travels.
    pub stick_radius: f32,
    /// Where the stick rests while no finger holds it.
    pub stick_home: (f32, f32),
}

impl Layout {
    /// The layout with every control where `settings` puts it.
    pub fn new(width: f32, height: f32, safe: Area, settings: &TouchSettings) -> Self {
        let mut layout = Self::defaults(width, height, safe);
        let screen = Area {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        };
        for item in &mut layout.items {
            let p = settings.placement(item.control);
            let size = if item.control.resizable() {
                p.size
            } else {
                1.0
            };
            let centre = p.centre.map_or(item.area.centre(), |(fx, fy)| {
                (safe.x + fx * safe.w, safe.y + fy * safe.h)
            });
            let (w, h) = match item.control {
                // The stick is a fixed square around its home; a finger that
                // comes down in it is the stick, measured from that home.
                Control::Stick => {
                    let side = STICK_RADIUS * 4.0 * size;
                    (side, side)
                }
                _ => (item.area.w * size, item.area.h * size),
            };
            item.area = Area::around(centre, w, h).kept_in(screen);
            item.opacity = p.opacity;
            item.hidden = p.hidden;
            if item.control == Control::Stick {
                layout.stick_radius = STICK_RADIUS * size;
                layout.stick_home = item.area.centre();
            }
        }
        layout
    }

    /// The default layout: action buttons in the bottom right, pause, chat,
    /// view and edit along the top right, the inventory beside the hotbar,
    /// the stick on the bottom left.
    fn defaults(width: f32, height: f32, safe: Area) -> Self {
        let right = safe.x + safe.w;
        let bottom = safe.y + safe.h;
        let col = |i: f32| right - GAP - BIG - i * (BIG + GAP);
        let row = |i: f32| bottom - GAP - BIG - i * (BIG + GAP);
        let big = |x, y| Area {
            x,
            y,
            w: BIG,
            h: BIG,
        };
        let top = |i: f32| Area {
            x: right - GAP - SMALL - i * (SMALL + GAP),
            y: safe.y + GAP,
            w: SMALL,
            h: SMALL,
        };
        let hotbar = Area {
            x: ((width - HOTBAR_WIDTH) / 2.0).floor(),
            y: bottom - HOTBAR_HEIGHT,
            w: HOTBAR_WIDTH,
            h: HOTBAR_HEIGHT,
        };
        // The stick sits at a fixed home in the bottom left; a finger that
        // comes down within twice its radius of it is the stick.
        let half = STICK_RADIUS * 2.0;
        let home = (safe.x + GAP * 3.0 + half, bottom - GAP - half);
        let stick = Area::around(home, half * 2.0, half * 2.0);
        let area = |control: Control| match control {
            Control::Button(TouchButton::Jump) => big(col(0.0), row(0.0)),
            Control::Button(TouchButton::Sneak) => big(col(1.0), row(0.0)),
            Control::Button(TouchButton::Attack) => big(col(0.0), row(1.0)),
            Control::Button(TouchButton::Use) => big(col(1.0), row(1.0)),
            Control::Button(TouchButton::Drop) => big(col(0.0), row(2.0)),
            Control::Button(TouchButton::Pause) => top(0.0),
            Control::Button(TouchButton::Chat) => top(1.0),
            Control::Button(TouchButton::Perspective) => top(2.0),
            Control::Button(TouchButton::Edit) => top(3.0),
            Control::Button(TouchButton::Debug) => top(4.0),
            Control::Button(TouchButton::Inventory | TouchButton::Sprint) => Area {
                x: hotbar.x + hotbar.w + GAP,
                y: hotbar.y,
                w: SMALL,
                h: SMALL,
            },
            Control::Stick => stick,
            Control::Hotbar => hotbar,
        };
        let items = Control::all()
            .map(|control| Item {
                control,
                area: area(control),
                opacity: 1.0,
                hidden: false,
            })
            .collect();
        Self {
            width,
            height,
            safe,
            items,
            stick_radius: STICK_RADIUS,
            stick_home: home,
        }
    }

    pub fn item(&self, control: Control) -> &Item {
        self.items
            .iter()
            .find(|i| i.control == control)
            .expect("every control is laid out")
    }

    pub fn hotbar(&self) -> Area {
        self.item(Control::Hotbar).area
    }

    /// How far the hotbar was moved from vanilla's place (the game draws
    /// the hotbar, hearts, food and experience bar that much over).
    pub fn hotbar_offset(&self) -> (f32, f32) {
        let hotbar = self.hotbar();
        (
            hotbar.x - ((self.width - HOTBAR_WIDTH) / 2.0).floor(),
            hotbar.y - (self.height - HOTBAR_HEIGHT),
        )
    }

    /// The shown button under `at`, if any.
    pub fn button_at(&self, at: (f32, f32)) -> Option<TouchButton> {
        self.items.iter().find_map(|i| match i.control {
            Control::Button(b) if !i.hidden && i.area.contains(at) => Some(b),
            _ => None,
        })
    }

    /// The hotbar slot (0 to 8) under `at`.
    pub fn slot_at(&self, at: (f32, f32)) -> Option<usize> {
        let hotbar = self.hotbar();
        hotbar
            .contains(at)
            .then(|| (((at.0 - hotbar.x - 1.0) / 20.0).floor().max(0.0) as usize).min(8))
    }

    /// The control the editor picks at `at`, shown or hidden: a button over
    /// the hotbar over the stick's area.
    pub fn control_at(&self, at: (f32, f32)) -> Option<Control> {
        let rank = |c: Control| match c {
            Control::Button(_) => 0,
            Control::Hotbar => 1,
            Control::Stick => 2,
        };
        self.items
            .iter()
            .filter(|i| i.area.contains(at))
            .min_by_key(|i| rank(i.control))
            .map(|i| i.control)
    }
}

/// The layout editor's toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorButton {
    Smaller,
    Bigger,
    Fainter,
    Stronger,
    Hide,
    Reset,
    Done,
}

impl EditorButton {
    pub const ALL: [EditorButton; 7] = [
        Self::Smaller,
        Self::Bigger,
        Self::Fainter,
        Self::Stronger,
        Self::Hide,
        Self::Reset,
        Self::Done,
    ];

    /// The translation key of its label; `hidden`: the selected control is hidden.
    pub fn label(self, hidden: bool) -> &'static str {
        match self {
            Self::Smaller => "horizon.touch.edit.smaller",
            Self::Bigger => "horizon.touch.edit.bigger",
            Self::Fainter => "horizon.touch.edit.fainter",
            Self::Stronger => "horizon.touch.edit.stronger",
            Self::Hide if hidden => "horizon.touch.edit.show",
            Self::Hide => "horizon.touch.edit.hide",
            Self::Reset => "horizon.touch.edit.reset",
            Self::Done => "horizon.touch.edit.done",
        }
    }

    /// The toolbar across the safe area, centred, below the top row of buttons.
    pub fn toolbar(layout: &Layout) -> Vec<(EditorButton, Area)> {
        let (w, h) = (56.0, 18.0);
        let total = Self::ALL.len() as f32 * (w + GAP) - GAP;
        let x0 = (layout.safe.x + (layout.safe.w - total) / 2.0).max(layout.safe.x);
        Self::ALL
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let area = Area {
                    x: x0 + i as f32 * (w + GAP),
                    y: layout.safe.y + GAP + SMALL + GAP * 2.0,
                    w,
                    h,
                };
                (*b, area)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    finger: u64,
    control: Control,
    /// The finger's place less the control's centre when it was picked up.
    grab: (f32, f32),
}

#[derive(Clone, Debug)]
struct Editor {
    draft: TouchSettings,
    selected: Option<Control>,
    drag: Option<Drag>,
}

/// What the editor shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorView {
    pub selected: Option<Control>,
    /// The selected control's placement.
    pub placement: Placement,
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

/// The on-screen controls while playing, and their layout editor.
#[derive(Default, Debug)]
pub struct TouchControls {
    /// The layout in use (loaded from the settings file).
    pub settings: TouchSettings,
    editor: Option<Editor>,
    /// A layout the editor finished, for the game to save.
    saved: Option<TouchSettings>,
    fingers: HashMap<u64, Role>,
    /// The stick is sprinting (`SPRINT_ON` reached, `SPRINT_OFF` not yet).
    sprinting: bool,
    /// The game's Sneak option is Toggle: it flips sneaking on each press, so
    /// the game's own state (not the finger) says whether the button is down.
    /// With Hold the button is down while the finger is. Either way the
    /// button only reports the finger; the game applies the option. Set by
    /// the game.
    pub sneak_taps: bool,
    /// Buttons tapped and held for a pulse, until then.
    pulses: Vec<(TouchButton, Instant)>,
    look: (f32, f32),
    slot: Option<usize>,
}

impl TouchControls {
    /// The controls laid out on a screen `width` by `height` GUI units: the
    /// editor's draft while it is open.
    pub fn layout(&self, width: f32, height: f32, safe: Area) -> Layout {
        let settings = self.editor.as_ref().map_or(&self.settings, |e| &e.draft);
        Layout::new(width, height, safe, settings)
    }

    pub fn editing(&self) -> bool {
        self.editor.is_some()
    }

    pub fn editor(&self) -> Option<EditorView> {
        self.editor.as_ref().map(|e| EditorView {
            selected: e.selected,
            placement: e.selected.map(|c| e.draft.placement(c)).unwrap_or_default(),
        })
    }

    /// A layout the editor's Done just kept, to write to the settings file.
    pub fn take_saved(&mut self) -> Option<TouchSettings> {
        self.saved.take()
    }

    /// A finger came down, moved, lifted or was cancelled at `at`.
    pub fn touch(
        &mut self,
        layout: &Layout,
        id: u64,
        phase: TouchPhase,
        at: (f32, f32),
        now: Instant,
    ) {
        if self.editor.is_some() {
            self.edit_touch(layout, id, phase, at);
            return;
        }
        use TouchPhase::*;
        match phase {
            Started => {
                let role = if let Some(button) = layout.button_at(at) {
                    match button {
                        TouchButton::Edit => {
                            self.release_all();
                            self.editor = Some(Editor {
                                draft: self.settings.clone(),
                                selected: None,
                                drag: None,
                            });
                            return;
                        }
                        _ => self.pulse(button, now),
                    }
                    Role::Button(button)
                } else if let Some(slot) = layout.slot_at(at) {
                    self.slot = Some(slot);
                    Role::Hotbar
                } else if layout.item(Control::Stick).area.contains(at)
                    && !layout.item(Control::Stick).hidden
                    && !self
                        .fingers
                        .values()
                        .any(|r| matches!(r, Role::Stick { .. }))
                {
                    Role::Stick {
                        origin: layout.stick_home,
                        at,
                    }
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

    /// A touch while the editor is open: the toolbar, or picking up and
    /// dragging a control.
    fn edit_touch(&mut self, layout: &Layout, id: u64, phase: TouchPhase, at: (f32, f32)) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        match phase {
            TouchPhase::Started => {
                if let Some((button, _)) = EditorButton::toolbar(layout)
                    .into_iter()
                    .find(|(_, area)| area.contains(at))
                {
                    self.editor_button(button);
                    return;
                }
                editor.selected = layout.control_at(at);
                editor.drag = editor.selected.map(|control| {
                    let (cx, cy) = layout.item(control).area.centre();
                    Drag {
                        finger: id,
                        control,
                        grab: (at.0 - cx, at.1 - cy),
                    }
                });
            }
            TouchPhase::Moved => {
                let Some(drag) = editor.drag.filter(|d| d.finger == id) else {
                    return;
                };
                let safe = layout.safe;
                if safe.w <= 0.0 || safe.h <= 0.0 {
                    return;
                }
                let (cx, cy) = (at.0 - drag.grab.0, at.1 - drag.grab.1);
                let mut p = editor.draft.placement(drag.control);
                p.centre = Some((
                    ((cx - safe.x) / safe.w).clamp(0.0, 1.0),
                    ((cy - safe.y) / safe.h).clamp(0.0, 1.0),
                ));
                editor.draft.set(drag.control, p);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if editor.drag.is_some_and(|d| d.finger == id) {
                    editor.drag = None;
                }
            }
        }
    }

    /// A toolbar button: it changes the selected control, or every button
    /// when none is selected (Hide needs one).
    fn editor_button(&mut self, button: EditorButton) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if button == EditorButton::Done {
            let draft = editor.draft.clone();
            self.settings = draft.clone();
            self.saved = Some(draft);
            self.editor = None;
            return;
        }
        let targets: Vec<Control> = match editor.selected {
            Some(c) => vec![c],
            None if button == EditorButton::Hide => Vec::new(),
            None => Control::all().collect(),
        };
        for control in targets {
            let mut p = editor.draft.placement(control);
            match button {
                EditorButton::Smaller if control.resizable() => {
                    p.size = step(p.size, -STEP, SIZES);
                }
                EditorButton::Bigger if control.resizable() => {
                    p.size = step(p.size, STEP, SIZES);
                }
                EditorButton::Fainter => p.opacity = step(p.opacity, -STEP, OPACITIES),
                EditorButton::Stronger => p.opacity = step(p.opacity, STEP, OPACITIES),
                EditorButton::Hide if control.hideable() => p.hidden = !p.hidden,
                EditorButton::Reset => p = Placement::default(),
                _ => {}
            }
            editor.draft.set(control, p);
        }
    }

    fn pulse(&mut self, button: TouchButton, now: Instant) {
        self.pulses.push((button, now + PULSE));
    }

    /// Lets go of everything (a menu opened, the app went to the background).
    pub fn release_all(&mut self) {
        self.fingers.clear();
        self.pulses.clear();
        self.sprinting = false;
        self.look = (0.0, 0.0);
        self.slot = None;
        if let Some(editor) = &mut self.editor {
            editor.drag = None;
        }
    }

    /// Whether any finger is on the controls.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }

    /// The stick's centre and its knob while a finger holds it, for drawing.
    pub fn stick(&self, layout: &Layout) -> Option<((f32, f32), (f32, f32))> {
        self.fingers.values().find_map(|r| match *r {
            Role::Stick { origin, at } => Some((origin, knob(origin, at, layout.stick_radius))),
            _ => None,
        })
    }

    /// Whether a finger is on `button` now, for drawing it pressed (Sneak is
    /// drawn from the game's own state instead).
    pub fn pressed(&self, button: TouchButton) -> bool {
        self.fingers
            .values()
            .any(|r| matches!(r, Role::Button(b) if *b == button))
    }

    /// The state to apply this frame; the look and the tapped slot are
    /// taken. Nothing while the editor is open.
    pub fn frame(&mut self, layout: &Layout, now: Instant) -> TouchFrame {
        if self.editor.is_some() {
            return TouchFrame::default();
        }
        self.pulses.retain(|(_, until)| *until > now);
        let mut held: Vec<TouchButton> = self.pulses.iter().map(|(b, _)| *b).collect();
        let mut stick = [0.0; 2];
        let mut stick_down = false;
        let radius = layout.stick_radius;
        for role in self.fingers.values_mut() {
            match role {
                Role::Button(b) if *b != TouchButton::Edit => {
                    held.push(*b);
                }
                Role::Stick { origin, at } => {
                    let k = knob(*origin, *at, radius);
                    stick = [(k.0 - origin.0) / radius, -(k.1 - origin.1) / radius];
                    stick_down = true;
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
        if !stick_down || stick[1] < SPRINT_OFF {
            self.sprinting = false;
        } else if stick[1] >= SPRINT_ON {
            self.sprinting = true;
        }
        if self.sprinting {
            held.push(TouchButton::Sprint);
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

/// `value` moved by `by`, kept within `bounds`, rounded to the step.
fn step(value: f32, by: f32, bounds: (f32, f32)) -> f32 {
    (((value + by) * 10.0).round() / 10.0).clamp(bounds.0, bounds.1)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// `at`, held within `radius` of `origin`.
fn knob(origin: (f32, f32), at: (f32, f32), radius: f32) -> (f32, f32) {
    let d = distance(origin, at);
    if d <= radius {
        return at;
    }
    let k = radius / d;
    (
        origin.0 + (at.0 - origin.0) * k,
        origin.1 + (at.1 - origin.1) * k,
    )
}

#[cfg(test)]
mod tests;
