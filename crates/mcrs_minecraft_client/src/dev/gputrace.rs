use bevy::prelude::*;
use objc2_foundation::{NSString, NSURL};
use objc2_metal::{
    MTLCaptureDescriptor, MTLCaptureDestination, MTLCaptureManager, MTLCreateSystemDefaultDevice,
};

use crate::stream;

const FRAMES: u32 = 10;

#[derive(Resource)]
pub struct AutoTrace(pub Option<String>);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "counts down the frames of an active capture and the settle frames of the automatic trace each tick"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        mcrs::blocking_in_system,
        reason = "the path and the removal of an old trace happen only when a capture starts"
    )
)]
pub fn gputrace(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    auto: Res<AutoTrace>,
    streaming: stream::Streaming,
    mut settled: Local<u32>,
    mut left: Local<u32>,
) {
    if *left > 0 {
        *left -= 1;
        if *left == 0 {
            // SAFETY: the shared capture manager is a process-wide singleton that Metal creates on first use.
            unsafe { MTLCaptureManager::sharedCaptureManager() }.stopCapture();
            info!("gpu trace written");
            if auto.0.is_some() {
                commands.write_message(AppExit::Success);
            }
        }
        return;
    }
    if auto.0.is_some() && streaming.done() {
        *settled += 1;
    }
    let path = match (&auto.0, keys.just_pressed(KeyCode::F9)) {
        (Some(path), _) if *settled == 30 => path.clone(),
        (_, true) => "mcrs.gputrace".to_string(),
        _ => return,
    };
    match start(&path) {
        Ok(()) => *left = FRAMES,
        Err(error) => error!("cannot capture {path}: {error}"),
    }
}

fn start(path: &str) -> Result<(), String> {
    // SAFETY: the shared capture manager is a process-wide singleton that Metal creates on first use.
    let manager = unsafe { MTLCaptureManager::sharedCaptureManager() };
    if !manager.supportsDestination(MTLCaptureDestination::GPUTraceDocument) {
        return Err("Metal refuses to write a trace; relaunch with MTL_CAPTURE_ENABLED=1".into());
    }
    let device = MTLCreateSystemDefaultDevice().ok_or("this machine has no Metal device")?;
    let path = std::path::absolute(path).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&path);
    let descriptor = MTLCaptureDescriptor::new();
    descriptor.set_capture_device(&device);
    descriptor.setDestination(MTLCaptureDestination::GPUTraceDocument);
    descriptor.setOutputURL(Some(&NSURL::fileURLWithPath(&NSString::from_str(
        &path.to_string_lossy(),
    ))));
    manager
        .startCaptureWithDescriptor_error(&descriptor)
        .map_err(|error| error.localizedDescription().to_string())
}
