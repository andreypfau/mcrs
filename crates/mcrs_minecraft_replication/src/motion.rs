use std::collections::VecDeque;

use bytes::{Bytes, BytesMut};
use mcrs_engine_muxer::Tier;
use mcrs_minecraft_protocol::{Result, VarInt};

use crate::delta_base::{DeltaBase, Path, Pose, Waypoint};

/// The longest path that a tier sends, in ticks: the interval of the slowest tier.
const MAX_PATH_TICKS: u64 = 20;

/// The most ticks between two steps of a path. With a step at least this often, the client
/// keeps a slow subject in motion.
const STEP_TICKS: u64 = 4;

/// The recent poses of a subject and what each tier last sent of it.
#[derive(Debug)]
pub(crate) struct Motion {
    /// The poses of the last [`MAX_PATH_TICKS`] ticks, and the one before them.
    history: VecDeque<Posed>,
    tiers: [TierState; Tier::ALL.len()],
}

#[derive(Clone, Copy, Debug)]
struct Posed {
    tick: u64,
    pose: Pose,
}

#[derive(Clone, Debug)]
struct TierState {
    base: DeltaBase,
    /// The tick of the last encode of the tier, where its next path starts.
    sent_tick: u64,
    /// The next encode sends the absolute state.
    fresh: bool,
    /// The spawn packet at this base, until the base moves.
    spawn: Option<Bytes>,
}

impl Motion {
    pub(crate) fn new(pose: &Pose, tick: u64) -> Self {
        let tier = TierState {
            base: DeltaBase::new(pose, tick),
            sent_tick: tick,
            fresh: false,
            spawn: None,
        };
        Self {
            history: VecDeque::from([Posed { tick, pose: *pose }]),
            tiers: [tier.clone(), tier.clone(), tier.clone(), tier],
        }
    }

    pub(crate) fn latest(&self) -> Option<&Pose> {
        self.history.back().map(|posed| &posed.pose)
    }

    pub(crate) fn base(&self, tier: Tier) -> Option<&DeltaBase> {
        self.tiers.get(tier.index()).map(|state| &state.base)
    }

    pub(crate) fn record(&mut self, tick: u64, pose: &Pose) {
        if self.history.back().is_some_and(|posed| posed.tick == tick) {
            self.history.pop_back();
        }
        self.history.push_back(Posed { tick, pose: *pose });
        let oldest = tick.saturating_sub(MAX_PATH_TICKS);
        while self.history.get(1).is_some_and(|next| next.tick <= oldest) {
            self.history.pop_front();
        }
    }

    pub(crate) fn refresh(&mut self) {
        for state in &mut self.tiers {
            state.fresh = true;
        }
    }

    /// Appends the move of `id` at `tier` from its base to its latest pose at `tick`: one move
    /// at tier 0, a path of steps at the slower tiers.
    pub(crate) fn encode(
        &mut self,
        id: VarInt,
        tier: Tier,
        tick: u64,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let Some(latest) = self.latest().copied() else {
            return Ok(());
        };
        let waypoints = if tier == Tier::NEAREST {
            Vec::new()
        } else {
            self.path(tier, tick)
        };
        let Some(state) = self.tiers.get_mut(tier.index()) else {
            return Ok(());
        };
        let start = bytes.len();
        if state.fresh {
            state.base.sync(id, &latest, tick, bytes)?;
        } else if tier == Tier::NEAREST {
            let path = Path::Linear(latest.position);
            state.base.advance(id, path, latest.look, tick, bytes)?;
        } else {
            let path = Path::Stepped(&waypoints);
            state.base.advance(id, path, latest.look, tick, bytes)?;
        }
        state.fresh = false;
        state.sent_tick = tick;
        if bytes.len() > start {
            state.spawn = None;
        }
        Ok(())
    }

    /// The spawn packet of the subject at its base at `tier`, made once until the base moves.
    pub(crate) fn spawn(
        &mut self,
        tier: Tier,
        make: impl FnOnce(&DeltaBase) -> Result<Bytes>,
    ) -> Result<Option<Bytes>> {
        let Some(state) = self.tiers.get_mut(tier.index()) else {
            return Ok(None);
        };
        if let Some(packet) = &state.spawn {
            return Ok(Some(packet.clone()));
        }
        let packet = make(&state.base)?;
        Ok(Some(state.spawn.insert(packet).clone()))
    }

    /// The waypoints from the last encode of `tier` to `tick`, at most [`STEP_TICKS`] apart.
    fn path(&self, tier: Tier, tick: u64) -> Vec<Waypoint> {
        let sent = self
            .tiers
            .get(tier.index())
            .map_or(tick, |state| state.sent_tick);
        let from = sent.max(tick.saturating_sub(MAX_PATH_TICKS));
        let span = tick.saturating_sub(from);
        let steps = span.div_ceil(STEP_TICKS).max(1);
        let mut waypoints = Vec::new();
        let mut previous = from;
        for step in 1..=steps {
            let at = from + span * step / steps;
            if let Some(pose) = self.pose_at(at) {
                waypoints.push(Waypoint {
                    ticks: u32::try_from(at - previous).unwrap_or(u32::MAX),
                    position: pose.position,
                });
            }
            previous = at;
        }
        waypoints
    }

    /// The pose of the subject at the end of `tick`.
    fn pose_at(&self, tick: u64) -> Option<&Pose> {
        self.history
            .iter()
            .rev()
            .find(|posed| posed.tick <= tick)
            .or_else(|| self.history.front())
            .map(|posed| &posed.pose)
    }
}
