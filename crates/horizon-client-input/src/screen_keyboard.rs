//! How much of the screen the iPhone's own keyboard covers, so a text box (chat) can sit above
//! it. iOS says so in `UIKeyboardWillChangeFrameNotification`; elsewhere it's always 0.

use std::sync::atomic::{AtomicU64, Ordering};

/// The keyboard's height over the bottom of the screen, in points (an f64's bits).
static HEIGHT: AtomicU64 = AtomicU64::new(0);

/// The keyboard's height over the bottom of the screen, in points: 0 when it's away.
pub fn height_points() -> f64 {
    f64::from_bits(HEIGHT.load(Ordering::Relaxed))
}

/// Start following the keyboard (once; on iOS, from the main thread).
pub fn watch() {
    #[cfg(target_os = "ios")]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(ios::watch);
    }
}

#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod ios {
    use super::HEIGHT;
    use objc2::encode::{Encode, Encoding};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{ClassType, class, define_class, msg_send, sel};
    use objc2_foundation::NSString;
    use std::sync::atomic::Ordering;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        origin: Point,
        size: Size,
    }
    // SAFETY: the layouts of CGPoint, CGSize and CGRect on 64-bit iOS (CGFloat is f64).
    unsafe impl Encode for Point {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for Size {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl Encode for Rect {
        const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and this class has no Drop.
        #[unsafe(super(NSObject))]
        #[name = "HorizonKeyboardWatch"]
        struct Watch;

        impl Watch {
            #[unsafe(method(keyboardChanged:))]
            fn keyboard_changed(&self, note: &AnyObject) {
                changed(note);
            }
        }
    );

    fn changed(note: &AnyObject) {
        // SAFETY: a UIKeyboard notification's userInfo holds an NSValue of a CGRect under
        // UIKeyboardFrameEndUserInfoKey, in the screen's coordinates, like UIScreen's bounds.
        unsafe {
            let info: Option<Retained<AnyObject>> = msg_send![note, userInfo];
            let Some(info) = info else { return };
            let key = NSString::from_str("UIKeyboardFrameEndUserInfoKey");
            let value: Option<Retained<AnyObject>> = msg_send![&*info, objectForKey: &*key];
            let Some(value) = value else { return };
            let frame: Rect = msg_send![&*value, CGRectValue];
            let screen: Retained<AnyObject> = msg_send![class!(UIScreen), mainScreen];
            let bounds: Rect = msg_send![&*screen, bounds];
            let height = (bounds.size.height - frame.origin.y).clamp(0.0, bounds.size.height);
            HEIGHT.store(height.to_bits(), Ordering::Relaxed);
        }
    }

    pub(super) fn watch() {
        // SAFETY: `new` makes the observer, kept for the app's life; `addObserver:…` takes
        // it, a selector it answers, a notification name, and no object.
        unsafe {
            let watch: Retained<Watch> = msg_send![Watch::class(), new];
            let center: Retained<AnyObject> =
                msg_send![class!(NSNotificationCenter), defaultCenter];
            for name in [
                "UIKeyboardWillChangeFrameNotification",
                "UIKeyboardWillHideNotification",
            ] {
                let name = NSString::from_str(name);
                let () = msg_send![&*center, addObserver: &*watch, selector: sel!(keyboardChanged:),
                    name: &*name, object: std::ptr::null::<AnyObject>()];
            }
            std::mem::forget(watch);
        }
    }
}
