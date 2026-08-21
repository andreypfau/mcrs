use bevy::prelude::*;
use objc2_foundation::{NSString, NSURL};
use objc2_metal::{
    MTLCaptureDescriptor, MTLCaptureDestination, MTLCaptureManager, MTLCreateSystemDefaultDevice,
};

use crate::{config, stream};

// Rendering runs a frame behind the update loop, so a couple of ticks after the capture
// opens may hold no complete frame at all; ten always covers several.
const FRAMES: u32 = 10;

pub fn gputrace(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    loader: Res<stream::Loader>,
    mut settled: Local<u32>,
    mut left: Local<u32>,
) {
    if *left > 0 {
        *left -= 1;
        if *left == 0 {
            unsafe { MTLCaptureManager::sharedCaptureManager() }.stopCapture();
            info!("gpu trace written");
            if config::gputrace_path().is_some() {
                commands.write_message(AppExit::Success);
            }
        }
        return;
    }
    if loader.done() {
        *settled += 1;
    }
    let auto = config::gputrace_path();
    let path = match (&auto, keys.just_pressed(KeyCode::F9)) {
        (Some(path), _) if *settled == 30 => path.clone(),
        (_, true) => "anvil_region_viewer.gputrace".to_string(),
        _ => return,
    };
    match start(&path) {
        Ok(()) => *left = FRAMES,
        Err(error) => error!("cannot capture {path}: {error}"),
    }
}

fn start(path: &str) -> Result<(), String> {
    let manager = unsafe { MTLCaptureManager::sharedCaptureManager() };
    if !manager.supportsDestination(MTLCaptureDestination::GPUTraceDocument) {
        return Err("Metal refuses to write a trace; relaunch with MTL_CAPTURE_ENABLED=1".into());
    }
    let device = MTLCreateSystemDefaultDevice().ok_or("this machine has no Metal device")?;
    let path = std::path::absolute(path).map_err(|error| error.to_string())?;
    // A .gputrace is a document package, and Metal refuses to write over one that is already there.
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
