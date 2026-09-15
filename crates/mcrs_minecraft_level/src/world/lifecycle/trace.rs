use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bevy_ecs::prelude::{ResMut, Resource};
use bevy_platform::time::Instant;

use mcrs_minecraft_core::ColumnPos;
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
    Filled,
    Ran,
    Merged,
    Loaded,
    Ready,
    Sent,
    Received,
    Meshed,
}

impl ColumnStage {
    pub const ALL: [ColumnStage; 12] = [
        ColumnStage::Ticketed,
        ColumnStage::Spawned,
        ColumnStage::Queued,
        ColumnStage::Generating,
        ColumnStage::Filled,
        ColumnStage::Ran,
        ColumnStage::Merged,
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
            ColumnStage::Filled => "fill",
            ColumnStage::Ran => "run",
            ColumnStage::Merged => "merge",
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

#[derive(Clone, Copy, Debug)]
pub enum TraceEvent {
    Mark {
        pos: ColumnPos,
        stage: ColumnStage,
        at: Instant,
    },
    Source {
        pos: ColumnPos,
        source: &'static str,
    },
    Forget(ColumnPos),
}

impl TraceEvent {
    pub fn mark(pos: ColumnPos, stage: ColumnStage) -> Self {
        Self::Mark {
            pos,
            stage,
            at: Instant::now(),
        }
    }
}

/// A player who keeps walking one way leaves entries behind that no unload ever
/// claims, so the map is dropped wholesale rather than grown without bound.
const CAPACITY: usize = 1 << 16;

#[derive(Default)]
pub struct ColumnTraces(FxHashMap<ColumnPos, Trace>);

impl ColumnTraces {
    pub fn apply(&mut self, event: TraceEvent) {
        match event {
            TraceEvent::Mark { pos, stage, at } => self.mark(pos, stage, at),
            TraceEvent::Source { pos, source } => {
                if let Some(trace) = self.0.get_mut(&pos) {
                    trace.source = Some(source);
                }
            }
            TraceEvent::Forget(pos) => {
                self.0.remove(&pos);
            }
        }
    }

    fn mark(&mut self, pos: ColumnPos, stage: ColumnStage, now: Instant) {
        match self.0.get_mut(&pos) {
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
                if self.0.len() >= CAPACITY {
                    self.0.clear();
                }
                let mut reached = [None; STAGES];
                reached[stage as usize] = Some(Duration::ZERO);
                self.0.insert(
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

    pub fn snapshot(&self, out: &mut Vec<ColumnSample>, now: Instant) {
        out.clear();
        out.extend(self.0.iter().map(|(pos, trace)| ColumnSample {
            pos: *pos,
            stage: trace.stage,
            in_stage: now - trace.entered,
            age: now - trace.first,
            source: trace.source,
            sent_after: trace.sent_after,
            reached: trace.reached,
        }));
    }
}

/// What a dimension has traced since the host last took it. Present only when the host shares
/// its traces with a client, so a server nobody reads the trace of records nothing.
#[derive(Resource, Default)]
pub struct ColumnTraceLog(Vec<TraceEvent>);

impl ColumnTraceLog {
    pub fn drain(&mut self) -> impl Iterator<Item = TraceEvent> + '_ {
        self.0.drain(..)
    }
}

pub fn mark(log: &mut Option<ResMut<ColumnTraceLog>>, pos: ColumnPos, stage: ColumnStage) {
    if let Some(log) = log {
        log.0.push(TraceEvent::mark(pos, stage));
    }
}

pub fn set_source(log: &mut Option<ResMut<ColumnTraceLog>>, pos: ColumnPos, source: &'static str) {
    if let Some(log) = log {
        log.0.push(TraceEvent::Source { pos, source });
    }
}

pub fn forget(log: &mut Option<ResMut<ColumnTraceLog>>, pos: ColumnPos) {
    if let Some(log) = log {
        log.0.push(TraceEvent::Forget(pos));
    }
}

/// A single-player client hosts its server in this same process, so the debug
/// map reads the server's column lifecycle through the sink both hold rather
/// than over the wire.
///
/// ponytail: against a remote server only the client's own `Received`/`Meshed`
/// marks land, and the earlier stages read as unknown; a debug packet is the
/// upgrade.
#[derive(Resource, Clone, Default)]
pub struct ColumnTraceSink(Arc<Mutex<FxHashMap<String, ColumnTraces>>>);

impl ColumnTraceSink {
    pub fn record(&self, dimension: &str, events: impl IntoIterator<Item = TraceEvent>) {
        let mut dimensions = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut events = events.into_iter().peekable();
        if events.peek().is_none() {
            return;
        }
        let traces = match dimensions.get_mut(dimension) {
            Some(traces) => traces,
            None => dimensions.entry(dimension.to_owned()).or_default(),
        };
        events.for_each(|event| traces.apply(event));
    }

    pub fn snapshot(&self, dimension: &str, out: &mut Vec<ColumnSample>) {
        let dimensions = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match dimensions.get(dimension) {
            Some(traces) => traces.snapshot(out, Instant::now()),
            None => out.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_advances_but_never_rewinds() {
        let pos = ColumnPos::new(3, -4);
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut traces = ColumnTraces::default();
        for event in [
            TraceEvent::Mark {
                pos,
                stage: ColumnStage::Ticketed,
                at: at(0),
            },
            TraceEvent::Mark {
                pos,
                stage: ColumnStage::Loaded,
                at: at(5),
            },
            TraceEvent::Mark {
                pos,
                stage: ColumnStage::Queued,
                at: at(7),
            },
            TraceEvent::Source {
                pos,
                source: "saved",
            },
            TraceEvent::Mark {
                pos,
                stage: ColumnStage::Sent,
                at: at(9),
            },
        ] {
            traces.apply(event);
        }

        let mut out = Vec::new();
        traces.snapshot(&mut out, at(10));
        let [sample] = out.as_slice() else {
            panic!("one column traced, got {}", out.len());
        };
        assert_eq!(sample.stage, ColumnStage::Sent);
        assert_eq!(sample.source, Some("saved"));
        assert_eq!(sample.sent_after, Some(Duration::from_millis(9)));
        assert_eq!(
            sample.reached[ColumnStage::Queued as usize],
            None,
            "a rewind is ignored"
        );

        traces.apply(TraceEvent::Forget(pos));
        traces.snapshot(&mut out, at(11));
        assert!(out.is_empty());
    }

    #[test]
    fn each_dimension_keeps_its_own_trace() {
        let sink = ColumnTraceSink::default();
        let pos = ColumnPos::new(0, 0);
        sink.record(
            "minecraft:overworld",
            [TraceEvent::mark(pos, ColumnStage::Sent)],
        );

        let mut out = Vec::new();
        sink.snapshot("minecraft:the_nether", &mut out);
        assert!(out.is_empty(), "the same column elsewhere is untraced");
        sink.snapshot("minecraft:overworld", &mut out);
        assert_eq!(out.len(), 1);
    }
}
