//! iOS: the helper threads (world generation, lighting, chunk meshing) at utility quality of
//! service. An iPhone has two fast cores and several efficiency ones; a helper at the default
//! class can take a fast core from the render or server thread, which a frame then waits
//! 10 to 200 ms to get back. Utility keeps the helpers on the efficiency cores while they are
//! enough. `HORIZON_HELPER_QOS=default` leaves them at the default class again.
use std::sync::OnceLock;

pub(super) fn helpers_behind() -> bool {
    static OFF: OnceLock<bool> = OnceLock::new();
    let off = *OFF.get_or_init(|| {
        std::env::var("HORIZON_HELPER_QOS").is_ok_and(|v| v.eq_ignore_ascii_case("default"))
    });
    if off {
        return false;
    }
    // SAFETY: sets the calling thread's own QoS class; no pointers.
    unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) == 0 }
}
