use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_simulation::{
    BlockEvent, DrainEnd, Drained, EventQueue, GameTick, Queued, QueuedEvent, SequenceClock,
    SubTick,
};

const A: u16 = 10;
const P: BlockPos = BlockPos::new(0, 64, 0);
const Q: BlockPos = BlockPos::new(0, 80, 0);

struct Shard {
    clock: SequenceClock,
    queue: EventQueue<u16>,
}

impl Shard {
    fn new() -> Self {
        Self {
            clock: SequenceClock::new(),
            queue: EventQueue::new(),
        }
    }

    fn queue(&mut self, pos: BlockPos, a: i32, b: i32) -> Queued {
        let sequence = self.clock.stamp(GameTick(1));
        self.queue.queue(QueuedEvent {
            sequence,
            event: BlockEvent {
                pos,
                block: A,
                a,
                b,
            },
        })
    }
}

type Delivered = (BlockPos, i32, i32);

fn delivered(event: QueuedEvent<u16>) -> Delivered {
    (event.event.pos, event.event.a, event.event.b)
}

fn drain<E>(queue: &mut EventQueue<u16>, bound: u32, eligible: E) -> (Vec<Delivered>, DrainEnd)
where
    E: Fn(SectionPos) -> bool,
{
    let mut out = Vec::new();
    loop {
        match queue.next_eligible(bound, &eligible) {
            Drained::Event(event) => out.push(delivered(event)),
            Drained::Done(end) => return (out, end),
        }
    }
}

fn not_in(pos: BlockPos) -> impl Fn(SectionPos) -> bool {
    move |section| section != SectionPos::from(pos)
}

#[test]
fn an_equal_pending_event_is_dropped_and_the_first_keeps_its_place() {
    let mut shard = Shard::new();
    let results =
        [(P, 1, 0), (Q, 0, 0), (P, 1, 0), (P, 2, 0)].map(|(pos, a, b)| shard.queue(pos, a, b));
    assert_eq!(
        results,
        [
            Queued::Accepted,
            Queued::Accepted,
            Queued::Duplicate,
            Queued::Accepted
        ]
    );
    let (out, _) = drain(&mut shard.queue, u32::MAX, |_| true);
    assert_eq!(out, [(P, 1, 0), (Q, 0, 0), (P, 2, 0)]);
}

#[test]
fn events_queued_during_the_drain_are_delivered_in_the_same_drain() {
    let mut shard = Shard::new();
    shard.queue(P, 0, 0);
    let mut out = Vec::new();
    while let Drained::Event(event) = shard.queue.next_eligible(u32::MAX, |_| true) {
        out.push(event.event.pos);
        if out.len() == 1 {
            shard.queue(Q, 0, 0);
            assert_eq!(shard.queue(P, 0, 0), Queued::Accepted);
        }
    }
    assert_eq!(out, [P, Q, P]);
    assert!(shard.queue.is_empty());
}

#[test]
fn deferred_events_wait_in_order_and_precede_later_ones() {
    let mut shard = Shard::new();
    shard.queue(Q, 1, 0);
    shard.queue(P, 0, 0);
    shard.queue(Q, 2, 0);

    let (out, end) = drain(&mut shard.queue, u32::MAX, not_in(Q));
    assert_eq!((out, end.deferred), (vec![(P, 0, 0)], 2));
    assert_eq!(shard.queue(Q, 1, 0), Queued::Duplicate);
    shard.queue(P, 3, 0);

    let (out, _) = drain(&mut shard.queue, u32::MAX, |_| true);
    assert_eq!(out, [(Q, 1, 0), (Q, 2, 0), (P, 3, 0)]);
}

#[test]
fn a_bounded_drain_leaves_the_rest_in_order_behind_the_deferred() {
    let mut shard = Shard::new();
    shard.queue(Q, 0, 0);
    for a in 1..=5 {
        shard.queue(P, a, 0);
    }
    let (out, end) = drain(&mut shard.queue, 3, not_in(Q));
    assert_eq!(out, [(P, 1, 0), (P, 2, 0), (P, 3, 0)]);
    assert_eq!(
        end,
        DrainEnd {
            deferred: 1,
            left_by_bound: 2
        }
    );
    let (out, _) = drain(&mut shard.queue, 3, |_| true);
    assert_eq!(out, [(Q, 0, 0), (P, 4, 0), (P, 5, 0)]);
    assert!(shard.queue.is_empty());
}

#[test]
fn events_waiting_on_a_section_never_hold_back_the_deliverable_ones() {
    let mut shard = Shard::new();
    for a in 0..4 {
        shard.queue(Q, a, 0);
    }
    shard.queue(P, 9, 0);
    let (out, end) = drain(&mut shard.queue, 2, not_in(Q));
    assert_eq!(out, [(P, 9, 0)]);
    assert_eq!(end.deferred, 4);
    let (out, _) = drain(&mut shard.queue, 2, not_in(Q));
    assert!(out.is_empty());
    assert_eq!(shard.queue.len(), 4);
}

fn event(pos: BlockPos, a: i32, sequence: SubTick) -> QueuedEvent<u16> {
    QueuedEvent {
        sequence,
        event: BlockEvent {
            pos,
            block: A,
            a,
            b: 0,
        },
    }
}

#[test]
fn an_event_with_a_stamp_older_than_the_tail_is_delivered_first() {
    let mut shard = Shard::new();
    shard.clock.stamp(GameTick(1));
    shard.queue(P, 1, 0);
    shard.queue(Q, 2, 0);
    let older = event(Q, 0, SubTick::runtime(GameTick(1), 0));
    assert_eq!(shard.queue.queue(older), Queued::Accepted);
    let (out, _) = drain(&mut shard.queue, u32::MAX, |_| true);
    assert_eq!(out, [(Q, 0, 0), (P, 1, 0), (Q, 2, 0)]);
}

#[test]
fn absorbing_another_queue_merges_by_stamp_and_keeps_the_earlier_of_two_equal_events() {
    let mut left = Shard::new();
    let mut right = Shard::new();
    for a in [1, 2, 3, 5] {
        left.queue(if a % 2 == 0 { P } else { Q }, a, 0);
    }
    for a in [5, 4, 2] {
        right.queue(if a % 2 == 0 { P } else { Q }, a, 0);
    }
    left.queue.absorb(right.queue);
    assert_eq!(left.queue.len(), 5);
    let (out, _) = drain(&mut left.queue, u32::MAX, |_| true);
    assert_eq!(out, [(Q, 1, 0), (Q, 5, 0), (P, 2, 0), (P, 4, 0), (Q, 3, 0)]);
}

#[test]
fn splitting_by_section_keeps_both_parts_in_stamp_order_with_their_duplicates() {
    let mut shard = Shard::new();
    for (pos, a) in [(P, 0), (Q, 1), (P, 2), (Q, 3), (P, 4)] {
        shard.queue(pos, a, 0);
    }
    let (_, end) = drain(&mut shard.queue, u32::MAX, |_| false);
    assert_eq!(end.deferred, 5);

    let moving = SectionPos::from(Q);
    let mut moved = shard.queue.split_off(|section| section == moving);
    assert_eq!(
        moved.queue(event(Q, 3, shard.clock.stamp(GameTick(1)))),
        Queued::Duplicate
    );
    assert_eq!(
        moved.queue(event(P, 0, shard.clock.stamp(GameTick(1)))),
        Queued::Accepted
    );
    assert_eq!(shard.queue(Q, 1, 0), Queued::Accepted);
    assert_eq!(shard.queue(P, 2, 0), Queued::Duplicate);

    let (out, _) = drain(&mut moved, u32::MAX, |_| true);
    assert_eq!(out, [(Q, 1, 0), (Q, 3, 0), (P, 0, 0)]);
    let (out, _) = drain(&mut shard.queue, u32::MAX, |_| true);
    assert_eq!(out, [(P, 0, 0), (P, 2, 0), (P, 4, 0), (Q, 1, 0)]);
}

#[test]
fn taking_a_section_parks_its_events_in_order_and_releases_their_duplicates() {
    let mut shard = Shard::new();
    for (pos, a) in [(Q, 2), (P, 0), (Q, 1)] {
        shard.queue(pos, a, 0);
    }
    let mut parked = Vec::new();
    shard.queue.take_section(SectionPos::from(Q), &mut parked);
    assert_eq!(
        parked.into_iter().map(delivered).collect::<Vec<_>>(),
        [(Q, 2, 0), (Q, 1, 0)]
    );
    assert_eq!(shard.queue(Q, 2, 0), Queued::Accepted);
    let (out, _) = drain(&mut shard.queue, u32::MAX, |_| true);
    assert_eq!(out, [(P, 0, 0), (Q, 2, 0)]);
}

fn stamp(counter: u32) -> SubTick {
    SubTick::runtime(GameTick(1), counter)
}

fn delivered_now<E>(queue: &mut EventQueue<u16>, bound: u32, eligible: E) -> Vec<Delivered>
where
    E: Fn(SectionPos) -> bool,
{
    let mut out = Vec::new();
    while let Drained::Event(event) = queue.next_eligible(bound, &eligible) {
        out.push(delivered(event));
    }
    out
}

fn drained_once_past_q(
    events: [(BlockPos, i32); 4],
    bound: u32,
) -> (EventQueue<u16>, QueuedEvent<u16>) {
    let mut queue = EventQueue::new();
    for (pos, a) in events {
        queue.queue(event(pos, a, stamp(a as u32)));
    }
    let Drained::Event(first) = queue.next_eligible(bound, not_in(Q)) else {
        panic!("P's first event is deliverable");
    };
    (queue, first)
}

#[test]
fn a_queue_change_in_the_middle_of_a_drain_keeps_its_bound_and_deferrals() {
    let (mut queue, first) = drained_once_past_q([(Q, 0), (P, 1), (P, 2), (P, 3)], 3);
    let mut other = EventQueue::new();
    other.queue(event(P, 9, stamp(9)));
    queue.absorb(other);
    let rest = queue.split_off(|_| false);
    queue.absorb(rest);
    let mut parked = Vec::new();
    queue.take_section(SectionPos::new(9, 9, 9), &mut parked);

    let mut out = vec![delivered(first)];
    let end = loop {
        match queue.next_eligible(3, not_in(Q)) {
            Drained::Event(event) => out.push(delivered(event)),
            Drained::Done(end) => break end,
        }
    };
    assert_eq!(out, [(P, 1, 0), (P, 2, 0), (P, 3, 0)]);
    assert_eq!(
        end,
        DrainEnd {
            deferred: 1,
            left_by_bound: 1
        }
    );
    let (out, _) = drain(&mut queue, u32::MAX, |_| true);
    assert_eq!(out, [(Q, 0, 0), (P, 9, 0)]);
}

#[test]
fn absorbing_during_a_drain_delivers_every_event_once_in_stamp_order() {
    let mut target = EventQueue::new();
    for (pos, a) in [(Q, 0), (P, 2), (P, 4)] {
        target.queue(event(pos, a, stamp(a as u32)));
    }
    let mut other = EventQueue::new();
    for (pos, a) in [(Q, 0), (P, 3), (Q, 5)] {
        other.queue(event(pos, a, stamp(a as u32)));
    }
    let mut first = Vec::new();
    for queue in [&mut target, &mut other] {
        let Drained::Event(event) = queue.next_eligible(u32::MAX, not_in(Q)) else {
            panic!("the first P event is deliverable");
        };
        first.push(delivered(event));
    }
    assert_eq!(first, [(P, 2, 0), (P, 3, 0)]);

    target.absorb(other);
    assert_eq!(target.len(), 3);
    assert_eq!(delivered_now(&mut target, u32::MAX, not_in(Q)), [(P, 4, 0)]);
    assert_eq!(
        delivered_now(&mut target, u32::MAX, |_| true),
        [(Q, 0, 0), (Q, 5, 0)]
    );
    assert!(target.is_empty());
}

#[test]
fn taking_a_section_during_a_drain_parks_its_deferred_events_too() {
    let (mut queue, first) = drained_once_past_q([(Q, 0), (P, 1), (Q, 2), (P, 3)], u32::MAX);
    let mut parked = Vec::new();
    queue.take_section(SectionPos::from(Q), &mut parked);
    assert_eq!(
        parked.into_iter().map(delivered).collect::<Vec<_>>(),
        [(Q, 0, 0), (Q, 2, 0)]
    );
    let mut out = vec![delivered(first)];
    out.extend(delivered_now(&mut queue, u32::MAX, not_in(Q)));
    assert_eq!(out, [(P, 1, 0), (P, 3, 0)]);
    assert!(queue.is_empty());
    assert_eq!(queue.queue(event(Q, 0, stamp(9))), Queued::Accepted);
}

#[test]
fn an_older_event_deferred_after_a_newer_one_still_wins_its_duplicate_on_absorb() {
    let x = BlockPos::new(1, 80, 0);
    let r = BlockPos::new(0, 0, 0);
    let mut queue = EventQueue::new();
    queue.queue(event(Q, 0, stamp(5)));
    queue.queue(event(P, 0, stamp(6)));
    let Drained::Event(p) = queue.next_eligible(u32::MAX, not_in(Q)) else {
        panic!("P's event is deliverable");
    };
    queue.queue(event(x, 0, stamp(1)));
    queue.queue(event(P, 1, stamp(7)));
    let Drained::Event(p_again) = queue.next_eligible(u32::MAX, not_in(Q)) else {
        panic!("P's second event is deliverable");
    };
    assert_eq!([delivered(p), delivered(p_again)], [(P, 0, 0), (P, 1, 0)]);

    let mut other = EventQueue::new();
    other.queue(event(x, 0, stamp(3)));
    other.queue(event(r, 0, stamp(2)));
    queue.absorb(other);
    assert_eq!(queue.len(), 3);

    let mut drains = Vec::new();
    for _ in 0..2 {
        let mut out = Vec::new();
        while let Drained::Event(event) = queue.next_eligible(u32::MAX, |_| true) {
            out.push((event.event.pos, event.sequence));
        }
        drains.push(out);
    }
    assert_eq!(
        drains,
        [vec![(r, stamp(2))], vec![(x, stamp(1)), (Q, stamp(5))]]
    );
}
