use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

pub fn decline() {
    let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
            | NSActivityOptions::LatencyCritical,
        &NSString::from_str("rendering"),
    );
    std::mem::forget(activity);
}
