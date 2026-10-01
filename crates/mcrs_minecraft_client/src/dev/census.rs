use std::time::Duration;

use bevy::prelude::*;
use mcrs_minecraft_level::world::lifecycle::trace::{ColumnSample, ColumnStage};

use crate::gui::chunk_map::JoinedTraces;

#[derive(Resource)]
pub struct Census {
    interval: Duration,
    next: Duration,
    started: std::time::Instant,
}

impl Census {
    pub fn every(seconds: f32) -> Self {
        Self {
            interval: Duration::from_secs_f32(seconds.max(0.05)),
            next: Duration::ZERO,
            started: std::time::Instant::now(),
        }
    }
}

pub fn census(
    mut census: ResMut<Census>,
    time: Res<Time>,
    mut samples: Local<Vec<ColumnSample>>,
    mut sent: Local<Vec<Duration>>,
    traces: JoinedTraces,
) {
    let now = time.elapsed();
    if now < census.next {
        return;
    }
    census.next = now + census.interval;

    traces.snapshot(&mut samples);
    let mut counts = [0usize; ColumnStage::ALL.len()];
    sent.clear();
    for sample in samples.iter() {
        counts[sample.stage as usize] += 1;
        if let Some(after) = sample.sent_after {
            sent.push(after);
        }
    }
    sent.sort_unstable();
    let quantile = |sorted: &[Duration], at: f32| -> f32 {
        if sorted.is_empty() {
            return f32::NAN;
        }
        let index = ((sorted.len() - 1) as f32 * at).round() as usize;
        sorted[index].as_secs_f32() * 1000.0
    };
    let tally = ColumnStage::ALL
        .iter()
        .map(|stage| format!("{}={}", stage.label(), counts[*stage as usize]))
        .collect::<Vec<_>>()
        .join(" ");
    let mut hop = Vec::new();
    let hops = ColumnStage::ALL
        .iter()
        .skip(1)
        .map(|stage| {
            hop.clear();
            hop.extend(samples.iter().filter_map(|sample| sample.hop(*stage)));
            hop.sort_unstable();
            format!(
                "{}={:.0}/{:.0}",
                stage.label(),
                quantile(&hop, 0.5),
                quantile(&hop, 0.95)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    info!(
        target: "census",
        "census t={:.1}s traced={} {tally} sent_p50={:.0}ms sent_p95={:.0}ms hop_p50/p95 {hops}",
        census.started.elapsed().as_secs_f32(),
        samples.len(),
        quantile(&sent, 0.5),
        quantile(&sent, 0.95),
    );
}
