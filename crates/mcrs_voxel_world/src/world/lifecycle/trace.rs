use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use bevy_platform::time::Instant;

use mcrs_voxel_math::ColumnPos;
use rustc_hash::FxHashMap;

/// How far a column has got on its way from a player's view ticket to a mesh on
/// screen. The order is the pipeline order: a mark only ever moves a column
/// forward, so a column that fell behind still reads as the stage it stalled in.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ColumnStage {
    Ticketed,
    Spawned,
    Queued,
    Generating,
    Loaded,
    Ready,
    Sent,
    Received,
    Meshed,
}

impl ColumnStage {
    pub const ALL: [ColumnStage; 9] = [
        ColumnStage::Ticketed,
        ColumnStage::Spawned,
        ColumnStage::Queued,
        ColumnStage::Generating,
        ColumnStage::Loaded,
        ColumnStage::Ready,
        ColumnStage::Sent,
        ColumnStage::Received,
        ColumnStage::Meshed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColumnStage::Ticketed => "ticket",
            ColumnStage::Spawned => "spawn",
            ColumnStage::Queued => "queue",
            ColumnStage::Generating => "gen",
            ColumnStage::Loaded => "load",
            ColumnStage::Ready => "ready",
            ColumnStage::Sent => "sent",
            ColumnStage::Received => "recv",
            ColumnStage::Meshed => "mesh",
        }
    }
}

pub const STAGES: usize = ColumnStage::ALL.len();

#[derive(Clone, Copy)]
struct Trace {
    stage: ColumnStage,
    entered: Instant,
    first: Instant,
    source: Option<&'static str>,
    sent_after: Option<Duration>,
    /// When each stage was entered, after `first`; a stage the column skipped stays empty.
    reached: [Option<Duration>; STAGES],
}

pub struct ColumnSample {
    pub pos: ColumnPos,
    pub stage: ColumnStage,
    pub in_stage: Duration,
    pub age: Duration,
    pub source: Option<&'static str>,
    pub sent_after: Option<Duration>,
    pub reached: [Option<Duration>; STAGES],
}

impl ColumnSample {
    /// How long the column spent getting from the stage before `stage` to `stage`, or
    /// `None` when it has not reached it.
    pub fn hop(&self, stage: ColumnStage) -> Option<Duration> {
        let at = self.reached[stage as usize]?;
        let before = self.reached[..stage as usize]
            .iter()
            .rev()
            .find_map(|earlier| *earlier)
            .unwrap_or(Duration::ZERO);
        Some(at.saturating_sub(before))
    }
}

/// A single-player client hosts its server in this same process, so the debug
/// map reads the server's column lifecycle straight out of here rather than
/// over the wire.
///
/// ponytail: process-local. Against a remote server only the client's own
/// `Received`/`Meshed` marks land, and the earlier stages read as unknown; a
/// debug packet is the upgrade.
static TRACES: LazyLock<Mutex<FxHashMap<ColumnPos, Trace>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));

/// A player who keeps walking one way leaves entries behind that no unload ever
/// claims, so the map is dropped wholesale rather than grown without bound.
const CAPACITY: usize = 1 << 16;

pub fn mark(pos: ColumnPos, stage: ColumnStage) {
    let Ok(mut traces) = TRACES.lock() else {
        return;
    };
    let now = Instant::now();
    match traces.get_mut(&pos) {
        Some(trace) => {
            if stage <= trace.stage {
                return;
            }
            trace.stage = stage;
            trace.entered = now;
            trace.reached[stage as usize] = Some(now - trace.first);
            if stage == ColumnStage::Sent {
                trace.sent_after = Some(now - trace.first);
            }
        }
        None => {
            if traces.len() >= CAPACITY {
                traces.clear();
            }
            let mut reached = [None; STAGES];
            reached[stage as usize] = Some(Duration::ZERO);
            traces.insert(
                pos,
                Trace {
                    stage,
                    entered: now,
                    first: now,
                    source: None,
                    sent_after: None,
                    reached,
                },
            );
        }
    }
}

pub fn set_source(pos: ColumnPos, source: &'static str) {
    if let Ok(mut traces) = TRACES.lock()
        && let Some(trace) = traces.get_mut(&pos)
    {
        trace.source = Some(source);
    }
}

pub fn forget(pos: ColumnPos) {
    if let Ok(mut traces) = TRACES.lock() {
        traces.remove(&pos);
    }
}

pub fn snapshot(out: &mut Vec<ColumnSample>) {
    out.clear();
    let Ok(traces) = TRACES.lock() else {
        return;
    };
    let now = Instant::now();
    out.extend(traces.iter().map(|(pos, trace)| ColumnSample {
        pos: *pos,
        stage: trace.stage,
        in_stage: now - trace.entered,
        age: now - trace.first,
        source: trace.source,
        sent_after: trace.sent_after,
        reached: trace.reached,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_advances_but_never_rewinds() {
        let pos = ColumnPos::new(i32::MAX - 7, i32::MAX - 11);
        forget(pos);

        mark(pos, ColumnStage::Ticketed);
        mark(pos, ColumnStage::Loaded);
        mark(pos, ColumnStage::Queued);
        set_source(pos, "saved");
        mark(pos, ColumnStage::Sent);

        let mut out = Vec::new();
        snapshot(&mut out);
        let sample = out
            .iter()
            .find(|sample| sample.pos == pos)
            .expect("the marked column is in the snapshot");
        assert_eq!(sample.stage, ColumnStage::Sent);
        assert_eq!(sample.source, Some("saved"));
        assert!(sample.sent_after.is_some(), "sent_after timed the ladder");

        forget(pos);
        let mut out = Vec::new();
        snapshot(&mut out);
        assert!(out.iter().all(|sample| sample.pos != pos));
    }
}
