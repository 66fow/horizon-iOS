//! The touch buttons' icons: 12 by 12 pixel art, `#` for a lit pixel, drawn
//! scaled to the button in the HUD's flat colour.
use super::controls::TouchButton;

/// An icon's rows, top first.
pub type Icon = [&'static str; 12];

pub const SIZE: usize = 12;

/// The icon drawn on `button`.
pub fn icon(button: TouchButton) -> &'static Icon {
    match button {
        TouchButton::Jump => &JUMP,
        TouchButton::Sneak => &SNEAK,
        TouchButton::Attack => &ATTACK,
        TouchButton::Use => &USE,
        TouchButton::Drop => &DROP,
        TouchButton::Inventory => &INVENTORY,
        TouchButton::Chat => &CHAT,
        TouchButton::Perspective => &PERSPECTIVE,
        TouchButton::Pause => &PAUSE,
        TouchButton::Debug => &DEBUG,
        TouchButton::Sprint => &SPRINT,
    }
}

/// The lit runs of `icon`: `(row, first column, length)`, so a row draws as
/// few rectangles as it has runs.
pub fn runs(icon: &Icon) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (y, row) in icon.iter().enumerate() {
        let mut start = None;
        for (x, c) in row.chars().chain(std::iter::once('.')).enumerate() {
            match (c == '#', start) {
                (true, None) => start = Some(x),
                (false, Some(s)) => {
                    out.push((y, s, x - s));
                    start = None;
                }
                _ => {}
            }
        }
    }
    out
}

/// An arrow up.
const JUMP: Icon = [
    "............",
    ".....##.....",
    "....####....",
    "...######...",
    "..########..",
    ".####..####.",
    ".....##.....",
    ".....##.....",
    ".....##.....",
    ".....##.....",
    ".....##.....",
    "............",
];

/// Two chevrons down.
const SNEAK: Icon = [
    "............",
    ".##......##.",
    "..##....##..",
    "...##..##...",
    "....####....",
    ".....##.....",
    ".##......##.",
    "..##....##..",
    "...##..##...",
    "....####....",
    ".....##.....",
    "............",
];

/// A sword, point up and right.
const ATTACK: Icon = [
    ".........###",
    "........####",
    ".......####.",
    "......####..",
    ".....####...",
    ".#..####....",
    ".######.....",
    "..####......",
    "..####......",
    ".##..##.....",
    "##....#.....",
    "#...........",
];

/// A block.
const USE: Icon = [
    ".....##.....",
    "...##..##...",
    ".##......##.",
    "#.##....##.#",
    "#...####...#",
    "#.....#....#",
    "#.....#....#",
    "#.....#....#",
    "#.....#....#",
    ".##...#..##.",
    "...##.#.##..",
    ".....###....",
];

/// An arrow down onto the ground.
const DROP: Icon = [
    ".....##.....",
    ".....##.....",
    ".....##.....",
    ".....##.....",
    ".##..##..##.",
    "..########..",
    "...######...",
    "....####....",
    ".....##.....",
    "............",
    "############",
    "############",
];

/// A chest.
const INVENTORY: Icon = [
    "............",
    ".##########.",
    "#..........#",
    "#..........#",
    "############",
    "#....##....#",
    "#....##....#",
    "#..........#",
    "#..........#",
    "#..........#",
    "############",
    "............",
];

/// A speech bubble.
const CHAT: Icon = [
    "............",
    ".##########.",
    "#..........#",
    "#..........#",
    "#.##.##.##.#",
    "#..........#",
    "#..........#",
    ".####..####.",
    ".....#.#....",
    "......##....",
    ".......#....",
    "............",
];

/// An eye.
const PERSPECTIVE: Icon = [
    "............",
    "............",
    "............",
    "....####....",
    "..##....##..",
    ".#...##...#.",
    "#...####...#",
    ".#...##...#.",
    "..##....##..",
    "....####....",
    "............",
    "............",
];

/// Two bars.
const PAUSE: Icon = [
    "............",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "..###..###..",
    "............",
];

/// "F3".
const DEBUG: Icon = [
    "............",
    "............",
    "#####.#####.",
    "#.........#.",
    "#.........#.",
    "####...####.",
    "#.........#.",
    "#.........#.",
    "#.........#.",
    "#....######.",
    "............",
    "............",
];

/// Running legs (never on screen: sprint is the stick pushed forward).
const SPRINT: Icon = [
    "......##....",
    "......##....",
    "............",
    ".....###....",
    "....#####...",
    "...#.###.#..",
    ".....###....",
    "....##.##...",
    "...##...##..",
    "..##.....#..",
    "............",
    "............",
];
