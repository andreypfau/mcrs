use crate::access::{PartitionKind, SectionProperties};
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::event_queue::{BlockEvent, QueuedEvent};
use crate::message::{Message, MessageBody};
use crate::output::{BlockWrite, Destroyed, StepOutput};
use crate::scalar::{GameTick, TickPriority};
use crate::sequence::{SubTick, TickKey};
use crate::shard::{BlockShard, Env, KernelConfig};
use crate::tick_queue::{Collected, Kinds, ScheduledTick};
use crate::{BlockUpdateFlags, SimulationVolume};
use mcrs_minecraft_core::{BlockPos, BlockStateId, SectionPos};
use mcrs_minecraft_volume::{Access, SectionBlocks, Unavailable};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane {
    Own,
    Away,
    Dropped,
}

fn tick_lane(kind: PartitionKind, access: Access, properties: SectionProperties) -> Lane {
    match (access, kind) {
        (Access::Write, PartitionKind::Shard)
            if properties.contains(SectionProperties::ACCEPTS_TICKS) =>
        {
            Lane::Own
        }
        (Access::Read | Access::Foreign, _) | (Access::Write, PartitionKind::Cell) => Lane::Away,
        _ => Lane::Dropped,
    }
}

fn event_lane(kind: PartitionKind, access: Access) -> Lane {
    match (access, kind) {
        (Access::Write | Access::Unloaded, PartitionKind::Shard) => Lane::Own,
        (Access::Read | Access::Foreign, _) | (Access::Write, PartitionKind::Cell) => Lane::Away,
        _ => Lane::Dropped,
    }
}

pub(crate) fn bump(counter: &mut u32) {
    *counter = counter.saturating_add(1);
}

fn due_after(now: GameTick, delay: i32) -> GameTick {
    GameTick(now.0.saturating_add(i64::from(delay)))
}

pub struct Cx<'a, K: Kinds, V, F, B> {
    pub(crate) shard: &'a mut BlockShard<K>,
    pub(crate) volume: &'a mut V,
    pub(crate) out: &'a mut StepOutput<K>,
    pub(crate) facts: &'a F,
    pub(crate) behaviour: &'a B,
    pub(crate) config: &'a KernelConfig,
    pub(crate) kind: PartitionKind,
    pub(crate) now: GameTick,
}

impl<'a, K, V, F, B> Cx<'a, K, V, F, B>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    pub(crate) fn new<P>(
        shard: &'a mut BlockShard<K>,
        volume: &'a mut V,
        out: &'a mut StepOutput<K>,
        env: &Env<'a, F, B, P>,
        now: GameTick,
    ) -> Self {
        Self {
            shard,
            volume,
            out,
            facts: env.facts,
            behaviour: env.behaviour,
            config: env.config,
            kind: env.kind,
            now,
        }
    }

    pub fn now(&self) -> GameTick {
        self.now
    }

    pub fn state(&mut self, pos: BlockPos) -> Result<BlockStateId, Unavailable> {
        match self.volume.block(pos) {
            Err(Unavailable::Outside) => Ok(self.facts.void_air()),
            Err(Unavailable::Foreign) => {
                bump(&mut self.out.report.foreign_reads);
                Err(Unavailable::Foreign)
            }
            other => other,
        }
    }

    fn lane_for_tick(&self, pos: BlockPos) -> Lane {
        let section = SectionPos::from(pos);
        tick_lane(
            self.kind,
            self.volume.access(section),
            self.volume.properties(section),
        )
    }

    fn lane_for_event(&self, pos: BlockPos) -> Lane {
        event_lane(self.kind, self.volume.access(SectionPos::from(pos)))
    }

    fn send(&mut self, stamp: SubTick, body: MessageBody<K>) {
        self.out.outbound.push(Message {
            stamp,
            origin: stamp,
            body,
        });
    }

    pub fn schedule_block_tick(
        &mut self,
        pos: BlockPos,
        block: K::Block,
        delay: i32,
        priority: TickPriority,
    ) {
        let stamp = self.shard.clock.stamp(self.now);
        let due = due_after(self.now, delay);
        match self.lane_for_tick(pos) {
            Lane::Own => self.push_block_tick(
                pos,
                block,
                TickKey {
                    due,
                    priority,
                    sequence: stamp,
                },
            ),
            Lane::Away => self.send(
                stamp,
                MessageBody::BlockTick {
                    pos,
                    block,
                    due,
                    priority,
                },
            ),
            Lane::Dropped => bump(&mut self.out.report.unaccepted_ticks),
        }
    }

    pub fn schedule_fluid_tick(
        &mut self,
        pos: BlockPos,
        fluid: K::Fluid,
        delay: i32,
        priority: TickPriority,
    ) {
        let stamp = self.shard.clock.stamp(self.now);
        let due = due_after(self.now, delay);
        match self.lane_for_tick(pos) {
            Lane::Own => self.push_fluid_tick(
                pos,
                fluid,
                TickKey {
                    due,
                    priority,
                    sequence: stamp,
                },
            ),
            Lane::Away => self.send(
                stamp,
                MessageBody::FluidTick {
                    pos,
                    fluid,
                    due,
                    priority,
                },
            ),
            Lane::Dropped => bump(&mut self.out.report.unaccepted_ticks),
        }
    }

    fn push_block_tick(&mut self, pos: BlockPos, kind: K::Block, key: TickKey) {
        self.shard
            .block_ticks
            .schedule(ScheduledTick { pos, kind, key });
    }

    fn push_fluid_tick(&mut self, pos: BlockPos, kind: K::Fluid, key: TickKey) {
        self.shard
            .fluid_ticks
            .schedule(ScheduledTick { pos, kind, key });
    }

    pub fn queue_block_event(&mut self, pos: BlockPos, block: K::Block, a: i32, b: i32) {
        let stamp = self.shard.clock.stamp(self.now);
        let event = BlockEvent { pos, block, a, b };
        match self.lane_for_event(pos) {
            Lane::Own => self.push_event(stamp, event),
            Lane::Away => self.send(stamp, MessageBody::Event(event)),
            Lane::Dropped => bump(&mut self.out.report.unloaded_drops),
        }
    }

    fn push_event(&mut self, sequence: SubTick, event: BlockEvent<K::Block>) {
        self.shard.events.queue(QueuedEvent { sequence, event });
    }

    pub fn has_scheduled_block_tick(&self, pos: BlockPos, block: K::Block) -> bool {
        self.lane_for_tick(pos) == Lane::Own && self.shard.block_ticks.contains(pos, block)
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, block: K::Block) -> bool {
        self.shard.ticking.contains(&(pos, block))
    }

    pub fn set(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    ) -> bool {
        match self.volume.access(SectionPos::from(pos)) {
            Access::Write => self.write_block(pos, state, flags, limit),
            Access::Read | Access::Foreign => {
                let stamp = self.shard.clock.stamp(self.now);
                self.send(
                    stamp,
                    MessageBody::Set {
                        pos,
                        state,
                        flags,
                        limit,
                    },
                );
                false
            }
            Access::Unloaded | Access::Outside => {
                bump(&mut self.out.report.refused_sets);
                false
            }
        }
    }

    fn section_is_air(&mut self, pos: BlockPos) -> bool {
        let facts = self.facts;
        let Ok(section) = self.volume.section(SectionPos::from(pos)) else {
            return false;
        };
        section
            .palette(&mut self.shard.palette)
            .iter()
            .all(|(state, _)| facts.is_air(state))
    }

    fn holds_block_of(&self, pos: BlockPos, state: BlockStateId) -> bool {
        self.volume
            .block(pos)
            .is_ok_and(|now| self.facts.block_of(now) == self.facts.block_of(state))
    }

    fn write_block(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    ) -> bool {
        if self.facts.is_air(state) && self.section_is_air(pos) {
            return false;
        }
        let Ok(old) = self.volume.set_block(pos, state) else {
            return false;
        };
        if old == state {
            return false;
        }
        self.out.writes.push(BlockWrite {
            pos,
            old,
            new: state,
            flags,
        });
        let behaviour = self.behaviour;
        let moved_by_piston = flags.contains(BlockUpdateFlags::MOVE_BY_PISTON);
        let type_changed = self.facts.block_of(old) != self.facts.block_of(state);
        let removal =
            flags.intersects(BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::MOVE_BY_PISTON);
        if (type_changed || self.facts.is_rail(state)) && removal {
            behaviour.removed(self, pos, old, moved_by_piston);
        }
        if !self.holds_block_of(pos, state) {
            return false;
        }
        if !flags.contains(BlockUpdateFlags::SKIP_ON_PLACE) {
            behaviour.placed(self, pos, state, old, moved_by_piston);
        }
        if self.volume.block(pos).is_ok_and(|now| now == state) {
            self.mark_for_clients(pos, flags);
            if flags.contains(BlockUpdateFlags::NEIGHBORS) {
                self.update_neighbours(pos, self.facts.block_of(old), None);
            }
            if !flags.contains(BlockUpdateFlags::KNOWN_SHAPE) {
                self.emit_shape_updates(pos, state, flags, limit);
            }
        }
        true
    }

    fn mark_for_clients(&mut self, pos: BlockPos, flags: BlockUpdateFlags) {
        let broadcasts = self
            .volume
            .properties(SectionPos::from(pos))
            .contains(SectionProperties::BROADCASTS_CHANGES);
        if flags.contains(BlockUpdateFlags::CLIENTS) && broadcasts {
            self.out.marked.push(pos);
        }
    }

    pub(crate) fn destroy(&mut self, pos: BlockPos, drops: bool, limit: u16) -> bool {
        let Ok(old) = self.state(pos) else {
            return false;
        };
        if self.facts.is_air(old) {
            return false;
        }
        self.out.destroyed.push(Destroyed {
            pos,
            state: old,
            drops,
        });
        let replacement = self.facts.legacy_block_of_fluid(old);
        self.set(
            pos,
            replacement,
            BlockUpdateFlags::from_bits_retain(3),
            limit,
        )
    }

    pub(crate) fn apply(&mut self, message: Message<K>) {
        self.shard.clock.observe(message.stamp);
        let sequence = message.stamp;
        match message.body {
            MessageBody::Set {
                pos,
                state,
                flags,
                limit,
            } => {
                self.set(pos, state, flags, limit);
            }
            MessageBody::BlockTick {
                pos,
                block,
                due,
                priority,
            } => {
                if self.lane_for_tick(pos) == Lane::Own {
                    self.push_block_tick(
                        pos,
                        block,
                        TickKey {
                            due,
                            priority,
                            sequence,
                        },
                    );
                } else {
                    bump(&mut self.out.report.unaccepted_ticks);
                }
            }
            MessageBody::FluidTick {
                pos,
                fluid,
                due,
                priority,
            } => {
                if self.lane_for_tick(pos) == Lane::Own {
                    self.push_fluid_tick(
                        pos,
                        fluid,
                        TickKey {
                            due,
                            priority,
                            sequence,
                        },
                    );
                } else {
                    bump(&mut self.out.report.unaccepted_ticks);
                }
            }
            MessageBody::Event(event) => {
                if self.lane_for_event(event.pos) == Lane::Own {
                    self.push_event(sequence, event);
                } else {
                    bump(&mut self.out.report.unloaded_drops);
                }
            }
            MessageBody::Update(_) => {}
        }
    }

    pub(crate) fn run_block_tick(&mut self, tick: Collected<K::Block>) {
        self.shard.ticking.remove(&(tick.pos, tick.kind));
        let behaviour = self.behaviour;
        if let Ok(state) = self.state(tick.pos)
            && self.facts.block_of(state) == tick.kind
        {
            behaviour.block_tick(self, tick.pos, state);
        }
    }

    pub(crate) fn run_fluid_tick(&mut self, tick: Collected<K::Fluid>) {
        let behaviour = self.behaviour;
        if let Ok(state) = self.state(tick.pos)
            && self.facts.fluid_of(state) == Some(tick.kind)
        {
            behaviour.fluid_tick(self, tick.pos, state, tick.kind);
        }
    }

    pub(crate) fn deliver(&mut self, event: BlockEvent<K::Block>) {
        let behaviour = self.behaviour;
        if let Ok(state) = self.state(event.pos)
            && self.facts.block_of(state) == event.block
            && behaviour.block_event(self, event.pos, state, event.a, event.b)
        {
            self.out.delivered.push(event);
        }
    }
}
