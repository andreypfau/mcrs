use std::fmt::Write as _;

use bevy::prelude::*;

use crate::render::DrawnTriangles;
use crate::{cave, config, daylight, mesh, probe, render, stream};

#[derive(Resource)]
pub struct FrameStats {
    times: Box<[f32; FrameStats::CAPACITY]>,
    sorted: Box<[f32; FrameStats::CAPACITY]>,
    frames: u32,
    written: usize,
    elapsed: f32,
    line: String,
}

impl FrameStats {
    const CAPACITY: usize = 4096;

    pub fn new() -> Self {
        Self {
            times: Box::new([0.0; Self::CAPACITY]),
            sorted: Box::new([0.0; Self::CAPACITY]),
            frames: 0,
            written: 0,
            elapsed: 0.0,
            line: String::new(),
        }
    }
}

pub fn spawn(mut commands: Commands) {
    if !config::overlay_shown() {
        return;
    }
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
}

pub fn frame_stats(
    time: Res<Time>,
    mut stats: ResMut<FrameStats>,
    triangles: Res<DrawnTriangles>,
    gpu: Res<probe::GpuTimings>,
    streams: Res<render::Streams>,
    cave: Res<cave::CaveCull>,
    loader: Res<stream::Loader>,
    day: Res<daylight::TimeOfDay>,
    overlay: Option<Single<&mut Text>>,
    window: Single<&Window>,
) {
    let delta = time.delta_secs();
    if delta <= 0.0 {
        return;
    }
    stats.elapsed += delta;
    stats.frames += 1;
    let slot = stats.written % FrameStats::CAPACITY;
    stats.times[slot] = delta * 1000.0;
    stats.written += 1;

    if stats.elapsed < 1.0 {
        return;
    }

    let stats = &mut *stats;
    let samples = stats.written.min(FrameStats::CAPACITY);
    stats.sorted[..samples].copy_from_slice(&stats.times[..samples]);
    stats.sorted[..samples].sort_unstable_by(f32::total_cmp);
    let fps = stats.frames as f32 / stats.elapsed;
    let p95 = stats.sorted[percentile_index(samples, 0.95)];
    let p99 = stats.sorted[percentile_index(samples, 0.99)];

    let line = &mut stats.line;
    line.clear();
    let _ = write!(
        line,
        "{fps:.0} fps @ {}x{}   {} tris   p95 {p95:.1} ms   p99 {p99:.1} ms",
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        triangles.get(),
    );
    if cave.enabled {
        let _ = write!(line, "   cave {} sections", cave.reached());
        if let Some(ms) = cave.took_ms() {
            let _ = write!(line, " in {ms:.3} ms");
        }
    } else {
        line.push_str("   cave off");
    }
    let status = loader.status();
    let _ = write!(
        line,
        "   arena {:.0}/{:.0}%",
        status.quads * 100.0,
        status.models * 100.0,
    );
    let _ = write!(line, "   {}/{} regions", status.regions, status.regions_total);
    if status.files < status.files_total {
        let _ = write!(line, "   loading {}/{} files", status.files, status.files_total);
    }
    if status.evicted > 0 {
        let _ = write!(line, "   {} evicted", status.evicted);
    }
    for (stream, name) in mesh::STREAM_NAMES.iter().enumerate() {
        if streams.0 & (1 << stream) == 0 {
            let _ = write!(line, "   no {name}");
        }
    }
    for (slot, name) in probe::NAMES.iter().enumerate() {
        if let Some(ms) = gpu.median(slot) {
            let _ = write!(line, "   {name} {ms:.2} ms");
        }
    }
    let (hour, minute) = day.clock();
    let _ = write!(line, "   {hour:02}:{minute:02}");
    info!("{}", line);
    if let Some(mut overlay) = overlay {
        overlay.0.clear();
        overlay.0.push_str(line);
    }

    stats.frames = 0;
    stats.written = 0;
    stats.elapsed = 0.0;
}

#[inline]
fn percentile_index(samples: usize, fraction: f32) -> usize {
    (((samples - 1) as f32) * fraction).round() as usize
}
