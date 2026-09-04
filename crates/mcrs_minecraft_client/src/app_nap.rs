use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

/// App Nap moves a process whose window is not frontmost into the background scheduling band
/// after about a minute, whatever it is drawing: the same frame read 0.7 ms before and 4 ms
/// after. A latency-critical activity held for the life of the process opts out of it.
pub fn decline() {
    let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
            | NSActivityOptions::LatencyCritical,
        &NSString::from_str("rendering"),
    );
    std::mem::forget(activity);
}
