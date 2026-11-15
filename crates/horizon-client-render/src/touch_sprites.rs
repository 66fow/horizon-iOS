//! Embedded Bedrock touch artwork, matching Starling's pinned manifest and glyphs.
//! Textures: Mojang AB; terms and provenance are in touch/bedrock/.
//! Additional glyphs adapted from Starling's bedrock_touch.rs (MIT).

macro_rules! sprites {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("touch/bedrock/", $name, ".png")) as &[u8])),*]
    };
}

pub(crate) const SPRITES: &[(&str, &[u8])] = sprites![
    "attack", "attack_pressed",
    "use", "use_pressed",
    "jump", "jump_pressed",
    "sneak", "sneak_pressed",
    "sprint", "sprint_pressed",
    "pause", "pause_pressed",
    "inventory", "inventory_pressed",
    "chat", "chat_pressed",
    "camera", "camera_pressed",
    "drop", "drop_pressed",
    "debug", "debug_pressed",
    "custom_blank", "custom_blank_pressed",
    "dpad_up", "dpad_up_pressed",
    "dpad_down", "dpad_down_pressed",
    "dpad_left", "dpad_left_pressed",
    "dpad_right", "dpad_right_pressed",
    "stick", "stick_knob",
];
