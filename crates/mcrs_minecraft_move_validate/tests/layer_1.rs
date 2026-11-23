#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::too_many_lines,
    reason = "a simulation that fails loudly on a broken invariant"
)]

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::time::Duration;

use glam::DVec3;
use mcrs_minecraft_move_validate::{
    Accept, Acknowledged, Action, ChangeState, Checked, Finding, Look, Move, MoveGuard, Report,
    Reports, Serverbound,
};
use proptest::prelude::*;

const SERVER_TICK_MS: u64 = 50;
const SERVER_TICKS: u64 = 200;

#[derive(Clone, Copy, Debug)]
struct Link {
    down_ms: u64,
    up_ms: u64,
    jitter_ms: u64,
    frame_ms: u64,
    stall_at_ms: u64,
    stall_ms: u64,
    seed: u64,
}

struct Draws(u64);

impl Draws {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound.max(1)
    }
}

#[derive(Clone, Copy, Debug)]
enum Clientbound {
    Change(u64),
    Ping(i32),
    Other,
}

#[derive(Clone, Copy, Debug)]
enum Event {
    ServerTick(u64),
    Deliver(Clientbound),
    Frame,
    Arrive(Serverbound),
}

#[derive(Default)]
struct Queue {
    heap: BinaryHeap<Reverse<(Duration, usize)>>,
    events: Vec<Event>,
}

impl Queue {
    fn push(&mut self, at: Duration, event: Event) {
        self.heap.push(Reverse((at, self.events.len())));
        self.events.push(event);
    }

    fn pop(&mut self) -> Option<(Duration, Event)> {
        let Reverse((at, index)) = self.heap.pop()?;
        Some((at, self.events[index]))
    }
}

#[derive(Debug, Default, PartialEq)]
struct Log {
    actions: Vec<Action>,
    reports: Vec<Report>,
    ticks_sent: u64,
    tick_rate_level_max: u64,
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn checked(packet: Serverbound) -> Checked {
    Checked {
        packet,
        reports: Reports::default(),
    }
}

fn position_move() -> Serverbound {
    Serverbound::Move(Move {
        position: Some(DVec3::ZERO),
        look: Some(Look::default()),
        ..Move::default()
    })
}

fn simulate(link: Link) -> Log {
    let mut draws = Draws(link.seed);
    let mut guard = MoveGuard::new();
    let mut log = Log::default();
    let mut queue = Queue::default();
    let stall = ms(link.stall_at_ms)..ms(link.stall_at_ms + link.stall_ms);
    let mut last_down = Duration::ZERO;
    let mut last_up = Duration::ZERO;
    let delay = |draws: &mut Draws, last: &mut Duration, now: Duration, base_ms: u64| {
        let mut at = now + ms(base_ms + draws.below(link.jitter_ms + 1));
        if stall.contains(&at) {
            at = stall.end;
        }
        *last = (*last).max(at);
        *last
    };

    for tick in 0..SERVER_TICKS {
        queue.push(ms(tick * SERVER_TICK_MS), Event::ServerTick(tick + 1));
    }
    let mut next_change = 0;
    let mut client_start: Option<Duration> = None;
    let mut inbox: Vec<Clientbound> = Vec::new();
    let mut applied = 0;
    let mut client_ticks = 0;
    let mut silent = 0;
    let mut applied_by_tick: Vec<u64> = vec![0];
    let mut last_state: HashMap<u64, ChangeState> = HashMap::new();
    let mut level: u64 = 0;
    let mut acknowledged = Acknowledged::default();
    let end = ms(SERVER_TICKS * SERVER_TICK_MS + 2_000);

    while let Some((now, event)) = queue.pop() {
        match event {
            Event::ServerTick(tick) => {
                let mut wire = vec![Clientbound::Ping(tick as i32), Clientbound::Other];
                for _ in 0..draws.below(4).saturating_sub(1) {
                    guard.change(tick);
                    wire.push(Clientbound::Change(next_change));
                    next_change += 1;
                    if draws.below(2) == 0 {
                        wire.push(Clientbound::Other);
                    }
                }
                guard.end_server_tick(now);
                for packet in wire {
                    let at = delay(&mut draws, &mut last_down, now, link.down_ms);
                    queue.push(at, Event::Deliver(packet));
                }
            }
            Event::Deliver(packet) => {
                inbox.push(packet);
                if client_start.is_none() {
                    client_start = Some(now);
                    queue.push(now, Event::Frame);
                }
            }
            Event::Frame => {
                let start = client_start.unwrap_or(now);
                let mut sent = Vec::new();
                for packet in inbox.drain(..) {
                    match packet {
                        Clientbound::Change(change) => applied = change + 1,
                        Clientbound::Ping(id) => sent.push(Serverbound::Pong(id)),
                        Clientbound::Other => {}
                    }
                }
                let due = ((now - start).as_millis() as u64 / SERVER_TICK_MS + 1) as usize;
                let run = due.saturating_sub(client_ticks).min(10);
                for _ in 0..run {
                    if client_ticks == 0 {
                        sent.push(Serverbound::PlayerLoaded);
                    }
                    if silent == 19 || draws.below(2) == 0 {
                        sent.push(position_move());
                        silent = 0;
                    } else {
                        silent += 1;
                        if draws.below(3) == 0 {
                            sent.push(Serverbound::Move(Move {
                                look: Some(Look::default()),
                                ..Move::default()
                            }));
                        }
                    }
                    sent.push(Serverbound::ClientTickEnd);
                    client_ticks += 1;
                    applied_by_tick.push(applied);
                }
                client_ticks = client_ticks.max(due);
                for packet in sent {
                    let at = delay(&mut draws, &mut last_up, now, link.up_ms);
                    queue.push(at, Event::Arrive(packet));
                }
                if now < end {
                    queue.push(now + ms(link.frame_ms), Event::Frame);
                }
            }
            Event::Arrive(Serverbound::Pong(id)) => {
                let tick = id as u64;
                acknowledged = Acknowledged {
                    tick,
                    sent_at: ms((tick - 1) * SERVER_TICK_MS),
                };
                log.actions.push(Action::Drop);
            }
            Event::Arrive(packet) => {
                let mut reports = Vec::new();
                let action = guard.serverbound(&checked(packet), now, acknowledged, &mut reports);
                match (packet, action) {
                    (Serverbound::ClientTickEnd, Action::EndTick(tick)) => {
                        log.ticks_sent += 1;
                        assert_eq!(tick, log.ticks_sent, "MVC-INV-006");
                        let had = applied_by_tick[tick as usize];
                        let mut previous = ChangeState::Applied;
                        for view in guard.changes() {
                            assert!(
                                view.change >= had || view.state != ChangeState::NotApplied,
                                "MVC-LAT-004: change {} applied before tick {tick}",
                                view.change
                            );
                            assert!(
                                view.change < had || view.state != ChangeState::Applied,
                                "MVC-LAT-004: change {} not applied before tick {tick}",
                                view.change
                            );
                            assert!(view.state <= previous, "MVC-LAT-005: order of changes");
                            previous = view.state;
                            let last = last_state.insert(view.change, view.state);
                            assert!(last.is_none_or(|last| last <= view.state), "MVC-INV-007");
                        }
                        if reports
                            .iter()
                            .any(|report| report.finding == Finding::TickRate)
                        {
                            level += 1_000;
                        } else {
                            level = level.saturating_sub(5);
                        }
                        log.tick_rate_level_max = log.tick_rate_level_max.max(level);
                    }
                    (_, Action::Forward) => {}
                    (packet, action) => panic!("MVC-INV-009: {action:?} for {packet:?}"),
                }
                let other = reports
                    .iter()
                    .find(|report| report.finding != Finding::TickRate);
                assert!(other.is_none(), "MVC-INV-009: {other:?}");
                log.actions.push(action);
                log.reports.extend(reports);
            }
        }
    }
    assert_eq!(
        log.ticks_sent as usize,
        applied_by_tick.len() - 1,
        "MVC-INV-006"
    );
    log
}

fn links() -> impl Strategy<Value = Link> {
    (
        0..=500u64,
        0..=500u64,
        0..=100u64,
        prop_oneof![Just(7u64), Just(50), Just(200), 7..=200u64],
        1_000..=5_000u64,
        prop_oneof![Just(0u64), Just(3_000), 0..=3_000u64],
        any::<u64>(),
    )
        .prop_map(
            |(down_ms, up_ms, jitter_ms, frame_ms, stall_at_ms, stall_ms, seed)| Link {
                down_ms,
                up_ms,
                jitter_ms,
                frame_ms,
                stall_at_ms,
                stall_ms,
                seed,
            },
        )
}

#[test]
fn a_client_at_5_frames_each_second_on_a_fast_link_passes_the_tick_rate_check() {
    let log = simulate(Link {
        down_ms: 0,
        up_ms: 0,
        jitter_ms: 0,
        frame_ms: 199,
        stall_at_ms: 1_000,
        stall_ms: 0,
        seed: 1,
    });
    assert!(
        log.tick_rate_level_max <= 10_000,
        "MVC-CHK-011: {}",
        log.tick_rate_level_max
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn a_legit_client_passes_layer_1_on_any_link(link in links()) {
        let log = simulate(link);
        prop_assert!(log.tick_rate_level_max <= 10_000, "MVC-CHK-011: {}", log.tick_rate_level_max);
        prop_assert_eq!(simulate(link), log);
    }

    #[test]
    fn a_stream_that_a_vanilla_server_accepts_never_disconnects(
        packets in prop::collection::vec(0..6u8, 0..400),
    ) {
        let mut guard = MoveGuard::new();
        let mut positioned = false;
        let mut reports = Vec::new();
        for (index, kind) in packets.into_iter().enumerate() {
            let packet = match kind {
                0 if !positioned => {
                    positioned = true;
                    position_move()
                }
                1 => Serverbound::Move(Move::default()),
                2 => Serverbound::Pong(index as i32),
                3 => Serverbound::Accept(Accept { teleport_id: index as i32, ..Accept::default() }),
                _ => {
                    positioned = false;
                    Serverbound::ClientTickEnd
                }
            };
            let action = guard.serverbound(&checked(packet), ms(index as u64), Acknowledged::default(), &mut reports);
            prop_assert!(!matches!(action, Action::Disconnect(_)), "{packet:?}");
        }
    }
}

#[test]
fn a_second_move_in_one_client_tick_is_dropped_and_a_second_position_disconnects() {
    let mut guard = MoveGuard::new();
    let mut reports = Vec::new();
    let mut take = |guard: &mut MoveGuard, packet| {
        guard.serverbound(
            &checked(packet),
            ms(0),
            Acknowledged::default(),
            &mut reports,
        )
    };
    let rotation = Serverbound::Move(Move {
        look: Some(Look::default()),
        ..Move::default()
    });
    assert_eq!(take(&mut guard, rotation), Action::Forward);
    assert_eq!(take(&mut guard, rotation), Action::Drop);
    assert_eq!(take(&mut guard, position_move()), Action::Drop);
    assert!(matches!(
        take(&mut guard, Serverbound::ClientTickEnd),
        Action::EndTick(1)
    ));
    assert_eq!(take(&mut guard, position_move()), Action::Forward);
    assert!(matches!(
        take(&mut guard, position_move()),
        Action::Disconnect(_)
    ));
    assert!(
        reports
            .iter()
            .all(|report| report.finding == Finding::SecondMove && report.client_tick == 1)
    );
}

#[test]
fn the_twentieth_silent_tick_is_reported_and_exempt_ticks_do_not_count() {
    let mut guard = MoveGuard::new();
    let mut reports = Vec::new();
    let tick = |guard: &mut MoveGuard, reports: &mut Vec<Report>| {
        guard.serverbound(
            &checked(Serverbound::ClientTickEnd),
            ms(0),
            Acknowledged::default(),
            reports,
        )
    };
    for _ in 0..30 {
        tick(&mut guard, &mut reports);
    }
    guard.serverbound(
        &checked(Serverbound::PlayerLoaded),
        ms(0),
        Acknowledged::default(),
        &mut reports,
    );
    guard.teleport_sent(7);
    for _ in 0..30 {
        tick(&mut guard, &mut reports);
    }
    let accept = Serverbound::Accept(Accept {
        teleport_id: 7,
        ..Accept::default()
    });
    guard.serverbound(
        &checked(accept),
        ms(0),
        Acknowledged::default(),
        &mut reports,
    );
    for _ in 0..19 {
        tick(&mut guard, &mut reports);
    }
    assert!(reports.is_empty(), "{reports:?}");
    tick(&mut guard, &mut reports);
    assert_eq!(
        reports
            .iter()
            .map(|report| report.finding)
            .collect::<Vec<_>>(),
        [Finding::SilentTicks]
    );
}

#[test]
fn player_input_is_dropped_until_the_client_has_loaded() {
    let mut guard = MoveGuard::new();
    let mut reports = Vec::new();
    let input = checked(Serverbound::PlayerInput);
    assert_eq!(
        guard.serverbound(&input, ms(0), Acknowledged::default(), &mut reports),
        Action::Drop
    );
    guard.serverbound(
        &checked(Serverbound::PlayerLoaded),
        ms(0),
        Acknowledged::default(),
        &mut reports,
    );
    assert_eq!(
        guard.serverbound(&input, ms(0), Acknowledged::default(), &mut reports),
        Action::Forward
    );
}
