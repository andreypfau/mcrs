use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_simulation::{
    CollectScratch, Collected, GameTick, OrderingPolicy, Queued, SavedTick, ScheduledTick,
    SemanticOrder, SubTick, TickKey, TickPriority, TickQueue, VanillaOrder,
};

pub(crate) const BUDGET: u32 = 65_536;
pub(crate) const A: u16 = 10;
pub(crate) const B: u16 = 11;

fn tick(pos: BlockPos, due: i64, priority: TickPriority, sequence: SubTick) -> ScheduledTick<u16> {
    ScheduledTick {
        pos,
        kind: A,
        key: TickKey {
            due: GameTick(due),
            priority,
            sequence,
        },
    }
}

fn normal(pos: BlockPos, due: i64, sequence: u32) -> ScheduledTick<u16> {
    tick(pos, due, TickPriority::Normal, rt(sequence))
}

pub(crate) fn saved(pos: BlockPos, kind: u16, delay: i32, sequence: SubTick) -> SavedTick<u16> {
    SavedTick {
        pos,
        kind,
        delay,
        priority: TickPriority::Normal,
        sequence,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Policy {
    Vanilla,
    Semantic,
}

pub(crate) const POLICIES: [Policy; 2] = [Policy::Vanilla, Policy::Semantic];

impl Policy {
    fn collect_into<E>(
        self,
        queue: &mut TickQueue<u16>,
        now: i64,
        budget: u32,
        eligible: E,
        scratch: &mut CollectScratch,
        out: &mut Vec<Collected<u16>>,
    ) where
        E: FnMut(SectionPos) -> bool,
    {
        let now = GameTick(now);
        match self {
            Self::Vanilla => VanillaOrder.collect_due(queue, now, budget, eligible, scratch, out),
            Self::Semantic => SemanticOrder.collect_due(queue, now, budget, eligible, scratch, out),
        }
    }

    pub(crate) fn collect_within<E>(
        self,
        queue: &mut TickQueue<u16>,
        now: i64,
        budget: u32,
        eligible: E,
    ) -> Vec<Collected<u16>>
    where
        E: FnMut(SectionPos) -> bool,
    {
        let mut out = Vec::new();
        self.collect_into(
            queue,
            now,
            budget,
            eligible,
            &mut CollectScratch::default(),
            &mut out,
        );
        out
    }

    pub(crate) fn collect(self, queue: &mut TickQueue<u16>, now: i64) -> Vec<Collected<u16>> {
        self.collect_within(queue, now, BUDGET, |_| true)
    }

    fn sequences(self, queue: &mut TickQueue<u16>, now: i64, budget: u32) -> Vec<i64> {
        sequences(&self.collect_within(queue, now, budget, |_| true))
    }

    fn positions(self, queue: &mut TickQueue<u16>, now: i64) -> Vec<BlockPos> {
        self.collect(queue, now).iter().map(|t| t.pos).collect()
    }
}

fn sequences(ticks: &[Collected<u16>]) -> Vec<i64> {
    ticks.iter().map(|tick| tick.sequence.get()).collect()
}

#[test]
fn priority_decoding_clamps_to_the_seven_values() {
    for (saved, expected) in [
        (-7, TickPriority::ExtremelyHigh),
        (9, TickPriority::ExtremelyLow),
        (2, TickPriority::VeryLow),
    ] {
        assert_eq!(TickPriority::from_saved(saved), expected, "saved {saved}");
    }
}

#[test]
fn a_pending_position_and_type_rejects_a_second_tick_until_collected() {
    let p = BlockPos::new(3, 64, 3);
    let mut queue = TickQueue::new();
    assert_eq!(queue.schedule(normal(p, 110, 0)), Queued::Accepted);
    assert_eq!(
        queue.schedule(tick(p, 102, TickPriority::ExtremelyHigh, rt(1))),
        Queued::Duplicate
    );
    let other_type = ScheduledTick {
        kind: B,
        ..normal(p, 105, 2)
    };
    assert_eq!(queue.schedule(other_type), Queued::Accepted);
    assert!(queue.contains(p, A) && queue.contains(p, B));
    assert_eq!(queue.len(), 2);

    assert_eq!(
        Policy::Vanilla.sequences(&mut queue, 104, BUDGET),
        [0_i64; 0]
    );
    assert_eq!(Policy::Vanilla.sequences(&mut queue, 110, BUDGET), [2, 0]);
    assert!(queue.is_empty() && !queue.contains(p, A));
    assert_eq!(queue.schedule(normal(p, 111, 3)), Queued::Accepted);
    assert_eq!(Policy::Vanilla.sequences(&mut queue, 111, BUDGET), [3]);
}

const LOW: BlockPos = BlockPos::new(-32, 64, 0);
const HIGH: BlockPos = BlockPos::new(48, 64, 0);
const HIGH_ABOVE: BlockPos = BlockPos::new(48, 96, 0);

struct OrderCase {
    name: &'static str,
    now: i64,
    ticks: &'static [(BlockPos, i64, TickPriority, SubTick)],
    vanilla: &'static [usize],
    semantic: &'static [usize],
}

const N: TickPriority = TickPriority::Normal;

const fn rt(counter: u32) -> SubTick {
    SubTick::runtime(GameTick(0), counter)
}

pub(crate) const fn rs(index: u32, count: u32) -> SubTick {
    match SubTick::restored(index, count) {
        Some(sequence) => sequence,
        None => panic!("a restored index must be below the count"),
    }
}

const ORDER_CASES: &[OrderCase] = &[
    OrderCase {
        name: "across columns the due tick is ignored",
        now: 11,
        ticks: &[
            (BlockPos::new(0, 0, 0), 11, N, rt(1)),
            (BlockPos::new(0, 80, 0), 10, N, rt(5)),
            (BlockPos::new(16, 0, 0), 11, N, rt(3)),
        ],
        vanilla: &[2, 1, 0],
        semantic: &[1, 0, 2],
    },
    OrderCase {
        name: "a column yields as soon as another head sorts first",
        now: 11,
        ticks: &[
            (BlockPos::new(0, 0, 0), 11, N, rt(1)),
            (BlockPos::new(0, 80, 0), 11, N, rt(4)),
            (BlockPos::new(16, 0, 0), 11, N, rt(2)),
        ],
        vanilla: &[0, 2, 1],
        semantic: &[0, 2, 1],
    },
    OrderCase {
        name: "an overdue tick precedes a higher priority one in its section",
        now: 11,
        ticks: &[
            (BlockPos::new(1, 0, 1), 10, N, rt(9)),
            (
                BlockPos::new(2, 0, 2),
                11,
                TickPriority::ExtremelyHigh,
                rt(10),
            ),
        ],
        vanilla: &[0, 1],
        semantic: &[0, 1],
    },
    OrderCase {
        name: "an overdue tick precedes a higher priority one in its column",
        now: 11,
        ticks: &[
            (BlockPos::new(1, 0, 1), 10, N, rt(9)),
            (
                BlockPos::new(2, 40, 2),
                11,
                TickPriority::ExtremelyHigh,
                rt(10),
            ),
        ],
        vanilla: &[0, 1],
        semantic: &[0, 1],
    },
    OrderCase {
        name: "across columns priority wins over an overdue tick",
        now: 11,
        ticks: &[
            (BlockPos::new(1, 0, 1), 10, N, rt(9)),
            (
                BlockPos::new(18, 0, 2),
                11,
                TickPriority::ExtremelyHigh,
                rt(10),
            ),
        ],
        vanilla: &[1, 0],
        semantic: &[0, 1],
    },
    OrderCase {
        name: "a higher priority due together runs before an earlier sequence",
        now: 5,
        ticks: &[
            (BlockPos::new(0, 64, 0), 5, N, rt(1)),
            (BlockPos::new(0, 80, 0), 5, TickPriority::High, rt(2)),
            (BlockPos::new(20, 64, 0), 5, TickPriority::VeryHigh, rt(3)),
        ],
        vanilla: &[2, 1, 0],
        semantic: &[2, 1, 0],
    },
    OrderCase {
        name: "a tie between columns goes to the lower column",
        now: 0,
        ticks: &[(HIGH, 0, N, rs(0, 1)), (LOW, 0, N, rs(0, 1))],
        vanilla: &[1, 0],
        semantic: &[1, 0],
    },
    OrderCase {
        name: "an exact tie keeps a column draining only under vanilla",
        now: 0,
        ticks: &[
            (HIGH, 0, N, rs(0, 2)),
            (HIGH_ABOVE, 0, N, rs(1, 2)),
            (LOW, 0, N, rs(1, 2)),
        ],
        vanilla: &[0, 1, 2],
        semantic: &[0, 2, 1],
    },
];

#[test]
fn collection_order_under_each_policy_does_not_depend_on_insertion_order() {
    for case in ORDER_CASES {
        for reversed in [false, true] {
            let mut queue = TickQueue::new();
            let mut ticks: Vec<_> = case.ticks.iter().enumerate().collect();
            if reversed {
                ticks.reverse();
            }
            for (_, &(pos, due, priority, sequence)) in ticks {
                queue.schedule(tick(pos, due, priority, sequence));
            }
            for (policy, expected) in [
                (Policy::Vanilla, case.vanilla),
                (Policy::Semantic, case.semantic),
            ] {
                let order: Vec<usize> = policy
                    .collect(&mut queue.clone(), case.now)
                    .iter()
                    .filter_map(|ran| {
                        case.ticks.iter().position(|&(pos, _, _, sequence)| {
                            pos == ran.pos && sequence == ran.sequence
                        })
                    })
                    .collect();
                assert_eq!(
                    order, expected,
                    "{policy:?}: {}, reversed {reversed}",
                    case.name
                );
            }
        }
    }
}

#[test]
fn a_budget_cut_leaves_every_untouched_tick_for_the_next_collection_in_order() {
    let across_columns: Vec<_> = (0..4)
        .flat_map(|column| {
            [column, column + 4]
                .into_iter()
                .enumerate()
                .map(move |(row, sequence)| {
                    (BlockPos::new(column * 16, row as i32 * 16, 0), sequence)
                })
        })
        .collect();
    let one_column: Vec<_> = [3, 0, 2, 1]
        .into_iter()
        .enumerate()
        .map(|(row, sequence)| (BlockPos::new(0, row as i32 * 16, 0), sequence))
        .collect();
    let cases: [(&[(BlockPos, i32)], u32, &[i64], &[i64]); 2] = [
        (&across_columns, 3, &[0, 1, 2], &[3, 4, 5, 6, 7]),
        (&one_column, 2, &[0, 1], &[2, 3]),
    ];
    for (ticks, budget, first, second) in cases {
        for policy in POLICIES {
            let mut queue = TickQueue::new();
            for &(pos, sequence) in ticks {
                queue.schedule(normal(pos, 5, sequence as u32));
            }
            assert_eq!(policy.sequences(&mut queue, 5, budget), first, "{policy:?}");
            assert_eq!(
                policy.sequences(&mut queue, 6, BUDGET),
                second,
                "{policy:?}"
            );
        }
    }
}

#[test]
fn an_ineligible_section_keeps_its_ticks_and_runs_them_overdue_once_eligible() {
    let held = BlockPos::new(0, 64, 0);
    let open = BlockPos::new(0, 0, 0);
    let held_section = SectionPos::from(held);
    let mut queue = TickQueue::new();
    queue.schedule(normal(held, 1, 0));
    for now in 1..4 {
        queue.schedule(normal(open, now, now as u32));
        let ran = Policy::Vanilla
            .collect_within(&mut queue, now, BUDGET, |section| section != held_section);
        assert_eq!(sequences(&ran), [now], "tick {now}");
        assert_eq!(queue.next_due(), Some(GameTick(1)));
    }
    assert_eq!(Policy::Vanilla.positions(&mut queue, 4), [held]);
}

#[test]
fn sections_far_beyond_any_vanilla_extent_keep_their_positions() {
    let far = [
        BlockPos::new(i32::MAX, i32::MIN, i32::MIN),
        BlockPos::new(-1_000_000_000, 1_000_000_000, 7),
        BlockPos::new(-1, -1, -1),
    ];
    let mut queue = TickQueue::new();
    for (sequence, pos) in far.into_iter().enumerate() {
        queue.schedule(normal(pos, 0, sequence as u32));
        assert!(queue.contains(pos, A), "{pos}");
    }
    assert_eq!(Policy::Semantic.positions(&mut queue, 0), far);
}

#[test]
fn next_due_follows_heads_scheduled_ahead_of_the_current_one() {
    let mut queue = TickQueue::new();
    queue.schedule(normal(BlockPos::new(16, 0, 0), 2000, 0));
    for i in 0..=70 {
        queue.schedule(normal(BlockPos::new(0, i, 0), 1000 - i64::from(i), 0));
    }
    assert_eq!(queue.next_due(), Some(GameTick(930)));
    assert_eq!(Policy::Vanilla.collect(&mut queue, 965).len(), 36);
    assert_eq!(queue.next_due(), Some(GameTick(966)));
    assert_eq!(Policy::Vanilla.collect(&mut queue, 1000).len(), 35);
    assert_eq!(queue.next_due(), Some(GameTick(2000)));
    assert_eq!(Policy::Vanilla.collect(&mut queue, 2000).len(), 1);
    assert_eq!(queue.next_due(), None);
}

#[test]
fn a_collection_into_reused_buffers_returns_only_its_own_ticks() {
    for policy in POLICIES {
        let mut queue = TickQueue::new();
        for (y, due) in [(0, 1), (1, 2), (2, 9)] {
            queue.schedule(normal(BlockPos::new(0, y, 0), due, due as u32));
        }
        let mut out = Vec::new();
        let mut scratch = CollectScratch::default();
        let mut dues = Vec::new();
        for now in [1, 2] {
            policy.collect_into(&mut queue, now, BUDGET, |_| true, &mut scratch, &mut out);
            dues.push(sequences(&out));
        }
        assert_eq!(dues, [[1], [2]], "{policy:?}");
    }
}

#[test]
fn queue_memory_follows_pending_ticks_and_not_head_churn() {
    let mut queue = TickQueue::new();
    queue.schedule(normal(BlockPos::new(0, 0, 0), 1_000_000_000, 0));
    let mut after_warm_up = 0;
    for now in 0..20_000 {
        queue.schedule(normal(BlockPos::new(0, 1, 0), now, now as u32));
        assert_eq!(Policy::Vanilla.collect(&mut queue, now).len(), 1);
        if now == 199 {
            after_warm_up = queue.heap_bytes();
        }
    }
    assert!(queue.heap_bytes() <= after_warm_up);
    for x in 0..1000 {
        queue.schedule(normal(BlockPos::new(x, 2, 0), 1_000_000_000, 0));
    }
    assert!(queue.heap_bytes() > after_warm_up);
}

#[test]
fn a_section_moved_between_queues_keeps_its_order_and_de_duplication() {
    let section = SectionPos::new(2, -1, 3);
    let inside = |x, z| BlockPos::new(32 + x, -16, 48 + z);
    let mut source = TickQueue::new();
    for (sequence, x) in [(3, 1), (1, 2), (2, 3)] {
        source.schedule(normal(inside(x, 0), 5, sequence));
    }
    source.schedule(normal(BlockPos::new(0, 0, 0), 9, 0));
    source.hold_saved(saved(inside(9, 9), A, 0, rs(0, 1)));
    assert_eq!(source.next_due(), Some(GameTick(5)));

    let ticks = source.take_section(section).expect("the section has ticks");
    assert_eq!((ticks.as_ref().len(), source.len()), (4, 1));
    assert!(!source.contains(inside(1, 0), A));
    assert_eq!(source.next_due(), Some(GameTick(9)));

    let mut target = TickQueue::new();
    target.schedule(normal(inside(5, 5), 9, 7));
    let refused = target
        .put_section(ticks)
        .expect_err("the target already queues this section");
    assert_eq!(target.len(), 1);
    let ticks = refused.ticks;

    let mut target = TickQueue::new();
    target.put_section(ticks).expect("an empty target");
    assert_eq!(target.len(), 4);
    for pos in [inside(2, 0), inside(9, 9)] {
        assert_eq!(
            target.schedule(normal(pos, 1, 9)),
            Queued::Duplicate,
            "{pos}"
        );
    }
    assert_eq!(target.next_due(), Some(GameTick(5)));
    assert_eq!(Policy::Vanilla.sequences(&mut target, 5, BUDGET), [1, 2, 3]);
    target.activate(section, GameTick(5));
    assert_eq!(Policy::Vanilla.positions(&mut target, 5), [inside(9, 9)]);
}

#[test]
fn ticks_read_back_for_saving_keep_their_fields_exactly() {
    let active_pos = BlockPos::new(-17, -33, -1);
    let held_pos = BlockPos::new(-31, -48, -16);
    let section = SectionPos::from(active_pos);
    assert_eq!(section, SectionPos::from(held_pos));
    let active = tick(active_pos, -40, TickPriority::VeryHigh, rt(6));
    let held = SavedTick {
        priority: TickPriority::Low,
        ..saved(held_pos, B, -7, rs(1, 3))
    };
    let mut queue = TickQueue::new();
    queue.schedule(active);
    queue.hold_saved(held);

    let view = queue.section_ticks(section).expect("the section has ticks");
    assert_eq!(view.active().collect::<Vec<_>>(), [active]);
    assert_eq!(view.held().collect::<Vec<_>>(), [held]);
    let taken = queue.take_section(section).expect("the section has ticks");
    assert_eq!(taken.as_ref().active().collect::<Vec<_>>(), [active]);
    assert_eq!(taken.as_ref().held().collect::<Vec<_>>(), [held]);
}

#[test]
fn a_save_and_load_keeps_the_run_order_of_ticks_due_together() {
    let section = SectionPos::new(0, 0, 0);
    let ticks = [
        (1, BlockPos::new(1, 0, 0)),
        (3, BlockPos::new(3, 0, 0)),
        (2, BlockPos::new(2, 0, 0)),
    ];
    let mut live = TickQueue::new();
    for (sequence, pos) in ticks {
        live.schedule(normal(pos, 5, sequence));
    }
    let list: Vec<_> = live
        .section_ticks(section)
        .expect("the section has ticks")
        .active()
        .collect();
    assert_eq!(
        list.iter()
            .map(|t| t.key.sequence.get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );

    let saved_at = 2;
    let mut loaded = TickQueue::new();
    let count = list.len() as u32;
    for (k, tick) in (0..).zip(&list) {
        let delay = (tick.key.due.0 - saved_at) as i32;
        let sequence = SubTick::restored(k, count).expect("index below count");
        loaded.hold_saved(saved(tick.pos, A, delay, sequence));
    }
    loaded.activate(section, GameTick(10));
    assert_eq!(
        Policy::Vanilla.positions(&mut loaded, 13),
        Policy::Vanilla.positions(&mut live, 5)
    );
}

#[test]
fn a_section_reads_back_held_ticks_in_list_order_and_active_ones_in_sequence_order() {
    let section = SectionPos::new(-1, 0, 2);
    let at = |x| BlockPos::new(-16 + x, 3, 32);
    let held = [
        saved(at(0), A, 9, rs(2, 3)),
        saved(at(1), A, -4, rs(0, 3)),
        saved(at(2), B, 1, rs(1, 3)),
    ];
    let mut queue = TickQueue::new();
    for tick in held {
        queue.hold_saved(tick);
    }
    for (sequence, x) in [(8, 3), (6, 4), (7, 5)] {
        queue.schedule(normal(at(x), 20 - i64::from(sequence), sequence));
    }
    let view = queue.section_ticks(section).expect("the section has ticks");
    assert_eq!(view.held().collect::<Vec<_>>(), held);
    assert_eq!(
        view.active()
            .map(|t| (t.pos, t.key.sequence.get()))
            .collect::<Vec<_>>(),
        [(at(4), 6), (at(5), 7), (at(3), 8)]
    );
}

#[test]
fn saved_ticks_are_pending_until_activation_and_then_run_in_list_order() {
    let u = BlockPos::new(0, 64, 0);
    let v = BlockPos::new(0, 0, 0);
    let late = BlockPos::new(5, 0, 5);
    for policy in POLICIES {
        let mut queue = TickQueue::new();
        for (k, pos, delay) in [(0, u, 0), (1, v, 0), (2, late, 3)] {
            queue.hold_saved(saved(pos, A, delay, rs(k, 3)));
        }
        assert!(queue.contains(u, A) && queue.contains(late, A));
        assert_eq!(queue.schedule(normal(u, 0, 0)), Queued::Duplicate);
        assert!(policy.collect(&mut queue, 100).is_empty());

        queue.activate(SectionPos::from(u), GameTick(50));
        queue.activate(SectionPos::from(v), GameTick(50));
        assert_eq!(policy.positions(&mut queue, 50), [u, v], "{policy:?}");
        assert!(policy.positions(&mut queue, 52).is_empty(), "{policy:?}");
        assert_eq!(policy.positions(&mut queue, 53), [late], "{policy:?}");
        assert!(queue.is_empty());
    }
}

#[test]
fn an_overdue_restored_tick_runs_before_a_higher_priority_tick_of_its_column() {
    let restored_pos = BlockPos::new(0, 0, 0);
    let high = BlockPos::new(0, 16, 0);
    for policy in POLICIES {
        let mut queue = TickQueue::new();
        queue.hold_saved(saved(restored_pos, A, -5, rs(0, 1)));
        queue.schedule(tick(high, 50, TickPriority::High, rt(0)));
        queue.activate(SectionPos::from(restored_pos), GameTick(50));
        assert_eq!(
            policy.positions(&mut queue, 50),
            [restored_pos, high],
            "{policy:?}"
        );
    }
}

#[test]
fn duplicate_restored_ticks_both_run_and_hold_the_position_until_the_last_one() {
    let p = BlockPos::new(4, 4, 4);
    let mut queue = TickQueue::new();
    queue.hold_saved(saved(p, A, 0, rs(0, 2)));
    queue.hold_saved(saved(p, A, 0, rs(1, 2)));
    assert_eq!(queue.schedule(normal(p, 1, 0)), Queued::Duplicate);
    queue.activate(SectionPos::from(p), GameTick(1));
    assert_eq!(queue.schedule(normal(p, 1, 0)), Queued::Duplicate);

    assert_eq!(Policy::Vanilla.sequences(&mut queue, 1, 1), [-2]);
    assert!(queue.contains(p, A));
    assert_eq!(queue.schedule(normal(p, 1, 0)), Queued::Duplicate);
    assert_eq!(Policy::Vanilla.sequences(&mut queue, 1, 1), [-1]);
    assert!(!queue.contains(p, A));
    assert_eq!(queue.schedule(normal(p, 2, 0)), Queued::Accepted);
}

#[test]
fn only_an_emptied_section_is_released() {
    let pos = BlockPos::new(7, 7, 7);
    let section = SectionPos::from(pos);
    let mut queue = TickQueue::new();
    queue.schedule(normal(pos, 1, 0));
    queue.hold_saved(saved(pos, B, 1, rs(0, 1)));
    assert!(!queue.release_empty(section));
    assert_eq!(Policy::Vanilla.sequences(&mut queue, 1, BUDGET), [0]);
    assert!(queue.holds(section) && !queue.release_empty(section));
    queue.activate(section, GameTick(1));
    assert_eq!(Policy::Vanilla.sequences(&mut queue, 2, BUDGET), [-1]);
    assert!(!queue.holds(section));
    assert!(queue.release_empty(section));
    assert_eq!(queue.sections().count(), 0);
}
