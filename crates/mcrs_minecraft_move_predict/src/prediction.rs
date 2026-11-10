use std::collections::VecDeque;
use std::mem;

use glam::DVec3;
use mcrs_minecraft_physics::BlockOverlay;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::Observation;
use mcrs_minecraft_physics::Orientation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Relative;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Teleport;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::apply_teleport;
use mcrs_minecraft_physics::observed_velocity;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::Change;
use crate::ClientTick;
use crate::Limits;
use crate::Mode;
use crate::Outcome;
use crate::Pusher;
use crate::candidates;
use crate::candidates::Candidate;
use crate::candidates::Context;
use crate::candidates::Record;
use crate::candidates::Scored;
use crate::candidates::Search;
use crate::candidates::Stop;
use crate::candidates::Work;
use crate::candidates::world_view;
use crate::kept_changes::KeptChanges;
use crate::rollback::Edit;
use crate::rollback::Rollback;

/// The fields of a teleport that add to the value of the client.
const KEPT_FROM_CLIENT: Relative = Relative::X
    .union(Relative::Y)
    .union(Relative::Z)
    .union(Relative::VELOCITY_X)
    .union(Relative::VELOCITY_Y)
    .union(Relative::VELOCITY_Z);

/// Layer 2 for one player: the candidates of its client, the record of its last client ticks
/// and the response to a violation.
#[derive(Clone, Debug)]
pub struct Prediction {
    limits: Limits,
    /// The first candidate of the accepted state; the server takes its results from it.
    first: Candidate,
    others: Vec<Candidate>,
    reference: DVec3,
    rotation: Orientation,
    kept: KeptChanges,
    records: VecDeque<Record>,
    buffer: f64,
    pending: Option<Teleport>,
    resync: bool,
    after_accept: bool,
    rollback: Option<Rollback>,
    work: Work,
    /// The start candidates of a record that left, for the next record.
    spare: Vec<Candidate>,
}

impl Prediction {
    /// A player in `state`, with no change sent yet.
    #[must_use]
    pub fn new(state: PlayerState, limits: Limits) -> Self {
        Self {
            limits,
            first: Candidate { state, applied: 0 },
            others: Vec::new(),
            reference: state.body.position,
            rotation: Orientation::default(),
            kept: KeptChanges::default(),
            records: VecDeque::new(),
            buffer: 0.0,
            pending: None,
            resync: false,
            after_accept: false,
            rollback: None,
            work: Work::default(),
            spare: Vec::new(),
        }
    }

    /// The state that the server takes its results of the movement from.
    #[must_use]
    pub fn accepted(&self) -> &PlayerState {
        &self.first.state
    }

    /// A predicted-state change goes out, in the order that layer 1 numbers the changes.
    pub fn change(&mut self, change: Change) {
        let number = self.kept.push(change);
        if change == Change::BlockAck
            && let Some(rollback) = &mut self.rollback
        {
            rollback.ack.get_or_insert(number);
        }
    }

    /// The client edits the block `at` with a sequence. From the next client tick on, its world
    /// view has `state` at `at` until it applies the acknowledgement, and it can move itself back
    /// to where it was at the first request for that block. `state` is the new state of an
    /// accepted edit, or the state that a refused action gives in the world of the server; `None`
    /// when the server cannot compute it.
    pub fn block_requested(&mut self, at: BlockPos, state: Option<BlockStateId>) {
        let position = self.first.state.body.position;
        let rollback = match &mut self.rollback {
            Some(rollback) if rollback.ack.is_none() => rollback,
            _ => self.rollback.insert(Rollback::default()),
        };
        match rollback.edits.iter_mut().find(|edit| edit.block == at) {
            Some(edit) => edit.state = state,
            None => rollback.edits.push(Edit {
                block: at,
                position,
                state,
            }),
        }
    }

    /// A teleport goes out; no client tick is predicted until its accept.
    pub fn teleport(&mut self, teleport: Teleport) {
        self.pending = Some(teleport);
    }

    /// The accept of the pending teleport, at the position that the client reports. The
    /// prediction starts again at the destination of the candidate that the accept matches.
    pub fn accept(&mut self, position: DVec3) -> Outcome {
        let Some(teleport) = self.pending else {
            return Outcome::from(Mode::Exempt);
        };
        let destination = |candidate: &Candidate| {
            let mut moved = *candidate;
            let mut rotation = self.rotation;
            apply_teleport(&mut moved.state, &mut rotation, &teleport);
            (moved, rotation)
        };
        let threshold = self.limits.offset_threshold;
        let found = self
            .candidates()
            .map(destination)
            .find(|(moved, _)| moved.state.body.position.distance(position) < threshold);
        let Some((moved, rotation)) = found else {
            let (moved, _) = destination(&self.first);
            return Outcome {
                offset: Some(moved.state.body.position.distance(position)),
                setback: Some(self.set_back_to(moved.state)),
                ..Outcome::default()
            };
        };
        self.restart(moved);
        self.rotation = rotation;
        self.reference = moved.state.body.position;
        self.pending = None;
        self.rollback = None;
        self.after_accept = true;
        Outcome::default()
    }

    /// Predicts one client tick from the tick packets and the world view of the client.
    /// `pushers` holds each living entity and boat near the player that the client has and that
    /// the team rules can let push the player. Its hull also holds the position before the
    /// oldest server tick that the client shows, because the client interpolates from it.
    pub fn tick<W: CollisionWorld, B: BodyView>(
        &mut self,
        view: View<'_, W, B>,
        pushers: &[Pusher],
        tick: &ClientTick,
    ) -> Outcome {
        self.rotation = tick.input.rotation;
        if let Some(teleport) = self.pending {
            if teleport.relative.intersects(KEPT_FROM_CLIENT) {
                self.follow(view, pushers, tick);
            }
            return Outcome::from(Mode::Exempt);
        }
        if self
            .rollback
            .as_ref()
            .is_some_and(|rollback| rollback.acked(tick.changes.applied))
        {
            self.rollback = None;
        }
        let outcome = if self.resync {
            self.resync(view, tick)
        } else {
            self.predict(view, pushers, tick)
        };
        // A candidate never applies fewer changes than the start it ran from, so the oldest
        // record holds the fewest.
        let oldest = match self.records.front() {
            Some(record) => record.start.as_slice(),
            None => &[],
        };
        let keep = oldest
            .iter()
            .chain(self.candidates())
            .map(|candidate| candidate.applied)
            .min()
            .unwrap_or(self.first.applied);
        self.kept.trim(keep);
        outcome
    }

    /// A relative teleport applies to the state of the client when it arrives, so the candidates
    /// follow the client ticks that run before that, with no check.
    fn follow<W: CollisionWorld, B: BodyView>(
        &mut self,
        view: View<'_, W, B>,
        pushers: &[Pusher],
        tick: &ClientTick,
    ) {
        let mut start = mem::take(&mut self.spare);
        start.clear();
        start.extend(self.candidates());
        let mut record = Record {
            start,
            tick: tick.clone(),
            reference: self.reference,
            rollback: self.rollback.clone(),
            explored: false,
        };
        let context = Context {
            view,
            pushers,
            kept: &self.kept,
            limits: &self.limits,
        };
        let ran = candidates::run_tick(&context, &mut record, Search::All, &mut 0, &mut self.work);
        let matched = 1..=self.limits.max_candidates;
        if ran.is_ok() && matched.contains(&self.work.matched.len()) {
            self.accept_matched();
            if let Some(position) = tick.reported.position {
                self.reference = position;
            }
        }
        self.spare = record.start;
    }

    fn predict<W: CollisionWorld, B: BodyView>(
        &mut self,
        view: View<'_, W, B>,
        pushers: &[Pusher],
        tick: &ClientTick,
    ) -> Outcome {
        let mut start = mem::take(&mut self.spare);
        start.clear();
        start.extend(self.candidates());
        let mut record = Record {
            start,
            tick: tick.clone(),
            reference: self.reference,
            rollback: self.rollback.clone(),
            explored: false,
        };
        let mut steps = 0;
        let context = Context {
            view,
            pushers,
            kept: &self.kept,
            limits: &self.limits,
        };
        let best = candidates::run_tick(
            &context,
            &mut record,
            Search::First,
            &mut steps,
            &mut self.work,
        );
        self.records.push_back(record);
        let result = best.and_then(|best| {
            let matched = !self.work.matched.is_empty()
                || replay(&context, &mut self.records, &mut steps, &mut self.work)?;
            Ok((best, matched))
        });
        let outcome = match result {
            Ok((_, true)) => self.matched(tick),
            Ok((best, false)) => self.violation(tick, best),
            Err(stop) => self.stopped(tick, stop),
        };
        if outcome.setback.is_none() && outcome.mode == Mode::Predicted {
            self.drop_old_record(view, pushers);
        }
        if let Some(position) = tick.reported.position
            && outcome.setback.is_none()
        {
            self.reference = position;
        }
        outcome
    }

    /// Before a record leaves, a tick in which the fast path left candidates out runs them all,
    /// so the accepted state holds each candidate that a later replay could need.
    fn drop_old_record<W: CollisionWorld, B: BodyView>(
        &mut self,
        view: View<'_, W, B>,
        pushers: &[Pusher],
    ) {
        if self.records.len() <= self.limits.replay_depth {
            return;
        }
        let context = Context {
            view,
            pushers,
            kept: &self.kept,
            limits: &self.limits,
        };
        let unexplored = self.records.front().is_some_and(|record| !record.explored);
        let mut steps = 0;
        if unexplored
            && replay(&context, &mut self.records, &mut steps, &mut self.work) == Ok(true)
            && self.work.matched.len() <= self.limits.max_candidates
        {
            self.accept_matched();
        }
        if let Some(record) = self.records.pop_front() {
            self.spare = record.start;
        }
    }

    fn matched(&mut self, tick: &ClientTick) -> Outcome {
        if self.work.matched.len() > self.limits.max_candidates {
            return self.coarse(tick);
        }
        let reported = tick.reported;
        let on_ground = self
            .work
            .matched
            .iter()
            .any(|candidate| candidate.state.body.flags.on_ground == reported.on_ground);
        let horizontal = self.work.matched.iter().any(|candidate| {
            candidate.state.body.flags.horizontal_collision == reported.horizontal_collision
        });
        self.accept_matched();
        self.buffer *= self.limits.buffer_decay;
        let first_after_accept = mem::take(&mut self.after_accept);
        Outcome {
            wrong_on_ground: !on_ground && !first_after_accept,
            wrong_horizontal_collision: !horizontal,
            ..Outcome::default()
        }
    }

    fn violation(&mut self, tick: &ClientTick, best: Option<Scored>) -> Outcome {
        let Some(best) = best else {
            return self.coarse(tick);
        };
        self.buffer += best.offset;
        let send = self.buffer >= self.limits.buffer_limit
            || best.offset >= self.limits.instant_setback_offset;
        self.buffer = self.buffer.min(self.limits.buffer_cap);
        let mut outcome = Outcome {
            offset: Some(best.offset),
            ..Outcome::default()
        };
        if send {
            outcome.setback = Some(self.set_back_to(self.first.state));
            return outcome;
        }
        let mut candidate = best.candidate;
        if let Some(position) = tick.reported.position {
            candidate.state.body.position = position;
        }
        self.set_accepted(&[candidate]);
        self.after_accept = false;
        outcome
    }

    /// The tick reached the cap of the physics steps, the push bound cannot hold the pushers, or
    /// the movement touches an edited block that is not settled.
    fn stopped(&mut self, tick: &ClientTick, stop: Stop) -> Outcome {
        let open = tick.changes.seen.saturating_sub(tick.changes.applied);
        if stop == Stop::Steps && open > self.limits.max_open_windows {
            return Outcome {
                setback: Some(self.set_back_to(self.first.state)),
                ..Outcome::default()
            };
        }
        self.coarse(tick)
    }

    /// Accepts the reported position with the velocity of the first candidate; the next tick
    /// with a position resyncs the velocity.
    fn coarse(&mut self, tick: &ClientTick) -> Outcome {
        let mut first = self.first;
        let applied = first.applied.max(tick.changes.applied);
        self.kept.apply(&mut first.state, first.applied, applied);
        first.applied = applied;
        if let Some(position) = tick.reported.position {
            first.state.body.position = position;
        }
        self.restart(first);
        self.resync = true;
        Outcome::from(Mode::Coarse)
    }

    /// The first position after a coarse tick: the reported position, with each velocity that
    /// the change of position can give.
    fn resync<W: CollisionWorld, B: BodyView>(
        &mut self,
        view: View<'_, W, B>,
        tick: &ClientTick,
    ) -> Outcome {
        let Some(position) = tick.reported.position else {
            return self.coarse(tick);
        };
        let mut start = self.first;
        let applied = start.applied.max(tick.changes.seen);
        self.kept.apply(&mut start.state, start.applied, applied);
        world_view(
            &self.kept,
            self.rollback.as_ref(),
            applied,
            &mut self.work.undone,
        );
        let world = BlockOverlay {
            base: view.world,
            blocks: &self.work.undone,
        };
        let client = View::new(&world, view.bodies, view.tables);
        let mut end = start.state;
        tick_player(
            client,
            Side::OwnClient,
            &start.state,
            &tick.input,
            &mut end,
            &mut self.work.actions,
            &mut self.work.scratch,
        );
        self.work.actions.clear();
        end.body.flags = MoveFlags {
            on_ground: tick.reported.on_ground,
            horizontal_collision: tick.reported.horizontal_collision,
            ..end.body.flags
        };
        let observation = Observation {
            displacement: position - start.state.body.position,
            end_flags: end.body.flags,
            end_support: end.body.support,
            flight: end.flight0,
            jump: tick.input.keys.jump,
            shift: tick.input.keys.shift,
        };
        let velocities = observed_velocity(client, &start.state, &observation);
        end.body.position = position;
        let resynced: Vec<Candidate> = velocities
            .as_slice()
            .iter()
            .map(|&velocity| {
                let mut state = end;
                state.body.velocity = velocity;
                Candidate { state, applied }
            })
            .collect();
        self.set_accepted(&resynced);
        self.records.clear();
        self.resync = false;
        self.reference = position;
        Outcome::from(Mode::Resync)
    }

    /// Refuses the tick and sends the client to `state`: absolute position and velocity, and
    /// the rotation of the client kept.
    fn set_back_to(&mut self, state: PlayerState) -> Teleport {
        let correction = Teleport {
            position: state.body.position,
            velocity: state.body.velocity,
            yaw: 0.0,
            pitch: 0.0,
            relative: Relative::YAW | Relative::PITCH,
        };
        self.pending = Some(correction);
        self.records.clear();
        correction
    }

    fn restart(&mut self, candidate: Candidate) {
        self.set_accepted(&[candidate]);
        self.records.clear();
        self.resync = false;
        self.after_accept = false;
    }

    fn accept_matched(&mut self) {
        let matched = mem::take(&mut self.work.matched);
        self.set_accepted(&matched);
        self.work.matched = matched;
    }

    fn set_accepted(&mut self, candidates: &[Candidate]) {
        let Some((first, others)) = candidates.split_first() else {
            return;
        };
        self.first = *first;
        self.others.clear();
        self.others.extend_from_slice(others);
    }

    fn candidates(&self) -> impl Iterator<Item = &Candidate> {
        std::iter::once(&self.first).chain(&self.others)
    }
}

/// Runs every candidate of each record from the oldest one in which the fast path left
/// candidates out. Returns true when each of these ticks has a match; the candidates of the
/// newest tick are then in `work.matched`.
fn replay<W: CollisionWorld, B: BodyView>(
    context: &Context<'_, W, B>,
    records: &mut VecDeque<Record>,
    steps: &mut u32,
    work: &mut Work,
) -> Result<bool, Stop> {
    let Some(oldest) = records.iter().position(|record| !record.explored) else {
        return Ok(false);
    };
    for (index, record) in records.iter_mut().enumerate().skip(oldest) {
        if index > oldest {
            record.start.clear();
            record.start.extend_from_slice(&work.matched);
        }
        candidates::run_tick(context, record, Search::All, steps, work)?;
        if work.matched.is_empty() {
            return Ok(false);
        }
    }
    Ok(true)
}
