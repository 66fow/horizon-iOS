//! Touch screens. Over menus, [`TouchMouse`] makes the first finger the mouse.
//! While playing, [`TouchControls`] are the on-screen controls, whose layout
//! the player edits (controls.rs), with their icons (icons.rs). iOS sends
//! only touches.
mod controls;
pub mod icons;

pub use controls::*;
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
}
