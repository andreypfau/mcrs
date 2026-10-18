use crate::probe::*;
use mcrs_minecraft_core::{BlockPos, BlockStateId};
use mcrs_minecraft_simulation::{
    BlockEvent, BlockUpdateFlags, GameTick, Message, MessageBody, Phase, SectionProperties,
    StepOutput, StepReport, TickPriority,
};
use mcrs_minecraft_volume::{Access, Unavailable};

fn hook_times(log: &[Entry], hook: Hook, pos: BlockPos) -> Vec<i64> {
    log.iter()
        .filter_map(|entry| match entry {
            Entry::Hook {
                now,
                hook: kind,
                pos: at,
                ..
            } if *kind == hook && *at == pos => Some(*now),
            _ => None,
        })
        .collect()
}

fn hook_runs(log: &[Entry], hook: Hook, now: i64) -> Vec<BlockPos> {
    log.iter()
        .filter_map(|entry| match entry {
            Entry::Hook {
                now: at,
                hook: kind,
                pos,
                ..
            } if *kind == hook && *at == now => Some(*pos),
            _ => None,
        })
        .collect()
}

fn ran(now: i64, hook: Hook, pos: BlockPos, state: BlockStateId) -> Entry {
    Entry::Hook {
        now,
        hook,
        pos,
        state,
    }
}

fn filled(sections: &[(i32, i32, i32)]) -> Host {
    let mut host = Host::new();
    for (x, y, z) in sections {
        host.insert(section(*x, *y, *z), A);
    }
    host
}

fn driver_rule(actions: Vec<Action>) -> Rule {
    Rule::on(Hook::BlockTick, at(0, 0, 0), actions).once()
}

fn driven(host: Host, rules: Vec<Rule>) -> (Runner, StepOutput<ProbeKinds>) {
    let mut host = host;
    host.put(at(0, 0, 0), C);
    let mut runner = Runner::new(host, rules);
    let trigger = [block_tick_message(0, at(0, 0, 0), BLOCK_C, 100)];
    let out = runner
        .step(100, Phase::BlockTicks, &trigger)
        .expect("owned message");
    (runner, out)
}

fn run_block_phases(runner: &mut Runner, from: i64, to: i64) {
    for now in from..=to {
        runner
            .step(now, Phase::BlockTicks, &[])
            .expect("no messages");
    }
}

fn schedule_block(pos: BlockPos, delay: i32) -> Action {
    Action::ScheduleBlock {
        pos,
        block: BLOCK_A,
        delay,
    }
}

fn check_sim_tick_002(log: &[Entry], now: i64) -> Result<(), String> {
    let order: Vec<Hook> = log
        .iter()
        .filter_map(|entry| match entry {
            Entry::Hook { now: at, hook, .. } if *at == now => Some(*hook),
            _ => None,
        })
        .collect();
    if order == [Hook::BlockTick, Hook::FluidTick, Hook::BlockEvent] {
        Ok(())
    } else {
        Err(format!("level tick {now} ran {order:?}"))
    }
}

#[test]
fn sim_tick_002_block_ticks_then_fluid_ticks_then_block_events() {
    let (p, q) = (at(1, 1, 1), at(5, 1, 1));
    let mut host = filled(&[(0, 0, 0)]);
    host.put(q, FLOWING);
    let mut runner = Runner::new(host, Vec::new());
    let inbound = [
        block_tick_message(0, p, BLOCK_A, 101),
        fluid_tick_message(1, q, WATER, 101),
        event_message(2, p, BLOCK_A),
    ];
    runner
        .step(100, Phase::Ingress, &inbound)
        .expect("owned messages");
    runner.level_tick(101, &[]);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_tick_002(&log, 101), Ok(()));

    let twin = [
        ran(101, Hook::FluidTick, q, FLOWING),
        ran(101, Hook::BlockTick, p, A),
        ran(101, Hook::BlockEvent, p, A),
    ];
    assert!(check_sim_tick_002(&twin, 101).is_err());
}

fn check_sim_tick_003(log: &[Entry], pos: BlockPos) -> Result<(), String> {
    let later: Vec<i64> = hook_times(log, Hook::BlockTick, pos)
        .into_iter()
        .filter(|now| *now > 100)
        .collect();
    if later == [102] {
        Ok(())
    } else {
        Err(format!(
            "a tick scheduled at 100 with delay 2 ran at {later:?}"
        ))
    }
}

#[test]
fn sim_tick_003_a_tick_scheduled_with_delay_two_runs_two_ticks_later() {
    let p = at(1, 1, 1);
    let schedule = Rule::on(Hook::BlockTick, p, vec![schedule_block(p, 2)]).once();
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), vec![schedule]);
    let inbound = [block_tick_message(0, p, BLOCK_A, 100)];
    runner
        .step(100, Phase::BlockTicks, &inbound)
        .expect("owned message");
    run_block_phases(&mut runner, 101, 102);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_tick_003(&log, p), Ok(()));

    let twin = [
        ran(100, Hook::BlockTick, p, A),
        ran(101, Hook::BlockTick, p, A),
    ];
    assert!(check_sim_tick_003(&twin, p).is_err());
}

fn check_sim_sch_003(log: &[Entry], first: BlockPos, second: BlockPos) -> Result<(), String> {
    let order: Vec<BlockPos> = hook_runs(log, Hook::BlockTick, 103);
    if order == [first, second] {
        Ok(())
    } else {
        Err(format!("ticks due at 103 ran at {order:?}"))
    }
}

#[test]
fn sim_sch_003_ticks_due_together_run_in_scheduling_order() {
    for (p, q) in [(at(5, 1, 1), at(2, 1, 1)), (at(17, 1, 1), at(1, 1, 1))] {
        let rules = vec![driver_rule(vec![
            schedule_block(p, 3),
            schedule_block(q, 3),
        ])];
        let (mut runner, _) = driven(filled(&[(0, 0, 0), (1, 0, 0)]), rules);
        run_block_phases(&mut runner, 101, 103);
        let log = runner.behaviour.log();
        assert_eq!(check_sim_sch_003(&log, p, q), Ok(()));

        let twin = [
            ran(103, Hook::BlockTick, q, A),
            ran(103, Hook::BlockTick, p, A),
        ];
        assert!(check_sim_sch_003(&twin, p, q).is_err());
    }
}

fn check_sim_sch_004(log: &[Entry], pos: BlockPos) -> Result<(), String> {
    let states: Vec<BlockStateId> = log
        .iter()
        .filter_map(|entry| match entry {
            Entry::Hook {
                now: 105,
                hook: Hook::BlockTick,
                pos: at,
                state,
            } if *at == pos => Some(*state),
            _ => None,
        })
        .collect();
    if states == [A, B] {
        Ok(())
    } else {
        Err(format!("handlers at 105 saw {states:?}"))
    }
}

#[test]
fn sim_sch_004_a_replacement_block_keeps_its_own_pending_tick() {
    let (d, p) = (at(0, 0, 0), at(5, 1, 1));
    assert_ne!(d, p);
    let rules = vec![
        driver_rule(vec![
            schedule_block(p, 5),
            Action::ScheduleBlock {
                pos: p,
                block: BLOCK_B,
                delay: 5,
            },
        ]),
        Rule::on(
            Hook::BlockTick,
            p,
            vec![Action::Set {
                pos: p,
                state: B,
                flags: 3,
                limit: 512,
            }],
        )
        .once(),
    ];
    let (mut runner, _) = driven(filled(&[(0, 0, 0)]), rules);
    run_block_phases(&mut runner, 101, 105);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_sch_004(&log, p), Ok(()));

    let twin = [ran(105, Hook::BlockTick, p, A)];
    assert!(check_sim_sch_004(&twin, p).is_err());
}

fn check_sim_sch_005(
    log: &[Entry],
    runs: BlockPos,
    refuses: BlockPos,
    unaccepted: u32,
) -> Result<(), String> {
    let ran_at = hook_times(log, Hook::BlockTick, runs);
    let never = hook_times(log, Hook::BlockTick, refuses);
    if ran_at == [150] && never.is_empty() && unaccepted == 1 {
        Ok(())
    } else {
        Err(format!(
            "ran at {ran_at:?}, refused section ran {never:?}, {unaccepted} dropped"
        ))
    }
}

#[test]
fn sim_sch_005_ticks_wait_for_a_section_that_runs_them() {
    let (accepts, refuses) = (at(17, 1, 1), at(33, 1, 1));
    let mut host = filled(&[(0, 0, 0), (1, 0, 0), (2, 0, 0)]);
    host.set_properties(section(1, 0, 0), SectionProperties::ACCEPTS_TICKS);
    host.set_properties(section(2, 0, 0), SectionProperties::RUNS_SCHEDULED_TICKS);
    let rules = vec![driver_rule(vec![
        schedule_block(accepts, 1),
        schedule_block(refuses, 1),
    ])];
    let (mut runner, out) = driven(host, rules);
    let unaccepted = out.report.unaccepted_ticks;
    run_block_phases(&mut runner, 101, 149);
    runner.host.set_properties(
        section(1, 0, 0),
        SectionProperties::ACCEPTS_TICKS | SectionProperties::RUNS_SCHEDULED_TICKS,
    );
    run_block_phases(&mut runner, 150, 150);
    let log = runner.behaviour.log();
    assert_eq!(
        check_sim_sch_005(&log, accepts, refuses, unaccepted),
        Ok(())
    );

    let twin = [ran(151, Hook::BlockTick, accepts, A)];
    assert!(check_sim_sch_005(&twin, accepts, refuses, 1).is_err());
}

fn check_sim_sch_007(log: &[Entry], block: BlockPos, fluid: BlockPos) -> Result<(), String> {
    let block_ran = hook_times(log, Hook::BlockTick, block);
    let fluid_ran = log.contains(&ran(102, Hook::FluidTick, fluid, SOURCE));
    if block_ran.iter().all(|now| *now < 101) && fluid_ran {
        Ok(())
    } else {
        Err(format!(
            "block handler ran at {block_ran:?}, fluid handler at 102: {fluid_ran}"
        ))
    }
}

#[test]
fn sim_sch_007_a_replaced_block_consumes_its_tick_and_a_fluid_tick_survives_source_and_flowing() {
    let (p, q) = (at(5, 1, 1), at(6, 1, 1));
    let mut host = filled(&[(0, 0, 0)]);
    host.put(q, FLOWING);
    host.put(p, A);
    let rules = vec![driver_rule(vec![
        schedule_block(p, 2),
        Action::ScheduleFluid {
            pos: q,
            fluid: WATER,
            delay: 2,
        },
    ])];
    let (mut runner, _) = driven(host, rules);
    let replacements = [set_message(0, p, B, 3), set_message(1, q, SOURCE, 530)];
    runner.level_tick(101, &replacements);
    runner.level_tick(102, &[]);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_sch_007(&log, p, q), Ok(()));

    let twin = [ran(102, Hook::BlockTick, p, B)];
    assert!(check_sim_sch_007(&twin, p, q).is_err());
}

fn block_positions(count: usize) -> Vec<BlockPos> {
    (0..count)
        .map(|index| {
            let (section, local) = (index / 4096, index % 4096);
            let at_section = i32::try_from(section).expect("few sections") * 16;
            let (x, z, y) = (local % 16, (local / 16) % 16, local / 256);
            at(
                at_section + i32::try_from(x).expect("small"),
                i32::try_from(y).expect("small"),
                i32::try_from(z).expect("small"),
            )
        })
        .collect()
}

fn check_sim_sch_008(
    first: &[BlockPos],
    second: &[BlockPos],
    scheduled: &[BlockPos],
) -> Result<(), String> {
    let in_order = first.iter().chain(second).eq(scheduled);
    if first.len() == 65_536 && second.len() == 4_464 && in_order {
        Ok(())
    } else {
        Err(format!(
            "{} ran in the first phase and {} in the next",
            first.len(),
            second.len()
        ))
    }
}

#[test]
fn sim_sch_008_seventy_thousand_due_ticks_run_65536_now_and_the_rest_next() {
    let scheduled = block_positions(70_000);
    let sections: Vec<(i32, i32, i32)> = (0..18).map(|x| (x, 0, 0)).collect();
    let mut runner = Runner::new(filled(&sections), Vec::new());
    let inbound: Vec<_> = scheduled
        .iter()
        .enumerate()
        .map(|(index, pos)| {
            let counter = u32::try_from(index).expect("under 2^23");
            block_tick_message(counter, *pos, BLOCK_A, 100)
        })
        .collect();
    runner
        .step(100, Phase::BlockTicks, &inbound)
        .expect("owned messages");
    runner
        .step(101, Phase::BlockTicks, &[])
        .expect("no messages");
    let log = runner.behaviour.log();
    let first = hook_runs(&log, Hook::BlockTick, 100);
    let second = hook_runs(&log, Hook::BlockTick, 101);
    assert_eq!(check_sim_sch_008(&first, &second, &scheduled), Ok(()));

    assert!(check_sim_sch_008(&scheduled, &[], &scheduled).is_err());
}

fn check_sim_sch_009_cross_kind(
    log: &[Entry],
    block: BlockPos,
    fluid: BlockPos,
) -> Result<(), String> {
    let block_ran = hook_times(log, Hook::BlockTick, block);
    let fluid_ran = hook_times(log, Hook::FluidTick, fluid);
    if block_ran == [100, 101] && fluid_ran == [100] {
        Ok(())
    } else {
        Err(format!(
            "block ticks ran at {block_ran:?}, fluid ticks at {fluid_ran:?}"
        ))
    }
}

#[test]
fn sim_sch_009_a_fluid_tick_scheduled_by_a_block_tick_runs_in_the_same_level_tick() {
    let (p, q) = (at(5, 1, 1), at(6, 1, 1));
    let mut host = filled(&[(0, 0, 0)]);
    host.put(q, FLOWING);
    let rules = vec![
        Rule::on(
            Hook::BlockTick,
            p,
            vec![
                schedule_block(p, 0),
                Action::ScheduleFluid {
                    pos: q,
                    fluid: WATER,
                    delay: 0,
                },
            ],
        )
        .once(),
    ];
    let mut runner = Runner::new(host, rules);
    runner.level_tick(100, &[block_tick_message(0, p, BLOCK_A, 100)]);
    runner.level_tick(101, &[]);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_sch_009_cross_kind(&log, p, q), Ok(()));

    let twin = [
        ran(100, Hook::BlockTick, p, A),
        ran(100, Hook::BlockTick, p, A),
        ran(100, Hook::FluidTick, q, FLOWING),
        ran(101, Hook::BlockTick, p, A),
    ];
    assert!(check_sim_sch_009_cross_kind(&twin, p, q).is_err());
}

fn check_sim_sch_009_reschedule(log: &[Entry], pos: BlockPos) -> Result<(), String> {
    let times = hook_times(log, Hook::BlockTick, pos);
    if times == [100, 101, 102] {
        Ok(())
    } else {
        Err(format!("self-rescheduling tick ran at {times:?}"))
    }
}

#[test]
fn sim_sch_009_a_block_tick_rescheduling_itself_with_delay_zero_runs_once_per_level_tick() {
    let p = at(5, 1, 1);
    let rules = vec![Rule::on(Hook::BlockTick, p, vec![schedule_block(p, 0)])];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    runner.level_tick(100, &[block_tick_message(0, p, BLOCK_A, 100)]);
    runner.level_tick(101, &[]);
    runner.level_tick(102, &[]);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_sch_009_reschedule(&log, p), Ok(()));

    let twin: Vec<Entry> = [100, 100, 101, 101, 102]
        .into_iter()
        .map(|now| ran(now, Hook::BlockTick, p, A))
        .collect();
    assert!(check_sim_sch_009_reschedule(&twin, p).is_err());
}

fn answer(query: Query, pos: BlockPos, value: bool) -> Entry {
    Entry::Answer {
        now: 100,
        query,
        pos,
        value,
    }
}

fn check_sim_sch_010(log: &[Entry], p: BlockPos, q: BlockPos) -> Result<(), String> {
    let answers: Vec<Entry> = log
        .iter()
        .copied()
        .filter(|entry| matches!(entry, Entry::Answer { .. }))
        .collect();
    let expected = [
        answer(Query::WillTick, q, true),
        answer(Query::HasScheduled, q, false),
        answer(Query::WillTick, p, false),
    ];
    if answers == expected {
        Ok(())
    } else {
        Err(format!("queries answered {answers:?}"))
    }
}

#[test]
fn sim_sch_010_queries_see_collected_and_pending_ticks() {
    let (p, q) = (at(5, 1, 1), at(6, 1, 1));
    let rules = vec![
        Rule::on(
            Hook::BlockTick,
            p,
            vec![
                Action::AskWillTick {
                    pos: q,
                    block: BLOCK_A,
                },
                Action::AskHasScheduled {
                    pos: q,
                    block: BLOCK_A,
                },
            ],
        ),
        Rule::on(
            Hook::BlockTick,
            q,
            vec![Action::AskWillTick {
                pos: p,
                block: BLOCK_A,
            }],
        ),
    ];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    let inbound = [
        block_tick_message(0, p, BLOCK_A, 100),
        block_tick_message(1, q, BLOCK_A, 100),
    ];
    runner
        .step(100, Phase::BlockTicks, &inbound)
        .expect("owned messages");
    let log = runner.behaviour.log();
    assert_eq!(check_sim_sch_010(&log, p, q), Ok(()));

    let twin = [
        answer(Query::WillTick, q, false),
        answer(Query::HasScheduled, q, true),
        answer(Query::WillTick, p, false),
    ];
    assert!(check_sim_sch_010(&twin, p, q).is_err());
}

fn event_runs(log: &[Entry]) -> Vec<(i64, BlockPos)> {
    log.iter()
        .filter_map(|entry| match entry {
            Entry::Hook {
                now,
                hook: Hook::BlockEvent,
                pos,
                ..
            } => Some((*now, *pos)),
            _ => None,
        })
        .collect()
}

fn check_sim_bev_002(log: &[Entry], p: BlockPos, q: BlockPos) -> Result<(), String> {
    let runs = event_runs(log);
    if runs == [(100, p), (100, q)] {
        Ok(())
    } else {
        Err(format!("events were delivered as {runs:?}"))
    }
}

#[test]
fn sim_bev_002_an_event_queued_during_the_drain_is_delivered_in_the_same_drain() {
    let (p, q) = (at(5, 1, 1), at(6, 1, 1));
    let queue_q = Action::QueueEvent {
        pos: q,
        block: BLOCK_A,
        a: 0,
        b: 0,
    };
    let rules = vec![Rule::on(Hook::BlockEvent, p, vec![queue_q]).once()];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    runner.level_tick(100, &[event_message(0, p, BLOCK_A)]);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_bev_002(&log, p, q), Ok(()));

    let twin = [
        ran(100, Hook::BlockEvent, p, A),
        ran(101, Hook::BlockEvent, q, A),
    ];
    assert!(check_sim_bev_002(&twin, p, q).is_err());
}

fn check_sim_bev_003(log: &[Entry], pos: BlockPos, gained_at: i64) -> Result<(), String> {
    let delivered = hook_times(log, Hook::BlockEvent, pos);
    if delivered == [gained_at] {
        Ok(())
    } else {
        Err(format!(
            "delivered at {delivered:?}, section gained delivery at {gained_at}"
        ))
    }
}

#[test]
fn sim_bev_003_an_event_waits_until_its_section_delivers_block_events() {
    let p = at(17, 1, 1);
    let mut host = filled(&[(0, 0, 0), (1, 0, 0)]);
    let silent = SectionProperties::ACCEPTS_TICKS
        | SectionProperties::RUNS_SCHEDULED_TICKS
        | SectionProperties::BROADCASTS_CHANGES;
    host.set_properties(section(1, 0, 0), silent);
    let mut runner = Runner::new(host, Vec::new());
    let first = runner.level_tick(100, &[event_message(0, p, BLOCK_A)]);
    assert_eq!(first.report.events_deferred, 1);
    for now in 101..105 {
        let out = runner.level_tick(now, &[]);
        assert_eq!(out.report.events_deferred, 1);
        assert!(out.delivered.is_empty());
    }
    runner
        .host
        .set_properties(section(1, 0, 0), SectionProperties::all());
    let out = runner.level_tick(105, &[]);
    assert_eq!(out.delivered.len(), 1);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_bev_003(&log, p, 105), Ok(()));

    let twin = [ran(103, Hook::BlockEvent, p, A)];
    assert!(check_sim_bev_003(&twin, p, 105).is_err());
}

fn check_sim_bev_004(
    log: &[Entry],
    delivered: &[BlockEvent<ProbeBlock>],
    expected: [BlockPos; 3],
) -> Result<(), String> {
    let [replaced, changed, refused] = expected;
    let handlers: Vec<BlockPos> = event_runs(log).into_iter().map(|(_, pos)| pos).collect();
    let sent: Vec<BlockPos> = delivered.iter().map(|event| event.pos).collect();
    if handlers == [changed, refused] && sent == [changed] {
        Ok(())
    } else {
        Err(format!(
            "handlers ran at {handlers:?} (replaced block at {replaced}), delivered {sent:?}"
        ))
    }
}

#[test]
fn sim_bev_004_an_event_whose_block_was_replaced_is_dropped() {
    let positions = [at(5, 1, 1), at(6, 1, 1), at(7, 1, 1)];
    let [replaced, changed, refused] = positions;
    let rules = vec![Rule::on(Hook::BlockEvent, refused, Vec::new()).answering(false)];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    let inbound = [
        event_message(0, replaced, BLOCK_A),
        event_message(1, changed, BLOCK_A),
        event_message(2, refused, BLOCK_A),
        set_message(3, replaced, B, 3),
        set_message(4, changed, A_OTHER, 3),
    ];
    let out = runner.level_tick(100, &inbound);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_bev_004(&log, &out.delivered, positions), Ok(()));

    let event = |pos| BlockEvent {
        pos,
        block: BLOCK_A,
        a: 0,
        b: 0,
    };
    let handler_on_replaced = [
        ran(100, Hook::BlockEvent, replaced, B),
        ran(100, Hook::BlockEvent, changed, A_OTHER),
        ran(100, Hook::BlockEvent, refused, A),
    ];
    let delivered = [event(changed)];
    assert!(check_sim_bev_004(&handler_on_replaced, &delivered, positions).is_err());
    let delivered_refused = [event(changed), event(refused)];
    assert!(check_sim_bev_004(&log, &delivered_refused, positions).is_err());
}

struct Observed {
    outbound: Vec<Message<ProbeKinds>>,
    report: StepReport,
    waiting_events: u32,
    reads: Vec<Entry>,
    writes: usize,
    tick_runs: usize,
    event_runs: usize,
}

const LEASED: (i32, i32, i32) = (1, 0, 0);
const FOREIGN: (i32, i32, i32) = (2, 0, 0);

fn leased() -> BlockPos {
    at(17, 1, 1)
}

fn foreign() -> BlockPos {
    at(33, 1, 1)
}

fn unloaded() -> BlockPos {
    at(49, 1, 1)
}

fn outside() -> BlockPos {
    at(2000, 1, 1)
}

fn read(pos: BlockPos, result: Result<BlockStateId, Unavailable>) -> Entry {
    Entry::Read {
        now: 100,
        pos,
        result,
    }
}

fn expected_bodies(pos: BlockPos) -> [MessageBody<ProbeKinds>; 3] {
    [
        MessageBody::Set {
            pos,
            state: A_OTHER,
            flags: BlockUpdateFlags::from_bits_retain(3),
            limit: 512,
        },
        MessageBody::BlockTick {
            pos,
            block: BLOCK_A,
            due: GameTick(104),
            priority: TickPriority::Normal,
        },
        MessageBody::Event(BlockEvent {
            pos,
            block: BLOCK_A,
            a: 1,
            b: 2,
        }),
    ]
}

fn check_sim_model_007(observed: &Observed) -> Result<(), String> {
    let expected: Vec<_> = [leased(), foreign()]
        .into_iter()
        .flat_map(expected_bodies)
        .collect();
    let bodies: Vec<_> = observed.outbound.iter().map(|m| m.body).collect();
    let stamped = observed
        .outbound
        .iter()
        .all(|m| m.stamp == m.origin && m.stamp >= stamp(100, 0) && m.stamp < stamp(101, 0));
    let ascending = observed.outbound.windows(2).all(|pair| match pair {
        [earlier, later] => earlier.stamp < later.stamp,
        _ => true,
    });
    let report = observed.report;
    let counts = (
        report.refused_sets,
        report.unaccepted_ticks,
        report.unloaded_drops,
        report.foreign_reads,
        observed.waiting_events,
    );
    let reads = [
        read(leased(), Ok(A)),
        read(foreign(), Err(Unavailable::Foreign)),
        read(unloaded(), Err(Unavailable::Unloaded)),
        read(outside(), Ok(VOID_AIR)),
    ];
    let applied = (observed.writes, observed.tick_runs, observed.event_runs);
    if bodies == expected
        && stamped
        && ascending
        && counts == (2, 2, 1, 1, 1)
        && observed.reads == reads
        && applied == (2, 2, 2)
    {
        Ok(())
    } else {
        Err(format!(
            "messages {bodies:?}, stamped {stamped}, ascending {ascending}, counts {counts:?}, \
             reads {:?}, applied {applied:?}",
            observed.reads
        ))
    }
}

fn reach(target: BlockPos) -> Vec<Action> {
    vec![
        Action::Set {
            pos: target,
            state: A_OTHER,
            flags: 3,
            limit: 512,
        },
        schedule_block(target, 4),
        Action::QueueEvent {
            pos: target,
            block: BLOCK_A,
            a: 1,
            b: 2,
        },
        Action::Read { pos: target },
    ]
}

fn observe_foreign_effects() -> Observed {
    let handler = at(1, 1, 1);
    let mut host = filled(&[(0, 0, 0), LEASED, FOREIGN]);
    host.set_access(section(1, 0, 0), Access::Read);
    host.set_access(section(2, 0, 0), Access::Foreign);
    let actions = [leased(), foreign(), unloaded(), outside()]
        .into_iter()
        .flat_map(reach)
        .collect();
    let rules = vec![Rule::on(Hook::BlockTick, handler, actions).once()];
    let mut sender = Runner::new(host, rules);
    let inbound = [block_tick_message(0, handler, BLOCK_A, 100)];
    let out = sender
        .step(100, Phase::BlockTicks, &inbound)
        .expect("owned message");
    let drain = sender
        .step(100, Phase::BlockEvents, &[])
        .expect("no messages");
    let reads: Vec<Entry> = sender
        .behaviour
        .log()
        .into_iter()
        .filter(|entry| matches!(entry, Entry::Read { .. }))
        .collect();

    let mut writer = Runner::new(filled(&[LEASED, FOREIGN]), Vec::new());
    let mut writes = 0;
    for now in 101..=104 {
        let inbound: &[_] = if now == 101 { &out.outbound } else { &[] };
        writes += writer.level_tick(now, inbound).writes.len();
    }
    let log = writer.behaviour.log();
    let count = |hook| {
        [leased(), foreign()]
            .into_iter()
            .map(|pos| hook_times(&log, hook, pos).len())
            .sum()
    };
    Observed {
        outbound: out.outbound,
        report: out.report,
        waiting_events: drain.report.events_deferred,
        reads,
        writes,
        tick_runs: count(Hook::BlockTick),
        event_runs: count(Hook::BlockEvent),
    }
}

#[test]
fn sim_model_007_effects_on_sections_the_shard_does_not_write_leave_as_messages() {
    let observed = observe_foreign_effects();
    assert_eq!(check_sim_model_007(&observed), Ok(()));

    let dropped = Observed {
        outbound: Vec::new(),
        ..observed_copy(&observed)
    };
    assert!(check_sim_model_007(&dropped).is_err());
    let twice = Observed {
        writes: observed.writes * 2,
        tick_runs: observed.tick_runs * 2,
        event_runs: observed.event_runs * 2,
        ..observed_copy(&observed)
    };
    assert!(check_sim_model_007(&twice).is_err());

    let pos = at(1, 1, 1);
    let schedule_and_queue = vec![
        schedule_block(pos, 3),
        Action::QueueEvent {
            pos,
            block: BLOCK_A,
            a: 0,
            b: 0,
        },
    ];
    let mut cell = Runner::new(
        filled(&[(0, 0, 0)]),
        vec![Rule::on(Hook::Placed, pos, schedule_and_queue)],
    );
    cell.kind = mcrs_minecraft_simulation::PartitionKind::Cell;
    let out = cell
        .step(100, Phase::Ingress, &[set_message(0, pos, B, 3)])
        .expect("a cell takes sets");
    let bodies: Vec<_> = out.outbound.iter().map(|m| m.body).collect();
    assert_eq!(
        bodies,
        [
            MessageBody::BlockTick {
                pos,
                block: BLOCK_A,
                due: GameTick(103),
                priority: TickPriority::Normal,
            },
            MessageBody::Event(BlockEvent {
                pos,
                block: BLOCK_A,
                a: 0,
                b: 0,
            }),
        ]
    );
    cell.level_tick(103, &[]);
    assert!(hook_times(&cell.behaviour.log(), Hook::BlockTick, pos).is_empty());
}

fn observed_copy(observed: &Observed) -> Observed {
    Observed {
        outbound: observed.outbound.clone(),
        report: observed.report,
        waiting_events: observed.waiting_events,
        reads: observed.reads.clone(),
        writes: observed.writes,
        tick_runs: observed.tick_runs,
        event_runs: observed.event_runs,
    }
}

#[test]
fn a_message_for_a_section_another_partition_holds_is_refused_before_any_message_applies() {
    let (owned, leased) = (at(1, 1, 1), at(17, 1, 1));
    let mut host = filled(&[(0, 0, 0), (1, 0, 0)]);
    host.set_access(section(1, 0, 0), Access::Read);
    let mut runner = Runner::new(host, Vec::new());
    let inbound = [
        set_message(0, owned, B, 3),
        block_tick_message(1, leased, BLOCK_A, 100),
    ];
    let mut out = StepOutput::default();
    let result = runner.step_into(100, Phase::BlockTicks, &inbound, &mut out);
    assert_eq!(
        result,
        Err(mcrs_minecraft_simulation::StepError::NotOwned {
            section: section(1, 0, 0)
        })
    );
    assert_eq!(runner.host.state(owned), A);
    assert_eq!(out, StepOutput::default());
}
