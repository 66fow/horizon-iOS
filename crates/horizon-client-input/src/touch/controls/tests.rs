use super::*;
use std::time::{Duration, Instant};
use winit::event::TouchPhase::*;

/// An iPhone in landscape at GUI scale 4: 639 by 295 units, its notch and
/// rounded corners taking 44 units at each side and the home bar 5 at the bottom.
const W: f32 = 639.0;
const H: f32 = 295.0;
fn safe() -> Area {
    Area {
        x: 44.0,
        y: 0.0,
        w: W - 88.0,
        h: H - 5.0,
    }
}
fn layout() -> Layout {
    Layout::new(W, H, safe(), &TouchSettings::default())
}
fn centre(l: &Layout, button: TouchButton) -> (f32, f32) {
    l.item(Control::Button(button)).area.centre()
}
fn after(now: Instant, ms: u64) -> Instant {
    now + Duration::from_millis(ms)
}

#[test]
fn the_default_buttons_stay_in_the_safe_area_and_apart() {
    for (w, h, safe) in [
        (W, H, safe()),
        (
            512.0,
            384.0,
            Area {
                x: 0.0,
                y: 6.0,
                w: 512.0,
                h: 372.0,
            },
        ),
        (
            427.0,
            240.0,
            Area {
                x: 0.0,
                y: 0.0,
                w: 427.0,
                h: 240.0,
            },
        ),
    ] {
        let l = Layout::new(w, h, safe, &TouchSettings::default());
        let buttons: Vec<_> = l
            .items
            .iter()
            .filter(|i| matches!(i.control, Control::Button(_)))
            .collect();
        for (i, a) in buttons.iter().enumerate() {
            let r = a.area;
            assert!(
                r.x >= safe.x
                    && r.y >= safe.y
                    && r.x + r.w <= safe.x + safe.w
                    && r.y + r.h <= safe.y + safe.h,
                "{:?} outside the safe area on {w}x{h}",
                a.control
            );
            for b in &buttons[i + 1..] {
                let o = b.area;
                let apart =
                    r.x + r.w <= o.x || o.x + o.w <= r.x || r.y + r.h <= o.y || o.y + o.h <= r.y;
                assert!(apart, "{:?} overlaps {:?} on {w}x{h}", a.control, b.control);
            }
            let bar = l.hotbar();
            let clear = r.x + r.w <= bar.x || bar.x + bar.w <= r.x || r.y + r.h <= bar.y;
            assert!(clear, "{:?} covers the hotbar on {w}x{h}", a.control);
        }
        let stick = l.item(Control::Stick).area;
        assert!(stick.x + stick.w <= l.hotbar().x);
    }
}

#[test]
fn the_hotbar_sits_above_the_home_bar() {
    let l = layout();
    // Vanilla draws it at the screen's bottom edge; the layout lifts it by the home bar.
    assert_eq!(l.hotbar_offset(), (0.0, -5.0));
    assert_eq!(l.hotbar().y + HOTBAR_HEIGHT, H - 5.0);
}

#[test]
fn a_button_is_held_while_the_finger_is_on_it() {
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.touch(&l, 1, Started, centre(&l, TouchButton::Jump), now);
    assert_eq!(t.frame(&l, after(now, 1000)).held, [TouchButton::Jump]);
    t.touch(
        &l,
        1,
        Ended,
        centre(&l, TouchButton::Jump),
        after(now, 1000),
    );
    assert!(t.frame(&l, after(now, 1000)).held.is_empty());
}

#[test]
fn a_quick_button_tap_still_lasts_a_game_tick() {
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.touch(&l, 1, Started, centre(&l, TouchButton::Pause), now);
    t.touch(&l, 1, Ended, centre(&l, TouchButton::Pause), now);
    assert_eq!(t.frame(&l, after(now, 50)).held, [TouchButton::Pause]);
    assert!(t.frame(&l, after(now, 200)).held.is_empty());
}

#[test]
fn sneak_is_down_while_the_finger_is_and_the_game_decides_toggle_or_hold() {
    // The touch button only reports the finger: with the game's Hold option
    // sneaking lasts as long as the finger, with Toggle the game flips it on
    // the press. No latch of the button's own (it made Toggle flip twice).
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    let at = centre(&l, TouchButton::Sneak);
    t.touch(&l, 1, Started, at, now);
    assert_eq!(t.frame(&l, after(now, 5000)).held, [TouchButton::Sneak]);
    t.touch(&l, 1, Ended, at, after(now, 5000));
    assert!(t.frame(&l, after(now, 5100)).held.is_empty());
    // A quick tap still lasts a game tick.
    t.touch(&l, 2, Started, at, after(now, 6000));
    t.touch(&l, 2, Ended, at, after(now, 6000));
    assert_eq!(t.frame(&l, after(now, 6020)).held, [TouchButton::Sneak]);
    assert!(t.frame(&l, after(now, 6500)).held.is_empty());
}

#[test]
fn where_the_game_toggles_sneaking_each_tap_is_a_press() {
    // The controller's toggle sneak flips sneaking on each press: every tap
    // must reach the game as one press, not every other one (a held toggle
    // would press on the first tap and only release on the second).
    let l = layout();
    let mut t = TouchControls {
        sneak_taps: true,
        ..TouchControls::default()
    };
    let now = Instant::now();
    let at = centre(&l, TouchButton::Sneak);
    for tap in 0..3u64 {
        let start = after(now, tap * 1000);
        t.touch(&l, tap, Started, at, start);
        t.touch(&l, tap, Ended, at, start);
        assert_eq!(
            t.frame(&l, after(start, 20)).held,
            [TouchButton::Sneak],
            "tap {tap}"
        );
        assert!(t.frame(&l, after(start, 500)).held.is_empty(), "tap {tap}");
    }
    assert!(!t.pressed(TouchButton::Sneak));
}

#[test]
fn the_stick_stays_where_it_is_and_the_finger_is_measured_from_its_home() {
    let l = layout();
    let home = l.stick_home;
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.touch(&l, 1, Started, home, now);
    // Half the radius to the right of home: half strafe, no forward.
    t.touch(&l, 1, Moved, (home.0 + STICK_RADIUS / 2.0, home.1), now);
    let f = t.frame(&l, now);
    assert!(
        (f.stick[0] - 0.5).abs() < 1e-5 && f.stick[1] == 0.0,
        "{:?}",
        f.stick
    );
    // The drawn stick never leaves its home.
    assert_eq!(t.stick(&l).map(|(origin, _)| origin), Some(home));
    // Far up: clamped to full forward, which sprints.
    t.touch(&l, 1, Moved, (home.0, home.1 - 200.0), now);
    let f = t.frame(&l, now);
    assert!((f.stick[1] - 1.0).abs() < 1e-5, "{:?}", f.stick);
    assert_eq!(f.held, [TouchButton::Sprint]);
    t.touch(&l, 1, Ended, (home.0, home.1 - 200.0), now);
    assert_eq!(t.frame(&l, now).stick, [0.0, 0.0]);
}

#[test]
fn a_finger_that_lands_off_the_stick_is_not_the_stick() {
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    // Well above the stick's square, on the left: the view.
    t.touch(&l, 1, Started, (60.0, 60.0), now);
    t.touch(&l, 1, Moved, (70.0, 60.0), now);
    let f = t.frame(&l, now);
    assert_eq!(f.stick, [0.0, 0.0]);
    assert_eq!(f.look, (10.0, 0.0));
}

#[test]
fn sprint_keeps_going_until_the_stick_comes_back() {
    let l = layout();
    let home = l.stick_home;
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.touch(&l, 1, Started, home, now);
    assert!(t.frame(&l, now).held.is_empty());
    // Pushed forward: sprints.
    t.touch(&l, 1, Moved, (home.0, home.1 - STICK_RADIUS), now);
    assert_eq!(t.frame(&l, now).held, [TouchButton::Sprint]);
    // The thumb relaxes to half way: still sprinting.
    t.touch(&l, 1, Moved, (home.0, home.1 - STICK_RADIUS * 0.5), now);
    assert_eq!(t.frame(&l, now).held, [TouchButton::Sprint]);
    // Back near the centre: the run ends, and it takes a new push to start.
    t.touch(&l, 1, Moved, (home.0, home.1 - STICK_RADIUS * 0.1), now);
    assert!(t.frame(&l, now).held.is_empty());
    t.touch(&l, 1, Moved, (home.0, home.1 - STICK_RADIUS * 0.5), now);
    assert!(t.frame(&l, now).held.is_empty());
    // Lifting the finger ends it too.
    t.touch(&l, 1, Moved, (home.0, home.1 - STICK_RADIUS), now);
    assert_eq!(t.frame(&l, now).held, [TouchButton::Sprint]);
    t.touch(&l, 1, Ended, (home.0, home.1 - STICK_RADIUS), now);
    assert!(t.frame(&l, now).held.is_empty());
}

#[test]
fn dragging_the_view_turns_it_by_the_distance_dragged() {
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.touch(&l, 1, Started, (400.0, 100.0), now);
    t.touch(&l, 1, Moved, (410.0, 95.0), now);
    t.touch(&l, 1, Moved, (420.0, 97.0), now);
    assert_eq!(t.frame(&l, now).look, (20.0, -3.0));
    // Taken: the next frame turns no further.
    assert_eq!(t.frame(&l, now).look, (0.0, 0.0));
    // A dragged touch is no tap.
    t.touch(&l, 1, Ended, (420.0, 97.0), after(now, 100));
    assert!(t.frame(&l, after(now, 110)).held.is_empty());
}

#[test]
fn a_quick_tap_on_the_view_uses_and_holding_still_attacks() {
    let l = layout();
    let now = Instant::now();
    let mut t = TouchControls::default();
    t.touch(&l, 1, Started, (400.0, 100.0), now);
    t.touch(&l, 1, Ended, (401.0, 100.0), after(now, 100));
    assert_eq!(t.frame(&l, after(now, 120)).held, [TouchButton::Use]);

    let mut t = TouchControls::default();
    t.touch(&l, 2, Started, (400.0, 100.0), now);
    assert!(t.frame(&l, after(now, 100)).held.is_empty());
    assert_eq!(t.frame(&l, after(now, 400)).held, [TouchButton::Attack]);
    // Moving while breaking keeps breaking (and turns the view).
    t.touch(&l, 2, Moved, (450.0, 100.0), after(now, 400));
    assert_eq!(t.frame(&l, after(now, 400)).held, [TouchButton::Attack]);
    t.touch(&l, 2, Ended, (450.0, 100.0), after(now, 400));
    assert!(t.frame(&l, after(now, 600)).held.is_empty());
}

#[test]
fn a_tap_on_the_hotbar_picks_that_slot() {
    let l = layout();
    let bar = l.hotbar();
    let mut t = TouchControls::default();
    let now = Instant::now();
    // Slot 4 is the middle one.
    t.touch(&l, 1, Started, (W / 2.0, bar.y + 10.0), now);
    assert_eq!(t.frame(&l, now).slot, Some(4));
    assert_eq!(t.frame(&l, now).slot, None);
    t.touch(&l, 2, Started, (bar.x + 2.0, bar.y + 2.0), now);
    assert_eq!(t.frame(&l, now).slot, Some(0));
    t.touch(&l, 3, Started, (bar.x + bar.w - 2.0, bar.y + 2.0), now);
    assert_eq!(t.frame(&l, now).slot, Some(8));
}

#[test]
fn a_second_finger_in_the_stick_area_looks() {
    let l = layout();
    let mut t = TouchControls::default();
    let now = Instant::now();
    let home = l.stick_home;
    t.touch(&l, 1, Started, home, now);
    t.touch(&l, 2, Started, (home.0 + 20.0, home.1), now);
    t.touch(&l, 2, Moved, (home.0 + 30.0, home.1), now);
    let f = t.frame(&l, now);
    assert_eq!(f.stick, [0.0, 0.0]);
    assert_eq!(f.look, (10.0, 0.0));
}

/// Opens the editor, as Options, Controls, Custom Controls does.
fn open_editor(t: &mut TouchControls, _l: &Layout, _now: Instant) {
    t.open_editor();
    assert!(t.editing());
}

fn toolbar(l: &Layout, button: EditorButton) -> (f32, f32) {
    EditorButton::toolbar(l)
        .into_iter()
        .find(|(b, _)| *b == button)
        .unwrap()
        .1
        .centre()
}

#[test]
fn the_editor_moves_a_button_where_it_is_dragged_and_done_keeps_it() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    let l = t.layout(W, H, safe());
    open_editor(&mut t, &l, now);
    // While editing, controls do nothing.
    // Picked up 3 units right of its centre, dropped at (200, 150): its centre is 3 units left.
    let jump = centre(&l, TouchButton::Jump);
    t.touch(&l, 1, Started, (jump.0 + 3.0, jump.1), now);
    assert!(t.frame(&l, now).held.is_empty());
    assert_eq!(
        t.editor().unwrap().selected,
        Some(Control::Button(TouchButton::Jump))
    );
    t.touch(&l, 1, Moved, (250.0, 100.0), now);
    t.touch(&l, 1, Moved, (200.0, 150.0), now);
    t.touch(&l, 1, Ended, (200.0, 150.0), now);
    let moved = t.layout(W, H, safe());
    let (cx, cy) = moved.item(Control::Button(TouchButton::Jump)).area.centre();
    assert!(
        (cx - 197.0).abs() < 0.01 && (cy - 150.0).abs() < 0.01,
        "{cx},{cy}"
    );
    // Not kept until Done.
    assert_eq!(t.settings, TouchSettings::default());
    let moved_layout = t.layout(W, H, safe());
    t.touch(
        &moved_layout,
        2,
        Started,
        toolbar(&moved_layout, EditorButton::Done),
        now,
    );
    assert!(!t.editing());
    let saved = t.take_saved().expect("Done saves");
    assert_eq!(saved, t.settings);
    let after_done = t.layout(W, H, safe());
    assert_eq!(
        after_done
            .item(Control::Button(TouchButton::Jump))
            .area
            .centre(),
        (cx, cy)
    );
}

#[test]
fn the_editor_resizes_fades_and_hides_the_selected_control() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    let l = t.layout(W, H, safe());
    open_editor(&mut t, &l, now);
    let sneak = Control::Button(TouchButton::Sneak);
    t.touch(&l, 1, Started, centre(&l, TouchButton::Sneak), now);
    t.touch(&l, 1, Ended, centre(&l, TouchButton::Sneak), now);
    for button in [
        EditorButton::Bigger,
        EditorButton::Bigger,
        EditorButton::Fainter,
        EditorButton::Hide,
    ] {
        let l = t.layout(W, H, safe());
        t.touch(&l, 2, Started, toolbar(&l, button), now);
        t.touch(&l, 2, Ended, toolbar(&l, button), now);
    }
    let view = t.editor().unwrap();
    assert_eq!(view.selected, Some(sneak));
    assert!((view.placement.size - 1.2).abs() < 1e-5);
    assert!((view.placement.opacity - 0.9).abs() < 1e-5);
    assert!(view.placement.hidden);
    let l = t.layout(W, H, safe());
    let item = l.item(sneak);
    assert!((item.area.w - 36.0).abs() < 1e-4, "{}", item.area.w);
    // Resizing keeps its centre.
    assert_eq!(item.area.centre(), layout().item(sneak).area.centre());
    // A hidden button can't be pressed (once the editor is closed).
    t.touch(&l, 3, Started, toolbar(&l, EditorButton::Done), now);
    let l = t.layout(W, H, safe());
    assert_eq!(l.button_at(l.item(sneak).area.centre()), None);
}

#[test]
fn sizes_and_opacities_stop_at_their_limits() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    let l = t.layout(W, H, safe());
    open_editor(&mut t, &l, now);
    t.touch(&l, 1, Started, centre(&l, TouchButton::Use), now);
    for _ in 0..30 {
        let l = t.layout(W, H, safe());
        t.touch(&l, 2, Started, toolbar(&l, EditorButton::Bigger), now);
        t.touch(&l, 2, Started, toolbar(&l, EditorButton::Fainter), now);
    }
    let p = t.editor().unwrap().placement;
    assert_eq!((p.size, p.opacity), (SIZES.1, OPACITIES.0));
}

#[test]
fn with_nothing_selected_reset_puts_everything_back() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    let l = t.layout(W, H, safe());
    open_editor(&mut t, &l, now);
    // Drag the hotbar up, then tap empty space and Reset.
    let bar = l.hotbar().centre();
    t.touch(&l, 1, Started, bar, now);
    t.touch(&l, 1, Moved, (bar.0, 120.0), now);
    t.touch(&l, 1, Ended, (bar.0, 120.0), now);
    assert!(t.layout(W, H, safe()).hotbar_offset().1 < -100.0);
    let l = t.layout(W, H, safe());
    t.touch(&l, 2, Started, (300.0, 60.0), now);
    assert_eq!(t.editor().unwrap().selected, None);
    t.touch(&l, 3, Started, toolbar(&l, EditorButton::Reset), now);
    assert_eq!(t.layout(W, H, safe()), layout());
}

#[test]
fn the_hotbar_cant_be_hidden() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    let l = t.layout(W, H, safe());
    open_editor(&mut t, &l, now);
    for control in [Control::Hotbar] {
        let l = t.layout(W, H, safe());
        t.touch(&l, 1, Started, l.item(control).area.centre(), now);
        t.touch(&l, 1, Ended, l.item(control).area.centre(), now);
        t.touch(&l, 2, Started, toolbar(&l, EditorButton::Hide), now);
        assert!(!t.editor().unwrap().placement.hidden, "{control:?}");
    }
}

#[test]
fn a_moved_stick_takes_a_square_around_where_it_was_put() {
    let mut s = TouchSettings::default();
    s.set(
        Control::Stick,
        Placement {
            centre: Some((0.25, 0.5)),
            size: 1.5,
            ..Placement::default()
        },
    );
    let l = Layout::new(W, H, safe(), &s);
    let home = (44.0 + 0.25 * (W - 88.0), 0.5 * (H - 5.0));
    let area = l.item(Control::Stick).area;
    assert!((area.centre().0 - home.0).abs() < 0.01 && (area.centre().1 - home.1).abs() < 0.01);
    assert!((l.stick_radius - STICK_RADIUS * 1.5).abs() < 1e-5);
    assert_eq!(l.stick_home, area.centre());
}

#[test]
fn the_default_stick_has_a_fixed_home_in_the_bottom_left() {
    let l = layout();
    let area = l.item(Control::Stick).area;
    assert_eq!(l.stick_home, area.centre());
    assert!(area.x >= l.safe.x && area.y + area.h <= l.safe.y + l.safe.h);
    assert!(l.stick_home.0 < W / 2.0 && l.stick_home.1 > H / 2.0);
}

#[test]
fn the_settings_file_reads_back_what_it_wrote() {
    let mut s = TouchSettings::default();
    s.set(
        Control::Button(TouchButton::Jump),
        Placement {
            centre: Some((0.9, 0.75)),
            size: 1.3,
            opacity: 0.6,
            hidden: false,
        },
    );
    s.set(
        Control::Button(TouchButton::Drop),
        Placement {
            hidden: true,
            ..Placement::default()
        },
    );
    s.set(
        Control::Hotbar,
        Placement {
            centre: Some((0.5, 0.2)),
            ..Placement::default()
        },
    );
    let text = s.to_text();
    assert_eq!(
        text,
        "jump=0.9000,0.7500,1.30,0.60,0\ndrop=-,-,1.00,1.00,1\nhotbar=0.5000,0.2000,1.00,1.00,0\n"
    );
    assert_eq!(TouchSettings::parse(&text), s);
}

#[test]
fn the_settings_file_skips_what_it_cant_read_and_keeps_values_in_range() {
    let s = TouchSettings::parse(
        "# a comment\nnonsense\nwings=0.1,0.1,1,1,0\njump=0.5,0.5\nsneak=2,-1,9,0,1\nedit=-,-,1,1,1\n",
    );
    let sneak = s.placement(Control::Button(TouchButton::Sneak));
    assert_eq!(sneak.centre, Some((1.0, 0.0)));
    assert_eq!(
        (sneak.size, sneak.opacity, sneak.hidden),
        (SIZES.1, OPACITIES.0, true)
    );
    // The old Edit button is gone: its line is skipped.
    assert_eq!(s.customs().len(), 0);
    assert_eq!(
        s.placement(Control::Button(TouchButton::Jump)),
        Placement::default()
    );
}

#[test]
fn every_icon_is_twelve_by_twelve() {
    for b in TouchButton::PLACED.into_iter().chain([TouchButton::Sprint]) {
        let icon = crate::touch::icons::icon(b);
        assert!(icon.iter().all(|row| row.chars().count() == 12), "{b:?}");
        assert!(
            !crate::touch::icons::runs(icon).is_empty(),
            "{b:?} is blank"
        );
    }
    // A run per lit stretch of a row.
    let runs = crate::touch::icons::runs(crate::touch::icons::icon(TouchButton::Pause));
    assert_eq!(runs[0], (1, 2, 3));
    assert_eq!(runs[1], (1, 7, 3));
}

#[test]
fn the_editors_toolbar_covers_no_control_where_they_start() {
    let l = layout();
    for (button, t) in EditorButton::toolbar(&l) {
        assert!(
            t.x >= l.safe.x && t.x + t.w <= l.safe.x + l.safe.w,
            "{button:?} off the safe area"
        );
        for item in &l.items {
            let a = item.area;
            let apart =
                t.x + t.w <= a.x || a.x + a.w <= t.x || t.y + t.h <= a.y || a.y + a.h <= t.y;
            // The stick's square may lie under the toolbar when the stick is moved there.
            assert!(
                apart || item.control == Control::Stick,
                "{button:?} covers {:?}",
                item.control
            );
        }
    }
}

fn custom_key(s: &mut TouchSettings, key: &str, at: (f32, f32)) -> u32 {
    s.add_custom(
        Bind::Key(key.to_owned()),
        Placement {
            centre: Some(at),
            ..Placement::default()
        },
    )
    .expect("room for another button")
}

#[test]
fn a_button_the_player_added_holds_its_key_while_the_finger_is_down() {
    let mut s = TouchSettings::default();
    let id = custom_key(&mut s, "key.keyboard.f7", (0.5, 0.5));
    let l = Layout::new(W, H, safe(), &s);
    let mut t = TouchControls::default();
    t.settings = s;
    let now = Instant::now();
    let at = l.item(Control::Custom(id)).area.centre();
    t.touch(&l, 1, Started, at, now);
    let f = t.frame(&l, after(now, 100));
    assert_eq!(f.keys, ["key.keyboard.f7"]);
    assert!(f.held.is_empty());
    assert!(t.pressed_custom(id));
    t.touch(&l, 1, Ended, at, after(now, 200));
    assert!(t.frame(&l, after(now, 300)).keys.is_empty());
    // A quick tap still lasts a game tick.
    t.touch(&l, 2, Started, at, after(now, 1000));
    t.touch(&l, 2, Ended, at, after(now, 1000));
    assert_eq!(t.frame(&l, after(now, 1020)).keys, ["key.keyboard.f7"]);
    assert!(t.frame(&l, after(now, 1500)).keys.is_empty());
}

#[test]
fn a_button_with_no_key_does_nothing_and_a_hidden_one_cant_be_pressed() {
    let mut s = TouchSettings::default();
    let blank = s
        .add_custom(
            Bind::Unbound,
            Placement {
                centre: Some((0.4, 0.5)),
                ..Placement::default()
            },
        )
        .unwrap();
    let hidden = s
        .add_custom(
            Bind::Key("key.keyboard.g".to_owned()),
            Placement {
                centre: Some((0.6, 0.5)),
                hidden: true,
                ..Placement::default()
            },
        )
        .unwrap();
    let l = Layout::new(W, H, safe(), &s);
    let mut t = TouchControls::default();
    t.settings = s;
    let now = Instant::now();
    t.touch(&l, 1, Started, l.item(Control::Custom(blank)).area.centre(), now);
    let f = t.frame(&l, now);
    assert!(f.keys.is_empty() && f.held.is_empty());
    assert_eq!(l.custom_at(l.item(Control::Custom(hidden)).area.centre()), None);
}

#[test]
fn a_copy_of_a_built_in_button_does_what_it_does() {
    let mut s = TouchSettings::default();
    let id = s
        .add_custom(
            Bind::Button(TouchButton::Jump),
            Placement {
                centre: Some((0.5, 0.4)),
                ..Placement::default()
            },
        )
        .unwrap();
    let l = Layout::new(W, H, safe(), &s);
    let mut t = TouchControls::default();
    t.settings = s;
    let now = Instant::now();
    t.touch(&l, 1, Started, l.item(Control::Custom(id)).area.centre(), now);
    assert_eq!(t.frame(&l, after(now, 10)).held, [TouchButton::Jump]);
}

#[test]
fn a_long_press_opens_the_menu_and_copy_copies_the_button_under_the_finger() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.open_editor();
    let l = t.layout(W, H, safe());
    let jump = centre(&l, TouchButton::Jump);
    t.touch(&l, 1, Started, jump, now);
    // Held still, but not long enough yet.
    t.editor_tick(after(now, 100));
    assert!(t.editor().unwrap().menu.is_none());
    t.editor_tick(after(now, 600));
    let menu = t.editor().unwrap().menu.expect("a long press opens the menu");
    assert_eq!(menu.target, Some(Control::Button(TouchButton::Jump)));
    let items = menu.items(&l);
    let usable = |item| items.iter().find(|(i, _, _)| *i == item).unwrap().2;
    assert!(usable(MenuItem::Add) && usable(MenuItem::Copy) && usable(MenuItem::Keybind));
    // Only the player's own buttons can be deleted.
    assert!(!usable(MenuItem::Delete));
    let copy = items.iter().find(|(i, _, _)| *i == MenuItem::Copy).unwrap().1;
    t.touch(&l, 2, Started, copy.centre(), after(now, 700));
    let view = t.editor().unwrap();
    assert!(view.menu.is_none());
    let Some(Control::Custom(id)) = view.selected else {
        panic!("the copy is picked: {:?}", view.selected);
    };
    // Done keeps it, and it holds Jump like the original.
    let l = t.layout(W, H, safe());
    t.touch(&l, 3, Started, toolbar(&l, EditorButton::Done), after(now, 800));
    assert!(!t.editing());
    assert_eq!(
        t.settings.custom(id).map(|c| c.bind.clone()),
        Some(Bind::Button(TouchButton::Jump))
    );
    let l = t.layout(W, H, safe());
    let at = l.item(Control::Custom(id)).area.centre();
    t.touch(&l, 4, Started, at, after(now, 900));
    assert_eq!(t.frame(&l, after(now, 910)).held, [TouchButton::Jump]);
}

#[test]
fn a_finger_that_moves_is_a_drag_not_a_long_press() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.open_editor();
    let l = t.layout(W, H, safe());
    let jump = centre(&l, TouchButton::Jump);
    t.touch(&l, 1, Started, jump, now);
    t.touch(&l, 1, Moved, (jump.0 - 40.0, jump.1), now);
    t.editor_tick(after(now, 900));
    assert!(t.editor().unwrap().menu.is_none());
}

#[test]
fn keybind_on_empty_ground_adds_a_button_and_the_picker_binds_a_key_to_it() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.open_editor();
    let l = t.layout(W, H, safe());
    let spot = (300.0, 100.0);
    assert_eq!(l.control_at(spot), None);
    t.touch(&l, 1, Started, spot, now);
    t.editor_tick(after(now, 600));
    let menu = t.editor().unwrap().menu.unwrap();
    assert_eq!(menu.target, None);
    let keybind = menu
        .items(&l)
        .into_iter()
        .find(|(i, _, _)| *i == MenuItem::Keybind)
        .unwrap()
        .1;
    t.touch(&l, 2, Started, keybind.centre(), after(now, 700));
    let picker = t.editor().unwrap().picker.expect("the picker opens");
    // The new button waits for its key.
    assert_eq!(
        t.editor().unwrap().selected,
        Some(Control::Custom(picker.id))
    );
    let grid = picker.grid(&l);
    let f7 = grid
        .cells
        .iter()
        .find(|(hit, _)| *hit == PickerHit::Key("key.keyboard.f7"))
        .expect("F7 is on the first page")
        .1;
    t.touch(&l, 3, Started, f7.centre(), after(now, 800));
    assert!(t.editor().unwrap().picker.is_none());
    t.touch(&l, 4, Started, toolbar(&l, EditorButton::Done), after(now, 900));
    assert_eq!(
        t.settings.custom(picker.id).map(|c| c.bind.clone()),
        Some(Bind::Key("key.keyboard.f7".to_owned()))
    );
    // It sits where the finger was.
    let l = t.layout(W, H, safe());
    let (cx, cy) = l.item(Control::Custom(picker.id)).area.centre();
    assert!((cx - spot.0).abs() < 0.5 && (cy - spot.1).abs() < 0.5, "{cx},{cy}");
}

#[test]
fn keybind_on_one_of_the_players_buttons_rebinds_it_and_delete_removes_it() {
    let mut s = TouchSettings::default();
    let id = custom_key(&mut s, "key.keyboard.f7", (0.5, 0.5));
    let mut t = TouchControls::default();
    t.settings = s;
    let now = Instant::now();
    t.open_editor();
    let l = t.layout(W, H, safe());
    let at = l.item(Control::Custom(id)).area.centre();
    t.touch(&l, 1, Started, at, now);
    t.editor_tick(after(now, 600));
    let menu = t.editor().unwrap().menu.unwrap();
    assert_eq!(menu.target, Some(Control::Custom(id)));
    let pick = |item| {
        menu.items(&l)
            .into_iter()
            .find(|(i, _, _)| *i == item)
            .unwrap()
            .1
            .centre()
    };
    t.touch(&l, 2, Started, pick(MenuItem::Keybind), after(now, 700));
    let picker = t.editor().unwrap().picker.unwrap();
    assert_eq!(picker.id, id);
    // Unbinding it leaves the button with no key.
    let clear = picker
        .grid(&l)
        .cells
        .iter()
        .find(|(hit, _)| *hit == PickerHit::Clear)
        .unwrap()
        .1;
    t.touch(&l, 3, Started, clear.centre(), after(now, 800));
    t.editor_tick(after(now, 900));
    // Pressed long again: delete it.
    t.touch(&l, 4, Started, at, after(now, 1000));
    t.editor_tick(after(now, 1600));
    let menu = t.editor().unwrap().menu.unwrap();
    let delete = menu
        .items(&l)
        .into_iter()
        .find(|(i, _, _)| *i == MenuItem::Delete)
        .unwrap();
    assert!(delete.2);
    t.touch(&l, 5, Started, delete.1.centre(), after(now, 1700));
    t.touch(&l, 6, Started, toolbar(&l, EditorButton::Done), after(now, 1800));
    assert!(t.settings.customs().is_empty());
}

#[test]
fn a_touch_off_the_menu_closes_it_and_does_nothing() {
    let mut t = TouchControls::default();
    let now = Instant::now();
    t.open_editor();
    let l = t.layout(W, H, safe());
    t.touch(&l, 1, Started, (300.0, 100.0), now);
    t.editor_tick(after(now, 600));
    assert!(t.editor().unwrap().menu.is_some());
    t.touch(&l, 2, Started, (500.0, 200.0), after(now, 700));
    assert!(t.editor().unwrap().menu.is_none());
    assert_eq!(t.layout(W, H, safe()), layout());
}

#[test]
fn the_picker_pages_stay_in_the_safe_area_and_page_buttons_turn_them() {
    let small = Area {
        x: 0.0,
        y: 0.0,
        w: 427.0,
        h: 240.0,
    };
    let l = Layout::new(427.0, 240.0, small, &TouchSettings::default());
    let first = Picker { id: 0, page: 0 }.grid(&l);
    assert!(first.pages > 1, "{} pages", first.pages);
    // A page past the last shows the last.
    let last = Picker { id: 0, page: 99 }.grid(&l);
    assert_eq!(last.page, last.pages - 1);
    for grid in [&first, &last] {
        for (hit, a) in &grid.cells {
            assert!(
                a.x >= small.x
                    && a.y >= small.y
                    && a.x + a.w <= small.x + small.w
                    && a.y + a.h <= small.y + small.h,
                "{hit:?} off the screen"
            );
        }
    }
    // Every key is on some page, once.
    let mut seen = Vec::new();
    for page in 0..first.pages {
        for (hit, _) in Picker { id: 0, page }.grid(&l).cells {
            if let PickerHit::Key(name) = hit {
                seen.push(name);
            }
        }
    }
    assert_eq!(seen, PICKER_KEYS);
    // Next turns the page and Prev turns it back.
    let mut t = TouchControls::default();
    t.open_editor();
    let id = t.editor.as_mut().unwrap().draft.add_custom(Bind::Unbound, Placement::default()).unwrap();
    t.editor.as_mut().unwrap().picker = Some(Picker { id, page: 0 });
    let next = first.cells.iter().find(|(h, _)| *h == PickerHit::Next).unwrap().1;
    t.touch(&l, 1, Started, next.centre(), Instant::now());
    assert_eq!(t.editor().unwrap().picker.unwrap().page, 1);
    let prev = first.cells.iter().find(|(h, _)| *h == PickerHit::Prev).unwrap().1;
    t.touch(&l, 2, Started, prev.centre(), Instant::now());
    assert_eq!(t.editor().unwrap().picker.unwrap().page, 0);
    let cancel = first.cells.iter().find(|(h, _)| *h == PickerHit::Cancel).unwrap().1;
    t.touch(&l, 3, Started, cancel.centre(), Instant::now());
    assert!(t.editor().unwrap().picker.is_none());
}

#[test]
fn every_key_the_picker_offers_is_a_key_the_game_knows() {
    for name in PICKER_KEYS {
        assert!(crate::keybinds::key_code(name).is_some(), "{name}");
    }
    let mut sorted = PICKER_KEYS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), PICKER_KEYS.len(), "a key is offered twice");
}

#[test]
fn the_players_buttons_are_kept_in_the_settings_file() {
    let mut s = TouchSettings::default();
    s.add_custom(
        Bind::Key("key.keyboard.f7".to_owned()),
        Placement {
            centre: Some((0.25, 0.5)),
            size: 1.5,
            opacity: 0.8,
            hidden: false,
        },
    )
    .unwrap();
    s.add_custom(Bind::Button(TouchButton::Jump), Placement::default())
        .unwrap();
    let text = s.to_text();
    assert_eq!(
        text,
        "custom0=key.keyboard.f7,0.2500,0.5000,1.50,0.80,0\ncustom1=button:jump,-,-,1.00,1.00,0\n"
    );
    assert_eq!(TouchSettings::parse(&text), s);
    // A key the game doesn't know, or a repeated number, is skipped.
    let bad = TouchSettings::parse(
        "custom0=key.keyboard.nope,-,-,1,1,0\ncustom1=-,-,-,1,1,0\ncustom1=-,-,-,1,1,0\ncustomx=-,-,-,1,1,0\n",
    );
    assert_eq!(bad.customs().len(), 1);
}

#[test]
fn no_more_than_the_most_can_be_added() {
    let mut s = TouchSettings::default();
    for _ in 0..MAX_CUSTOM {
        assert!(s.add_custom(Bind::Unbound, Placement::default()).is_some());
    }
    assert!(s.add_custom(Bind::Unbound, Placement::default()).is_none());
}
