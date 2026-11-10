//! The on-screen controls while playing: a movement stick fixed on the left,
//! dragging anywhere else turns the view, a quick tap uses (places, eats,
//! opens) and holding still attacks (breaks), plus buttons for jump, sneak,
//! attack, use, drop, inventory, chat, perspective and pause, and a tap on a
//! hotbar slot selects it.
//!
//! Every control can be moved, resized, faded or hidden in the layout editor
//! (Options, Controls, Custom Controls), like a phone shooter's HUD editor;
//! holding a finger down in the editor adds a button, copies one or binds a
//! keyboard key to one, as a keybind on a PC (a mod's key, say);
//! [`TouchSettings`] is the saved layout. Positions are in GUI units (physical pixels over the GUI
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
    /// F3: the debug screen.
    Debug,
    /// Held while the stick is pushed forward (kept until it comes back); not on screen.
    Sprint,
}

impl TouchButton {
    /// The buttons on screen, in the order they are laid out.
    pub const PLACED: [TouchButton; 10] = [
        Self::Jump,
        Self::Sneak,
        Self::Attack,
        Self::Use,
        Self::Drop,
        Self::Pause,
        Self::Chat,
        Self::Perspective,
        Self::Debug,
        Self::Inventory,
    ];

    /// The key mapping it stands for; `None` for pause (the controller's
    /// Start) and F3 (the keyboard's key, which no controller button has).
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
            Self::Pause | Self::Debug => return None,
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
            Self::Debug => "debug",
            Self::Sprint => "sprint",
        }
    }

    /// The button called `name` in the settings file.
    fn from_key(name: &str) -> Option<Self> {
        Self::PLACED.into_iter().find(|b| b.key() == name)
    }
}

/// What a button the player added presses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bind {
    /// Nothing yet: it waits for a key.
    Unbound,
    /// A keyboard key (`key.keyboard.f7`), pressed as the keyboard's own: a
    /// mod that has a keybind on it hears it.
    Key(String),
    /// What a built-in button does: a copy of it.
    Button(TouchButton),
}

impl Bind {
    fn to_text(&self) -> String {
        match self {
            Self::Unbound => "-".to_owned(),
            Self::Key(name) => name.clone(),
            Self::Button(b) => format!("button:{}", b.key()),
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text == "-" {
            return Some(Self::Unbound);
        }
        if let Some(name) = text.strip_prefix("button:") {
            return TouchButton::from_key(name).map(Self::Button);
        }
        crate::keybinds::key_code(text).map(|_| Self::Key(text.to_owned()))
    }
}

/// A button the player added in the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomButton {
    pub id: u32,
    pub bind: Bind,
}

/// The most buttons a player can add.
pub const MAX_CUSTOM: usize = 24;

/// Something the layout editor moves: a button, the movement stick or the hotbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Button(TouchButton),
    /// A button the player added (its id).
    Custom(u32),
    Stick,
    Hotbar,
}

impl Control {
    /// Every built-in control, in the settings file's order (the player's
    /// own buttons come after them, from the settings).
    pub fn all() -> impl Iterator<Item = Control> {
        TouchButton::PLACED
            .into_iter()
            .map(Control::Button)
            .chain([Control::Stick, Control::Hotbar])
    }

    fn key(self) -> &'static str {
        match self {
            Self::Button(b) => b.key(),
            Self::Custom(_) => "custom",
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
            Self::Button(TouchButton::Debug) => "horizon.touch.debug",
            Self::Button(TouchButton::Sprint) => "key.sprint",
            Self::Custom(_) => "horizon.touch.custom",
            Self::Stick => "horizon.touch.stick",
            Self::Hotbar => "horizon.touch.hotbar",
        }
    }

    /// The hotbar keeps vanilla's size.
    pub fn resizable(self) -> bool {
        self != Self::Hotbar
    }

    /// The hotbar always shows.
    pub fn hideable(self) -> bool {
        self != Self::Hotbar
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
    /// The buttons the player added, in the order they were made.
    customs: Vec<CustomButton>,
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

    /// The buttons the player added.
    pub fn customs(&self) -> &[CustomButton] {
        &self.customs
    }

    pub fn custom(&self, id: u32) -> Option<&CustomButton> {
        self.customs.iter().find(|c| c.id == id)
    }

    /// A new button pressing `bind`, placed by `placement`; its id, or `None`
    /// when there are already `MAX_CUSTOM`.
    pub fn add_custom(&mut self, bind: Bind, placement: Placement) -> Option<u32> {
        if self.customs.len() >= MAX_CUSTOM {
            return None;
        }
        let id = self.customs.iter().map(|c| c.id + 1).max().unwrap_or(0);
        self.customs.push(CustomButton { id, bind });
        self.set(Control::Custom(id), placement);
        Some(id)
    }

    pub fn remove_custom(&mut self, id: u32) {
        self.customs.retain(|c| c.id != id);
        self.placements.remove(&Control::Custom(id));
    }

    pub fn set_bind(&mut self, id: u32, bind: Bind) {
        if let Some(c) = self.customs.iter_mut().find(|c| c.id == id) {
            c.bind = bind;
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
            if let Some(id) = name
                .trim()
                .strip_prefix("custom")
                .and_then(|n| n.parse::<u32>().ok())
            {
                settings.parse_custom(id, value);
                continue;
            }
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

    /// One `customN=bind,x,y,size,opacity,hidden` line.
    fn parse_custom(&mut self, id: u32, value: &str) {
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        let [bind, x, y, size, opacity, hidden] = parts[..] else {
            return;
        };
        let number = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite());
        let (Some(bind), Some(size), Some(opacity)) =
            (Bind::parse(bind), number(size), number(opacity))
        else {
            return;
        };
        if self.custom(id).is_some() || self.customs.len() >= MAX_CUSTOM {
            return;
        }
        let centre = match (number(x), number(y)) {
            (Some(x), Some(y)) => Some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))),
            _ => None,
        };
        self.customs.push(CustomButton { id, bind });
        self.set(
            Control::Custom(id),
            Placement {
                centre,
                size: size.clamp(SIZES.0, SIZES.1),
                opacity: opacity.clamp(OPACITIES.0, OPACITIES.1),
                hidden: hidden == "1",
            },
        );
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
        for c in &self.customs {
            let p = self.placement(Control::Custom(c.id));
            let (x, y) = match p.centre {
                Some((x, y)) => (format!("{x:.4}"), format!("{y:.4}")),
                None => ("-".to_owned(), "-".to_owned()),
            };
            out.push_str(&format!(
                "custom{}={},{x},{y},{:.2},{:.2},{}\n",
                c.id,
                c.bind.to_text(),
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
/// Held still this long in the editor, a finger opens its menu.
const LONG: Duration = Duration::from_millis(500);
/// A copied button lies this far (GUI units) down and right of the original.
const COPY_OFFSET: f32 = 10.0;
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
        // The player's buttons start in the middle of the screen until placed.
        let middle = (safe.x + safe.w / 2.0, safe.y + safe.h / 2.0);
        for c in settings.customs() {
            layout.items.push(Item {
                control: Control::Custom(c.id),
                area: Area::around(middle, BIG, BIG),
                opacity: 1.0,
                hidden: false,
            });
        }
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
    /// view and F3 along the top right, the inventory beside the hotbar,
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
            Control::Button(TouchButton::Debug) => top(3.0),
            Control::Button(TouchButton::Inventory | TouchButton::Sprint) => Area {
                x: hotbar.x + hotbar.w + GAP,
                y: hotbar.y,
                w: SMALL,
                h: SMALL,
            },
            Control::Stick => stick,
            Control::Hotbar => hotbar,
            // The player's own buttons are laid out by `Layout::new`.
            Control::Custom(_) => big(safe.x, safe.y),
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

    /// The shown button of the player's under `at`, if any (the last one
    /// added is on top).
    pub fn custom_at(&self, at: (f32, f32)) -> Option<u32> {
        self.items.iter().rev().find_map(|i| match i.control {
            Control::Custom(id) if !i.hidden && i.area.contains(at) => Some(id),
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
            Control::Custom(_) => 0,
            Control::Button(_) => 1,
            Control::Hotbar => 2,
            Control::Stick => 3,
        };
        self.items
            .iter()
            .rev()
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

/// A finger that came down in the editor, for the long press.
#[derive(Clone, Copy, Debug)]
struct Press {
    finger: u64,
    at: (f32, f32),
    since: Instant,
    /// It moved more than `STILL`: a drag, not a long press.
    moved: bool,
}

/// What a long press in the editor offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    /// A new button where the finger is, to bind a key to.
    Add,
    /// A copy of the control the finger is on.
    Copy,
    /// Bind a key to the player's button the finger is on (a new button
    /// where the finger is, when it is on none).
    Keybind,
    /// Remove the player's button the finger is on.
    Delete,
}

impl MenuItem {
    pub const ALL: [MenuItem; 4] = [Self::Add, Self::Copy, Self::Keybind, Self::Delete];

    /// The translation key of its label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "horizon.touch.menu.add",
            Self::Copy => "horizon.touch.menu.copy",
            Self::Keybind => "horizon.touch.menu.keybind",
            Self::Delete => "horizon.touch.menu.delete",
        }
    }

    /// Whether it can be used on `target` (the control the finger was on).
    pub fn enabled(self, target: Option<Control>) -> bool {
        match self {
            Self::Add | Self::Keybind => true,
            Self::Copy => matches!(target, Some(Control::Button(_) | Control::Custom(_))),
            Self::Delete => matches!(target, Some(Control::Custom(_))),
        }
    }
}

const MENU_W: f32 = 96.0;
const MENU_H: f32 = 18.0;

/// The menu a long press opened, at where the finger was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorMenu {
    pub at: (f32, f32),
    /// The control the finger was on.
    pub target: Option<Control>,
}

impl EditorMenu {
    /// Its items, each with its area and whether it can be used, kept in
    /// the safe area.
    pub fn items(&self, layout: &Layout) -> Vec<(MenuItem, Area, bool)> {
        let total = MenuItem::ALL.len() as f32 * MENU_H;
        let x = self
            .at
            .0
            .min(layout.safe.x + layout.safe.w - MENU_W)
            .max(layout.safe.x);
        let y = self
            .at
            .1
            .min(layout.safe.y + layout.safe.h - total)
            .max(layout.safe.y);
        MenuItem::ALL
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let area = Area {
                    x,
                    y: y + i as f32 * MENU_H,
                    w: MENU_W,
                    h: MENU_H,
                };
                (*item, area, item.enabled(self.target))
            })
            .collect()
    }
}

/// The keys a button can be bound to, as `key.keyboard.*` names.
pub const PICKER_KEYS: &[&str] = &[
    "key.keyboard.a",
    "key.keyboard.b",
    "key.keyboard.c",
    "key.keyboard.d",
    "key.keyboard.e",
    "key.keyboard.f",
    "key.keyboard.g",
    "key.keyboard.h",
    "key.keyboard.i",
    "key.keyboard.j",
    "key.keyboard.k",
    "key.keyboard.l",
    "key.keyboard.m",
    "key.keyboard.n",
    "key.keyboard.o",
    "key.keyboard.p",
    "key.keyboard.q",
    "key.keyboard.r",
    "key.keyboard.s",
    "key.keyboard.t",
    "key.keyboard.u",
    "key.keyboard.v",
    "key.keyboard.w",
    "key.keyboard.x",
    "key.keyboard.y",
    "key.keyboard.z",
    "key.keyboard.1",
    "key.keyboard.2",
    "key.keyboard.3",
    "key.keyboard.4",
    "key.keyboard.5",
    "key.keyboard.6",
    "key.keyboard.7",
    "key.keyboard.8",
    "key.keyboard.9",
    "key.keyboard.0",
    "key.keyboard.f1",
    "key.keyboard.f2",
    "key.keyboard.f3",
    "key.keyboard.f4",
    "key.keyboard.f5",
    "key.keyboard.f6",
    "key.keyboard.f7",
    "key.keyboard.f8",
    "key.keyboard.f9",
    "key.keyboard.f10",
    "key.keyboard.f11",
    "key.keyboard.f12",
    "key.keyboard.space",
    "key.keyboard.tab",
    "key.keyboard.enter",
    "key.keyboard.backspace",
    "key.keyboard.escape",
    "key.keyboard.left.shift",
    "key.keyboard.left.control",
    "key.keyboard.left.alt",
    "key.keyboard.up",
    "key.keyboard.down",
    "key.keyboard.left",
    "key.keyboard.right",
    "key.keyboard.insert",
    "key.keyboard.delete",
    "key.keyboard.home",
    "key.keyboard.end",
    "key.keyboard.page.up",
    "key.keyboard.page.down",
    "key.keyboard.minus",
    "key.keyboard.equal",
    "key.keyboard.left.bracket",
    "key.keyboard.right.bracket",
    "key.keyboard.backslash",
    "key.keyboard.semicolon",
    "key.keyboard.apostrophe",
    "key.keyboard.grave.accent",
    "key.keyboard.comma",
    "key.keyboard.period",
    "key.keyboard.slash",
    "key.keyboard.caps.lock",
    "key.keyboard.keypad.0",
    "key.keyboard.keypad.1",
    "key.keyboard.keypad.2",
    "key.keyboard.keypad.3",
    "key.keyboard.keypad.4",
    "key.keyboard.keypad.5",
    "key.keyboard.keypad.6",
    "key.keyboard.keypad.7",
    "key.keyboard.keypad.8",
    "key.keyboard.keypad.9",
    "key.keyboard.keypad.add",
    "key.keyboard.keypad.subtract",
    "key.keyboard.keypad.multiply",
    "key.keyboard.keypad.divide",
    "key.keyboard.keypad.enter",
    "key.keyboard.keypad.period",
    "key.keyboard.f13",
    "key.keyboard.f14",
    "key.keyboard.f15",
    "key.keyboard.f16",
    "key.keyboard.f17",
    "key.keyboard.f18",
    "key.keyboard.f19",
    "key.keyboard.f20",
    "key.keyboard.f21",
    "key.keyboard.f22",
    "key.keyboard.f23",
    "key.keyboard.f24",
];

const KEY_W: f32 = 34.0;
const KEY_H: f32 = 20.0;
const KEY_GAP: f32 = 3.0;
/// The picker's title line above the keys.
const PICKER_HEAD: f32 = 18.0;

/// What a touch in the key picker can land on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerHit {
    Key(&'static str),
    Prev,
    Next,
    /// Unbind the button.
    Clear,
    Cancel,
}

/// One page of the key picker, as laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerGrid {
    pub cells: Vec<(PickerHit, Area)>,
    pub pages: usize,
    /// The page shown (the asked page kept within `pages`).
    pub page: usize,
}

/// The key picker a Keybind opened, for the player's button `id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picker {
    pub id: u32,
    pub page: usize,
}

impl Picker {
    /// The keys of the page and the buttons under them, over the whole safe
    /// area.
    pub fn grid(&self, layout: &Layout) -> PickerGrid {
        let safe = layout.safe;
        let foot_h = KEY_H;
        let avail_h = (safe.h - PICKER_HEAD - foot_h - KEY_GAP * 2.0).max(KEY_H);
        let cols = (((safe.w + KEY_GAP) / (KEY_W + KEY_GAP)).floor() as usize).max(1);
        let rows = (((avail_h + KEY_GAP) / (KEY_H + KEY_GAP)).floor() as usize).max(1);
        let per_page = cols * rows;
        let pages = PICKER_KEYS.len().div_ceil(per_page).max(1);
        let page = self.page.min(pages - 1);
        let grid_w = cols as f32 * (KEY_W + KEY_GAP) - KEY_GAP;
        let x0 = safe.x + (safe.w - grid_w) / 2.0;
        let y0 = safe.y + PICKER_HEAD;
        let mut cells = Vec::new();
        for (n, name) in PICKER_KEYS.iter().enumerate().skip(page * per_page).take(per_page) {
            let i = n - page * per_page;
            cells.push((
                PickerHit::Key(*name),
                Area {
                    x: x0 + (i % cols) as f32 * (KEY_W + KEY_GAP),
                    y: y0 + (i / cols) as f32 * (KEY_H + KEY_GAP),
                    w: KEY_W,
                    h: KEY_H,
                },
            ));
        }
        let foot = [
            PickerHit::Prev,
            PickerHit::Next,
            PickerHit::Clear,
            PickerHit::Cancel,
        ];
        let foot_w = 56.0;
        let total = foot.len() as f32 * (foot_w + KEY_GAP) - KEY_GAP;
        let fx = safe.x + (safe.w - total) / 2.0;
        let fy = safe.y + safe.h - foot_h;
        for (i, hit) in foot.into_iter().enumerate() {
            cells.push((
                hit,
                Area {
                    x: fx + i as f32 * (foot_w + KEY_GAP),
                    y: fy,
                    w: foot_w,
                    h: foot_h,
                },
            ));
        }
        PickerGrid { cells, pages, page }
    }
}

#[derive(Clone, Debug)]
struct Editor {
    draft: TouchSettings,
    selected: Option<Control>,
    drag: Option<Drag>,
    press: Option<Press>,
    menu: Option<EditorMenu>,
    picker: Option<Picker>,
}

/// What the editor shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorView {
    pub selected: Option<Control>,
    /// The selected control's placement.
    pub placement: Placement,
    /// The long press's menu, while it is open.
    pub menu: Option<EditorMenu>,
    /// The key picker, while it is open.
    pub picker: Option<Picker>,
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
    /// One of the player's own buttons.
    Custom(u32),
    /// A finger on the hotbar picks its slot as it lands and then does nothing.
    Hotbar,
}

/// What the controls want this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchFrame {
    /// Buttons to hold down now.
    pub held: Vec<TouchButton>,
    /// Keyboard keys (`key.keyboard.*`) held down now by the player's buttons.
    pub keys: Vec<String>,
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
    /// The same for the player's buttons.
    custom_pulses: Vec<(u32, Instant)>,
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
            menu: e.menu,
            picker: e.picker,
        })
    }

    /// Opens the layout editor (Options, Controls, Custom Controls).
    pub fn open_editor(&mut self) {
        self.release_all();
        self.editor = Some(Editor {
            draft: self.settings.clone(),
            selected: None,
            drag: None,
            press: None,
            menu: None,
            picker: None,
        });
    }

    /// The player's button `id` as the layout in use has it (the editor's
    /// draft while it is open).
    pub fn custom(&self, id: u32) -> Option<&CustomButton> {
        self.editor
            .as_ref()
            .map_or(&self.settings, |e| &e.draft)
            .custom(id)
    }

    /// Whether a finger is on the player's button `id` now.
    pub fn pressed_custom(&self, id: u32) -> bool {
        self.fingers
            .values()
            .any(|r| matches!(r, Role::Custom(c) if *c == id))
    }

    /// The editor's long press: call it every frame while editing.
    pub fn editor_tick(&mut self, now: Instant) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.menu.is_none()
            && editor.picker.is_none()
            && let Some(press) = editor.press
            && !press.moved
            && now.duration_since(press.since) >= LONG
        {
            editor.menu = Some(EditorMenu {
                at: press.at,
                target: editor.selected,
            });
            editor.drag = None;
            editor.press = None;
        }
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
            self.edit_touch(layout, id, phase, at, now);
            return;
        }
        use TouchPhase::*;
        match phase {
            Started => {
                let role = if let Some(id) = layout.custom_at(at) {
                    self.custom_pulses.push((id, now + PULSE));
                    Role::Custom(id)
                } else if let Some(button) = layout.button_at(at) {
                    self.pulse(button, now);
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

    /// A touch while the editor is open: the key picker or the long press's
    /// menu when one is open, else the toolbar, or picking up and dragging a
    /// control (a finger held still opens the menu, see `editor_tick`).
    fn edit_touch(
        &mut self,
        layout: &Layout,
        id: u64,
        phase: TouchPhase,
        at: (f32, f32),
        now: Instant,
    ) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        match phase {
            TouchPhase::Started => {
                if let Some(picker) = editor.picker {
                    self.pick_key(layout, picker, at);
                    return;
                }
                if let Some(menu) = editor.menu {
                    self.use_menu(layout, menu, at);
                    return;
                }
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
                editor.press = Some(Press {
                    finger: id,
                    at,
                    since: now,
                    moved: false,
                });
            }
            TouchPhase::Moved => {
                if let Some(press) = editor.press.as_mut().filter(|p| p.finger == id)
                    && distance(press.at, at) > STILL
                {
                    press.moved = true;
                }
                // A finger that has not left its spot is a long press, not a drag.
                if editor.press.is_some_and(|p| p.finger == id && !p.moved) {
                    return;
                }
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
                if editor.press.is_some_and(|p| p.finger == id) {
                    editor.press = None;
                }
            }
        }
    }

    /// A touch in the key picker: a key binds it to the button, the rest are
    /// its page and unbind and cancel buttons; a touch off them does nothing.
    fn pick_key(&mut self, layout: &Layout, picker: Picker, at: (f32, f32)) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let grid = picker.grid(layout);
        let Some(hit) = grid
            .cells
            .iter()
            .find(|(_, area)| area.contains(at))
            .map(|(hit, _)| *hit)
        else {
            return;
        };
        match hit {
            PickerHit::Key(name) => {
                editor.draft.set_bind(picker.id, Bind::Key(name.to_owned()));
                editor.picker = None;
            }
            PickerHit::Clear => {
                editor.draft.set_bind(picker.id, Bind::Unbound);
                editor.picker = None;
            }
            PickerHit::Cancel => editor.picker = None,
            PickerHit::Prev => {
                editor.picker = Some(Picker {
                    page: grid.page.saturating_sub(1),
                    ..picker
                });
            }
            PickerHit::Next => {
                editor.picker = Some(Picker {
                    page: (grid.page + 1).min(grid.pages - 1),
                    ..picker
                });
            }
        }
    }

    /// A touch while the long press's menu is open: it closes the menu, and
    /// when it is on a usable item, that item happens.
    fn use_menu(&mut self, layout: &Layout, menu: EditorMenu, at: (f32, f32)) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        editor.menu = None;
        let Some(item) = menu
            .items(layout)
            .into_iter()
            .find(|(_, area, usable)| *usable && area.contains(at))
            .map(|(item, _, _)| item)
        else {
            return;
        };
        let safe = layout.safe;
        let fraction = |(x, y): (f32, f32)| {
            (
                ((x - safe.x) / safe.w.max(1.0)).clamp(0.0, 1.0),
                ((y - safe.y) / safe.h.max(1.0)).clamp(0.0, 1.0),
            )
        };
        let at_menu = Placement {
            centre: Some(fraction(menu.at)),
            ..Placement::default()
        };
        match item {
            MenuItem::Add => {
                if let Some(id) = editor.draft.add_custom(Bind::Unbound, at_menu) {
                    editor.selected = Some(Control::Custom(id));
                }
            }
            MenuItem::Copy => {
                let Some(target) = menu.target else {
                    return;
                };
                let bind = match target {
                    Control::Custom(id) => editor.draft.custom(id).map(|c| c.bind.clone()),
                    Control::Button(b) => Some(Bind::Button(b)),
                    _ => None,
                };
                let (cx, cy) = layout.item(target).area.centre();
                let placement = Placement {
                    centre: Some(fraction((cx + COPY_OFFSET, cy + COPY_OFFSET))),
                    hidden: false,
                    ..editor.draft.placement(target)
                };
                if let Some(bind) = bind
                    && let Some(id) = editor.draft.add_custom(bind, placement)
                {
                    editor.selected = Some(Control::Custom(id));
                }
            }
            MenuItem::Keybind => {
                let id = match menu.target {
                    Some(Control::Custom(id)) => Some(id),
                    _ => editor.draft.add_custom(Bind::Unbound, at_menu),
                };
                if let Some(id) = id {
                    editor.selected = Some(Control::Custom(id));
                    editor.picker = Some(Picker { id, page: 0 });
                }
            }
            MenuItem::Delete => {
                if let Some(Control::Custom(id)) = menu.target {
                    editor.draft.remove_custom(id);
                    editor.selected = None;
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
            None => Control::all()
                .chain(editor.draft.customs().iter().map(|c| Control::Custom(c.id)))
                .collect(),
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
        self.custom_pulses.clear();
        self.sprinting = false;
        self.look = (0.0, 0.0);
        self.slot = None;
        if let Some(editor) = &mut self.editor {
            editor.drag = None;
            editor.press = None;
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
            self.editor_tick(now);
            return TouchFrame::default();
        }
        self.pulses.retain(|(_, until)| *until > now);
        self.custom_pulses.retain(|(_, until)| *until > now);
        let mut held: Vec<TouchButton> = self.pulses.iter().map(|(b, _)| *b).collect();
        let mut down: Vec<u32> = self.custom_pulses.iter().map(|(id, _)| *id).collect();
        let mut stick = [0.0; 2];
        let mut stick_down = false;
        let radius = layout.stick_radius;
        for role in self.fingers.values_mut() {
            match role {
                Role::Button(b) => {
                    held.push(*b);
                }
                Role::Custom(id) => down.push(*id),
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
        // The player's buttons: a copy of a built-in one is that button held;
        // a key is held as the keyboard's.
        let mut keys = Vec::new();
        for id in down {
            match self.settings.custom(id).map(|c| &c.bind) {
                Some(Bind::Button(b)) => held.push(*b),
                Some(Bind::Key(name)) => keys.push(name.clone()),
                _ => {}
            }
        }
        keys.sort();
        keys.dedup();
        held.sort_by_key(|b| *b as u8);
        held.dedup();
        TouchFrame {
            held,
            keys,
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
