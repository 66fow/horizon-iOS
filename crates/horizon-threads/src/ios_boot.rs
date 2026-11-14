//! Keeps the bundled launch design over the window until Metal presents its first frame.
use std::cell::RefCell;

use objc2::{MainThreadMarker, rc::Retained};
use objc2_foundation::{NSBundle, NSString};
use objc2_quartz_core::CATransaction;
use objc2_ui_kit::{UIApplication, UILabel, UIView, UIViewAutoresizing};

thread_local! {
    static OVERLAY: RefCell<Option<Retained<UIView>>> = const { RefCell::new(None) };
}

// winit's app uses the UIApplication lifecycle, not a UIScene manifest.
#[allow(deprecated)]
pub fn show(logs: &[String]) {
    use objc2_ui_kit::UIStoryboard;
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    OVERLAY.with_borrow_mut(|overlay| {
        if overlay.is_some() {
            return;
        }
        let name = NSString::from_str("LaunchScreen");
        let bundle = NSBundle::mainBundle();
        if bundle
            .pathForResource_ofType(Some(&name), Some(&NSString::from_str("storyboardc")))
            .is_none()
        {
            return;
        }
        let windows = UIApplication::sharedApplication(mtm).windows();
        let Some(window) = windows
            .iter()
            .find(|window| window.isKeyWindow())
            .or_else(|| windows.lastObject())
        else {
            return;
        };
        let storyboard = UIStoryboard::storyboardWithName_bundle(&name, Some(&bundle), mtm);
        let Some(controller) = storyboard.instantiateInitialViewController() else {
            return;
        };
        let Some(view) = controller.view() else {
            return;
        };
        view.setFrame(window.bounds());
        view.setAutoresizingMask(
            UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
        );
        view.setUserInteractionEnabled(false);
        if let Some(label) = view
            .viewWithTag(1001)
            .and_then(|view| view.downcast::<UILabel>().ok())
        {
            let lines = &logs[logs.len().saturating_sub(4)..];
            if !lines.is_empty() {
                label.setText(Some(&NSString::from_str(&lines.join("\n"))));
            }
        }
        window.addSubview(&view);
        view.layoutIfNeeded();
        // Commit before synchronous pipeline creation occupies the main thread.
        CATransaction::flush();
        *overlay = Some(view);
    });
}

pub fn hide() {
    if MainThreadMarker::new().is_none() {
        return;
    }
    OVERLAY.with_borrow_mut(|overlay| {
        if let Some(view) = overlay.take() {
            view.removeFromSuperview();
        }
    });
}
