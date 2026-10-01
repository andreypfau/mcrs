//! Tracy shows a frame preview only for frames the client sent an image for, so
//! the profiler has nothing to draw until the window is read back and handed to
//! it explicitly.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

/// Tracy wants both dimensions divisible by four.
const WIDTH: u32 = 320;

const HEIGHT: u32 = 180;

/// A readback per frame at window resolution would dominate the very timings
/// being profiled, so the next capture waits for the one in flight and the
/// rate settles wherever the readback keeps up.
#[derive(Resource, Default)]
pub(super) struct Preview {
    frame: u32,
    in_flight: Option<u32>,
}

pub(super) fn request(mut commands: Commands, mut preview: ResMut<Preview>) {
    preview.frame = preview.frame.wrapping_add(1);
    if preview.in_flight.is_some() {
        return;
    }
    preview.in_flight = Some(preview.frame);
    commands.spawn(Screenshot::primary_window()).observe(send);
}

fn send(captured: On<ScreenshotCaptured>, mut preview: ResMut<Preview>) {
    let captured_at = preview.in_flight.take();
    let rgba = match captured.image.clone().try_into_dynamic() {
        Ok(image) => image.into_rgba8(),
        Err(err) => {
            error!("cannot convert the screenshot for tracy: {err}");
            return;
        }
    };
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return;
    }
    let (out_width, out_height) = fit(width, height);
    let mut scaled = Vec::with_capacity((out_width * out_height * 4) as usize);
    for y in 0..out_height {
        let source_y = (y * height / out_height).min(height - 1);
        for x in 0..out_width {
            let source_x = (x * width / out_width).min(width - 1);
            let pixel = rgba.get_pixel(source_x, source_y).0;
            scaled.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
        }
    }
    // Tracy pins the image back onto the frame that produced it, which is as
    // many frames ago as the readback took to land here.
    let delay = captured_at
        .map(|at| preview.frame.wrapping_sub(at))
        .unwrap_or(0);
    tracing_tracy::client::frame_image(
        &scaled,
        out_width as u16,
        out_height as u16,
        delay.try_into().unwrap_or(u8::MAX),
        false,
    );
}

/// The largest box within `WIDTH` by `HEIGHT` that keeps the window's aspect
/// ratio, rounded down to what Tracy accepts.
fn fit(width: u32, height: u32) -> (u32, u32) {
    let (mut out_width, mut out_height) = (width, height);
    if out_width > WIDTH {
        out_width = WIDTH;
        out_height = height * WIDTH / width;
    }
    if out_height > HEIGHT {
        out_width = width * HEIGHT / height;
        out_height = HEIGHT;
    }
    ((out_width & !3).max(4), (out_height & !3).max(4))
}

#[cfg(test)]
mod tests {
    use super::fit;

    #[test]
    fn fit_stays_within_tracy_limits() {
        for (width, height) in [(2560, 1440), (1920, 1080), (1024, 768), (100, 3000), (7, 5)] {
            let (out_width, out_height) = fit(width, height);
            assert!(out_width <= super::WIDTH && out_height <= super::HEIGHT);
            assert_eq!((out_width % 4, out_height % 4), (0, 0));
        }
    }
}
