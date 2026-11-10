use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::{Cx, bump};
use crate::message::{ChainOrigin, Message, MessageBody, UpdateRequest};
use crate::output::ChainLedger;
use crate::shard::ChainLimit;
use crate::tick_queue::Kinds;
use crate::{BlockUpdateFlags, SimulationVolume};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_volume::Access;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

use Direction::{Down, East, North, South, Up, West};

pub(crate) fn neighbour(pos: BlockPos, direction: Direction) -> Option<BlockPos> {
    let normal = direction.normal();
    Some(BlockPos::new(
        pos.x.checked_add(normal.x)?,
        pos.y.checked_add(normal.y)?,
        pos.z.checked_add(normal.z)?,
    ))
}

pub(crate) struct Frame<K: Kinds> {
    request: UpdateRequest<K>,
    next: u8,
}

pub(crate) struct Chain<K: Kinds> {
    stack: Vec<Frame<K>>,
    added: Vec<UpdateRequest<K>>,
    count: u32,
    running: bool,
    origin: Option<ChainOrigin>,
    sent: bool,
}

impl<K: Kinds> Default for Chain<K> {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            added: Vec::new(),
            count: 0,
            running: false,
            origin: None,
            sent: false,
        }
    }
}

impl<K: Kinds> Chain<K> {
    pub(crate) fn reset(&mut self) {
        self.stack.clear();
        self.added.clear();
        self.count = 0;
        self.running = false;
        self.origin = None;
        self.sent = false;
    }

    fn start_pieces(&mut self) {
        let stack = &mut self.stack;
        stack.extend(
            self.added
                .drain(..)
                .rev()
                .map(|request| Frame { request, next: 0 }),
        );
    }
}

fn next_direction(
    order: &[Direction; 6],
    skip: Option<Direction>,
    from: u8,
) -> Option<(Direction, u8)> {
    let mut index = from;
    loop {
        let direction = *order.get(usize::from(index))?;
        index = index.saturating_add(1);
        if Some(direction) != skip {
            return Some((direction, index));
        }
    }
}

fn has_direction_after(order: &[Direction; 6], skip: Option<Direction>, next: u8) -> bool {
    next_direction(order, skip, next).is_some()
}

impl<K, V, F, B> Cx<'_, K, V, F, B>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    pub fn update_neighbours(&mut self, pos: BlockPos, source: K::Block, skip: Option<Direction>) {
        self.offer(UpdateRequest::Around {
            source: pos,
            block: source,
            skip,
        });
    }

    pub fn update_neighbour(&mut self, target: BlockPos, source: K::Block) {
        self.offer(UpdateRequest::Single {
            target,
            block: source,
        });
    }

    pub fn update_neighbour_with_state(
        &mut self,
        target: BlockPos,
        state: BlockStateId,
        source: K::Block,
        moved_by_piston: bool,
    ) {
        self.offer(UpdateRequest::WithState {
            target,
            state,
            block: source,
            moved_by_piston,
        });
    }

    pub(crate) fn emit_shape_updates(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    ) {
        let Some(limit) = limit.checked_sub(1) else {
            return;
        };
        let flags =
            flags.difference(BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::SUPPRESS_DROPS);
        for direction in self.chain_order.shapes {
            if let Some(target) = neighbour(pos, direction) {
                self.offer(UpdateRequest::Shape {
                    target,
                    from: direction.opposite(),
                    neighbour_state: state,
                    flags,
                    limit,
                });
            }
        }
    }

    pub(crate) fn emit_indirect_shape_updates(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    ) {
        let Some(limit) = limit.checked_sub(1) else {
            return;
        };
        let flags =
            flags.difference(BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::SUPPRESS_DROPS);
        let facts = self.facts;
        if !facts.is_redstone_wire(state) {
            return;
        }
        for direction in [North, East, South, West] {
            if !facts.wire_connects(state, direction) {
                continue;
            }
            let Some(adjacent) = neighbour(pos, direction) else {
                continue;
            };
            if self
                .state(adjacent)
                .is_ok_and(|state| facts.is_redstone_wire(state))
            {
                continue;
            }
            for vertical in [Down, Up] {
                let Some(target) = neighbour(adjacent, vertical) else {
                    continue;
                };
                if !self
                    .state(target)
                    .is_ok_and(|state| facts.is_redstone_wire(state))
                {
                    continue;
                }
                let Some(neighbour_pos) = neighbour(pos, vertical) else {
                    continue;
                };
                let Ok(neighbour_state) = self.state(neighbour_pos) else {
                    continue;
                };
                self.offer(UpdateRequest::Shape {
                    target,
                    from: direction.opposite(),
                    neighbour_state,
                    flags,
                    limit,
                });
            }
        }
    }

    pub(crate) fn update_comparators(&mut self, pos: BlockPos, source: K::Block) {
        let facts = self.facts;
        for direction in [North, East, South, West] {
            let Some(target) = neighbour(pos, direction) else {
                continue;
            };
            if !matches!(
                self.volume.access(SectionPos::from(target)),
                Access::Read | Access::Write
            ) {
                continue;
            }
            let Ok(state) = self.state(target) else {
                continue;
            };
            if facts.is_comparator(state) {
                self.update_neighbour_with_state(target, state, source, false);
                continue;
            }
            if !facts.is_redstone_conductor(state) {
                continue;
            }
            let Some(far) = neighbour(target, direction) else {
                continue;
            };
            let Ok(far_state) = self.state(far) else {
                continue;
            };
            if facts.is_comparator(far_state) {
                self.update_neighbour_with_state(far, far_state, source, false);
            }
        }
    }

    pub(crate) fn offer(&mut self, request: UpdateRequest<K>) {
        let outermost = !self.shard.chain.running;
        self.shard.chain.running = true;
        let accepted = self.count_offer(request.pos());
        if accepted {
            self.admit(request);
        }
        if outermost {
            self.drain_chain();
            self.end_chain();
        }
    }

    pub(crate) fn seed_chain(
        &mut self,
        origin: ChainOrigin,
        offered: u32,
        run: &[Message<K>],
    ) -> u32 {
        let chain = &mut self.shard.chain;
        chain.running = true;
        chain.origin = Some(origin);
        chain.count = offered;
        for message in run {
            self.shard.clock.observe(message.stamp);
            if let MessageBody::Update(request) = message.body {
                self.admit(request);
            }
        }
        self.drain_chain();
        let ended = self.shard.chain.count;
        self.end_chain();
        ended
    }

    fn count_offer(&mut self, pos: BlockPos) -> bool {
        let chain = &mut self.shard.chain;
        let (over, first_over) = match self.config.chain_limit {
            ChainLimit::Limited(limit) => (chain.count >= limit, chain.count == limit),
            ChainLimit::Unlimited => (false, false),
        };
        chain.count = chain.count.saturating_add(1);
        // Keyed on the count, not a per-piece flag: a continuation seeded past the limit
        // belongs to a chain whose sender already reported the overflow.
        if first_over {
            bump(&mut self.out.report.chain_overflows);
            self.out.report.first_chain_overflow.get_or_insert(pos);
        }
        !over
    }

    fn admit(&mut self, request: UpdateRequest<K>) {
        let access = self.volume.access(SectionPos::from(request.pos()));
        match (access, request) {
            // An around request's position is its source, not a block it updates; with no
            // owner to send it to, run it here and let each neighbour route by its own access.
            (Access::Write, _)
            | (Access::Unloaded | Access::Outside, UpdateRequest::Around { .. }) => {
                self.shard.chain.added.push(request);
            }
            (Access::Read | Access::Foreign, _) => self.send_update(request),
            (Access::Unloaded, _) => bump(&mut self.out.report.unloaded_drops),
            (Access::Outside, _) => {}
        }
    }

    fn send_update(&mut self, request: UpdateRequest<K>) {
        if self.task_leaves() {
            return;
        }
        let stamp = self.shard.clock.stamp(self.now);
        let own = self.origin_at(stamp);
        let chain = &mut self.shard.chain;
        let origin = *chain.origin.get_or_insert(own);
        chain.sent = true;
        self.out.outbound.push(Message {
            stamp,
            origin,
            body: MessageBody::Update(request),
        });
    }

    fn end_chain(&mut self) {
        let chain = &mut self.shard.chain;
        if chain.sent
            && let Some(origin) = chain.origin
        {
            let offered = chain.count;
            match self
                .out
                .chains
                .iter_mut()
                .find(|ledger| ledger.origin == origin)
            {
                Some(ledger) => ledger.offered = ledger.offered.max(offered),
                None => self.out.chains.push(ChainLedger { origin, offered }),
            }
        }
        chain.count = 0;
        chain.running = false;
        chain.origin = None;
        chain.sent = false;
    }

    fn drain_chain(&mut self) {
        loop {
            self.shard.chain.start_pieces();
            let Some(mut frame) = self.shard.chain.stack.pop() else {
                return;
            };
            if self.run_step(&mut frame) {
                self.shard.chain.stack.push(frame);
            }
        }
    }

    fn run_step(&mut self, frame: &mut Frame<K>) -> bool {
        match frame.request {
            UpdateRequest::Around {
                source,
                block,
                skip,
            } => {
                let order = self.chain_order.neighbours;
                let Some((direction, next)) = next_direction(&order, skip, frame.next) else {
                    return false;
                };
                frame.next = next;
                self.run_around(source, block, direction);
                has_direction_after(&order, skip, next)
            }
            UpdateRequest::Single { target, block } => {
                self.run_single(target, block);
                false
            }
            UpdateRequest::WithState {
                target,
                state,
                block,
                moved_by_piston,
            } => {
                let behaviour = self.behaviour;
                if self.fits(target, state) {
                    behaviour.neighbour_changed(self, target, state, block, moved_by_piston);
                }
                false
            }
            UpdateRequest::Shape {
                target,
                from,
                neighbour_state,
                flags,
                limit,
            } => {
                self.run_shape(target, from, neighbour_state, flags, limit);
                false
            }
        }
    }

    fn run_around(&mut self, source: BlockPos, block: K::Block, direction: Direction) {
        let Some(target) = neighbour(source, direction) else {
            return;
        };
        match self.volume.access(SectionPos::from(target)) {
            Access::Write => self.run_single(target, block),
            Access::Read | Access::Foreign => {
                self.send_update(UpdateRequest::Single { target, block });
            }
            Access::Unloaded => bump(&mut self.out.report.unloaded_drops),
            Access::Outside => {}
        }
    }

    fn run_single(&mut self, target: BlockPos, block: K::Block) {
        let behaviour = self.behaviour;
        if let Ok(state) = self.state(target)
            && self.fits(target, state)
        {
            behaviour.neighbour_changed(self, target, state, block, false);
        }
    }

    fn run_shape(
        &mut self,
        target: BlockPos,
        from: Direction,
        neighbour_state: BlockStateId,
        flags: BlockUpdateFlags,
        limit: u16,
    ) {
        let Ok(current) = self.state(target) else {
            return;
        };
        if flags.contains(BlockUpdateFlags::SKIP_SHAPE_UPDATE_ON_WIRE)
            && self.facts.is_redstone_wire(current)
        {
            return;
        }
        let Some(changed) = neighbour(target, from) else {
            return;
        };
        if !self.fits(target, current) {
            return;
        }
        let behaviour = self.behaviour;
        let next = behaviour.update_shape(self, target, current, from, changed, neighbour_state);
        if next == current {
            return;
        }
        if self.facts.is_air(next) {
            self.destroy(
                target,
                !flags.contains(BlockUpdateFlags::SUPPRESS_DROPS),
                limit,
            );
        } else {
            self.set(
                target,
                next,
                flags.difference(BlockUpdateFlags::SUPPRESS_DROPS),
                limit,
            );
        }
    }
}
