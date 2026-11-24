use std::time::Duration;

use glam::DVec3;
use mcrs_minecraft_move_validate::{
    Accept, Acknowledged, Action, Checked, Finding, Look, Move, MoveGuard, Report, Reports,
    Serverbound,
};
use proptest::prelude::*;

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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

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

#[test]
fn an_answer_that_reaches_the_guard_is_reported_and_dropped() {
    let mut guard = MoveGuard::new();
    let mut reports = Vec::new();
    let action = guard.serverbound(
        &checked(Serverbound::Pong(7)),
        ms(0),
        Acknowledged::default(),
        &mut reports,
    );
    assert_eq!(action, Action::Drop, "MVC-LAT-011");
    assert_eq!(
        reports
            .iter()
            .map(|report| report.finding)
            .collect::<Vec<_>>(),
        [Finding::UnknownAnswer]
    );
}
