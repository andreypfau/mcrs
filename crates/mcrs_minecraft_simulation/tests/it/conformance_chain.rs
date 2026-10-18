use crate::probe::*;
use mcrs_minecraft_core::{BlockPos, BlockStateId, Direction};
use mcrs_minecraft_simulation::{
    BlockUpdateFlags, ChainLedger, ChainLimit, Destroyed, Message, MessageBody, PartitionKind,
    Phase, StepError, StepOutput, StepReport, UpdateRequest,
};
use mcrs_minecraft_volume::Access;

fn neighbour_hits(log: &[Entry]) -> Vec<BlockPos> {
    neighbour_entries(log)
        .into_iter()
        .map(|(pos, _, _)| pos)
        .collect()
}

fn run_set(host: Host, rules: Vec<Rule>, pos: BlockPos, state: BlockStateId, flags: u32) -> Runner {
    let mut runner = Runner::new(host, rules);
    runner
        .step(100, Phase::Ingress, &[set_message(0, pos, state, flags)])
        .expect("owned message");
    runner
}

fn update_messages(out: &StepOutput<ProbeKinds>) -> Vec<Message<ProbeKinds>> {
    out.outbound
        .iter()
        .filter(|message| matches!(message.body, MessageBody::Update(_)))
        .copied()
        .collect()
}

fn update_requests(messages: &[Message<ProbeKinds>]) -> Vec<UpdateRequest<ProbeKinds>> {
    messages
        .iter()
        .filter_map(|message| match message.body {
            MessageBody::Update(request) => Some(request),
            _ => None,
        })
        .collect()
}

fn check_sim_nbr_001(
    entries: &[(BlockPos, BlockStateId, ProbeBlock)],
    p: BlockPos,
) -> Result<(), String> {
    let expected = around(p, &NEIGHBOUR_ORDER);
    let hits: Vec<BlockPos> = entries.iter().map(|(pos, _, _)| *pos).collect();
    if hits != expected {
        return Err(format!(
            "neighbour updates ran at {hits:?}, expected {expected:?}"
        ));
    }
    match entries.iter().find(|(_, _, source)| *source != BLOCK_A) {
        Some((pos, _, source)) => Err(format!("update at {pos:?} carried source {source:?}")),
        None => Ok(()),
    }
}

#[test]
fn sim_nbr_001_a_set_updates_its_six_neighbours_in_neighbour_order() {
    let p = at(8, 8, 8);
    let runner = run_set(filled(&[(0, 0, 0)]), Vec::new(), p, B, 1);
    let entries = neighbour_entries(&runner.behaviour.log());
    assert_eq!(check_sim_nbr_001(&entries, p), Ok(()));

    let in_shape_order: Vec<_> = around(p, &SHAPE_ORDER)
        .into_iter()
        .map(|pos| (pos, A, BLOCK_A))
        .collect();
    assert!(check_sim_nbr_001(&in_shape_order, p).is_err());
}

fn check_sim_nbr_002(
    entries: &[(BlockPos, BlockStateId, ProbeBlock)],
    q: BlockPos,
) -> Result<(), String> {
    let seen: Vec<BlockStateId> = entries
        .iter()
        .filter(|(pos, _, _)| *pos == q)
        .map(|(_, state, _)| *state)
        .collect();
    if seen == [A] {
        Ok(())
    } else {
        Err(format!(
            "handler at {q:?} ran with states {seen:?}, expected the captured {A:?}"
        ))
    }
}

#[test]
fn sim_nbr_002_a_single_update_with_state_runs_the_captured_state() {
    let x = at(8, 8, 8);
    let q = at(2, 8, 2);
    let rules = vec![
        neighbour_rule(
            offset(x, Direction::West),
            vec![
                Action::UpdateNeighbourWithState {
                    target: q,
                    state: A,
                    source: BLOCK_A,
                },
                Action::Set {
                    pos: q,
                    state: C,
                    flags: 0,
                    limit: 512,
                },
            ],
        )
        .once(),
    ];
    let runner = run_set(filled(&[(0, 0, 0)]), rules, x, B, 1);
    assert_eq!(runner.host.state(q), C);
    let entries = neighbour_entries(&runner.behaviour.log());
    assert_eq!(check_sim_nbr_002(&entries, q), Ok(()));

    let current: Vec<_> = entries
        .iter()
        .map(|(pos, state, source)| (*pos, if *pos == q { C } else { *state }, *source))
        .collect();
    assert!(check_sim_nbr_002(&current, q).is_err());
}

fn check_sim_nbr_003_depth_first(
    hits: &[BlockPos],
    p: BlockPos,
    z: BlockPos,
) -> Result<(), String> {
    let side = |d| offset(p, d);
    let mut expected = vec![side(Direction::West)];
    expected.extend(around(z, &NEIGHBOUR_ORDER));
    expected.extend(
        [
            Direction::East,
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
        ]
        .map(side),
    );
    if hits == expected {
        Ok(())
    } else {
        Err(format!(
            "neighbour updates ran at {hits:?}, expected {expected:?}"
        ))
    }
}

fn nested_set_rule(p: BlockPos, z: BlockPos, after: Vec<Action>) -> Rule {
    let mut actions = vec![Action::Set {
        pos: z,
        state: B,
        flags: 17,
        limit: 512,
    }];
    actions.extend(after);
    neighbour_rule(offset(p, Direction::West), actions).once()
}

#[test]
fn sim_nbr_003_a_nested_set_runs_before_the_next_sibling() {
    let p = at(8, 8, 8);
    let z = at(6, 8, 8);
    let runner = run_set(
        filled(&[(0, 0, 0)]),
        vec![nested_set_rule(p, z, Vec::new())],
        p,
        B,
        1,
    );
    let hits = neighbour_hits(&runner.behaviour.log());
    assert_eq!(check_sim_nbr_003_depth_first(&hits, p, z), Ok(()));

    let mut breadth_first = vec![offset(p, Direction::West)];
    breadth_first.extend(
        [
            Direction::East,
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
        ]
        .map(|d| offset(p, d)),
    );
    breadth_first.extend(around(z, &NEIGHBOUR_ORDER));
    assert!(check_sim_nbr_003_depth_first(&breadth_first, p, z).is_err());
}

fn index_of(log: &[Entry], wanted: Entry) -> Option<usize> {
    log.iter().position(|entry| *entry == wanted)
}

fn first_update_at(log: &[Entry], pos: BlockPos) -> Option<usize> {
    log.iter()
        .position(|entry| matches!(entry, Entry::Neighbour { pos: at, .. } if *at == pos))
}

fn check_sim_nbr_003_handler_first(log: &[Entry], first_nested: BlockPos) -> Result<(), String> {
    let mark = index_of(log, Entry::Mark(1)).ok_or("the handler never logged after its set")?;
    let nested =
        first_update_at(log, first_nested).ok_or("the nested set made no neighbour update")?;
    if mark < nested {
        Ok(())
    } else {
        Err(format!(
            "the handler's own line came at {mark}, after the nested set's first update at {nested}"
        ))
    }
}

#[test]
fn sim_nbr_003_a_handler_finishes_before_its_requests_run() {
    let p = at(8, 8, 8);
    let z = at(6, 8, 8);
    let rules = vec![nested_set_rule(p, z, vec![Action::Mark(1)])];
    let runner = run_set(filled(&[(0, 0, 0)]), rules, p, B, 1);
    let log = runner.behaviour.log();
    let first_nested = offset(z, Direction::West);
    assert_eq!(check_sim_nbr_003_handler_first(&log, first_nested), Ok(()));

    let mut recursive = log.clone();
    if let Some(mark) = index_of(&recursive, Entry::Mark(1)) {
        let line = recursive.remove(mark);
        recursive.push(line);
    }
    assert!(check_sim_nbr_003_handler_first(&recursive, first_nested).is_err());
}

fn check_sim_nbr_003_returns_last(log: &[Entry]) -> Result<(), String> {
    let returned = index_of(log, Entry::Mark(2)).ok_or("the tick never logged after its set")?;
    let last_update = log
        .iter()
        .rposition(|entry| matches!(entry, Entry::Neighbour { .. }))
        .ok_or("the set made no neighbour update")?;
    if returned > last_update {
        Ok(())
    } else {
        Err(format!(
            "returned at {returned}, before the update at {last_update}"
        ))
    }
}

#[test]
fn sim_nbr_003_a_set_outside_a_chain_returns_after_its_whole_chain() {
    let (p, tick) = (at(8, 8, 8), at(12, 8, 12));
    let rules = vec![
        Rule::on(
            Hook::BlockTick,
            tick,
            vec![
                Action::Set {
                    pos: p,
                    state: B,
                    flags: 1,
                    limit: 512,
                },
                Action::Mark(2),
            ],
        )
        .once(),
    ];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    let trigger = [block_tick_message(0, tick, BLOCK_A, 100)];
    runner
        .step(100, Phase::BlockTicks, &trigger)
        .expect("owned message");
    let log = runner.behaviour.log();
    assert_eq!(check_sim_nbr_003_returns_last(&log), Ok(()));

    let mut early = log.clone();
    if let Some(mark) = index_of(&early, Entry::Mark(2)) {
        let line = early.remove(mark);
        early.insert(0, line);
    }
    assert!(check_sim_nbr_003_returns_last(&early).is_err());
}

const CHAIN_LENGTH: i32 = 1000;

fn check_sim_nbr_003_nesting(deepest: u32, handled: usize, length: usize) -> Result<(), String> {
    if handled != length {
        return Err(format!("{handled} of {length} requests ran"));
    }
    if deepest == 1 {
        Ok(())
    } else {
        Err(format!(
            "handlers nested {deepest} deep over a chain of {length}"
        ))
    }
}

#[test]
fn sim_nbr_003_handler_nesting_does_not_grow_with_the_chain() {
    let mut host = Host::tall();
    for x in 0..=(CHAIN_LENGTH / 16) {
        host.insert(section(x, 0, 0), A);
    }
    let link = |i: i32| at(i, 0, 0);
    let tick = at(0, 4, 0);
    let mut rules = vec![
        Rule::on(
            Hook::BlockTick,
            tick,
            vec![Action::UpdateNeighbour {
                target: link(0),
                source: BLOCK_A,
            }],
        )
        .once(),
    ];
    for i in 0..CHAIN_LENGTH - 1 {
        rules.push(neighbour_rule(
            link(i),
            vec![Action::UpdateNeighbour {
                target: link(i + 1),
                source: BLOCK_A,
            }],
        ));
    }
    let mut runner = Runner::new(host, rules);
    let trigger = [block_tick_message(0, tick, BLOCK_A, 100)];
    runner
        .step(100, Phase::BlockTicks, &trigger)
        .expect("owned message");
    let handled = neighbour_entries(&runner.behaviour.log()).len();
    let length = usize::try_from(CHAIN_LENGTH).expect("positive length");
    assert_eq!(
        check_sim_nbr_003_nesting(runner.behaviour.deepest_nesting(), handled, length),
        Ok(())
    );
    assert!(check_sim_nbr_003_nesting(CHAIN_LENGTH.unsigned_abs(), handled, length).is_err());
}

struct Neighbourhood {
    centre: BlockPos,
    inert: [BlockPos; 6],
}

impl Neighbourhood {
    fn new(centre: BlockPos, row: i32) -> Self {
        let inert = [0, 1, 2, 3, 4, 5].map(|i| at(1 + i, row, 1));
        Self { centre, inert }
    }

    fn rules(&self) -> Vec<Rule> {
        NEIGHBOUR_ORDER
            .iter()
            .zip(self.inert)
            .map(|(d, target)| {
                neighbour_rule(
                    offset(self.centre, *d),
                    vec![Action::UpdateNeighbour {
                        target,
                        source: BLOCK_A,
                    }],
                )
            })
            .collect()
    }

    fn expected_hits(&self, limit: i32) -> Vec<BlockPos> {
        if limit == 0 {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for (index, (d, target)) in NEIGHBOUR_ORDER.iter().zip(self.inert).enumerate() {
            hits.push(offset(self.centre, *d));
            let offered_before = i64::try_from(index).unwrap_or(i64::MAX) + 1;
            if limit < 0 || offered_before < i64::from(limit) {
                hits.push(target);
            }
        }
        hits
    }

    fn expected_overflows(&self, limit: i32) -> u32 {
        match limit {
            0 => 7,
            1..=6 => 1,
            _ => 0,
        }
    }

    fn expected_first(&self, limit: i32) -> Option<BlockPos> {
        match limit {
            0 => Some(self.centre),
            1..=6 => self
                .inert
                .get(usize::try_from(limit - 1).unwrap_or(0))
                .copied(),
            _ => None,
        }
    }
}

fn check_sim_nbr_004(
    limit: i32,
    sets: &[Neighbourhood],
    hits: &[BlockPos],
    report: &StepReport,
) -> Result<(), String> {
    let expected: Vec<BlockPos> = sets
        .iter()
        .flat_map(|set| set.expected_hits(limit))
        .collect();
    if hits != expected {
        return Err(format!(
            "with M = {limit}, updates ran at {hits:?}, expected {expected:?}"
        ));
    }
    let overflows: u32 = sets.iter().map(|set| set.expected_overflows(limit)).sum();
    if report.chain_overflows != overflows {
        return Err(format!(
            "with M = {limit}, {} chains overflowed, expected {overflows}",
            report.chain_overflows
        ));
    }
    let first = sets.iter().find_map(|set| set.expected_first(limit));
    if report.first_chain_overflow == first {
        Ok(())
    } else {
        Err(format!(
            "with M = {limit}, the first overflow named {:?}, expected {first:?}",
            report.first_chain_overflow
        ))
    }
}

fn run_limited(limit: i32, sets: &[&Neighbourhood]) -> (Vec<BlockPos>, StepReport) {
    let rules = sets.iter().flat_map(|set| set.rules()).collect();
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    runner.config.chain_limit = ChainLimit::from_property(limit);
    let inbound: Vec<_> = sets
        .iter()
        .zip(0u32..)
        .map(|(set, counter)| set_message(counter, set.centre, B, 1))
        .collect();
    let out = runner
        .step(100, Phase::Ingress, &inbound)
        .expect("owned messages");
    (neighbour_hits(&runner.behaviour.log()), out.report)
}

#[test]
fn sim_nbr_004_the_chain_accepts_only_the_first_m_requests() {
    let first = Neighbourhood::new(at(8, 8, 8), 2);
    let second = Neighbourhood::new(at(8, 8, 12), 3);

    for limit in [3, 0, -1] {
        let (hits, report) = run_limited(limit, &[&first]);
        assert_eq!(
            check_sim_nbr_004(
                limit,
                &[Neighbourhood::new(first.centre, 2)],
                &hits,
                &report
            ),
            Ok(())
        );
    }

    let (hits, report) = run_limited(3, &[&first]);
    let aborted: Vec<BlockPos> = hits
        .iter()
        .copied()
        .take_while(|pos| *pos != offset(first.centre, Direction::Down))
        .collect();
    assert!(
        check_sim_nbr_004(3, &[Neighbourhood::new(first.centre, 2)], &aborted, &report).is_err()
    );

    let (hits, report) = run_limited(3, &[&first, &second]);
    let both = [
        Neighbourhood::new(first.centre, 2),
        Neighbourhood::new(second.centre, 3),
    ];
    assert_eq!(check_sim_nbr_004(3, &both, &hits, &report), Ok(()));

    let shared_budget: Vec<BlockPos> = both
        .first()
        .map(|set| set.expected_hits(3))
        .unwrap_or_default();
    let one_overflow = StepReport {
        chain_overflows: 1,
        ..report
    };
    assert!(check_sim_nbr_004(3, &both, &shared_budget, &one_overflow).is_err());
}

fn check_sim_shp_001_order(
    entries: &[(BlockPos, Direction, BlockPos, BlockStateId)],
    p: BlockPos,
    state: BlockStateId,
) -> Result<(), String> {
    let expected: Vec<_> = SHAPE_ORDER
        .iter()
        .map(|d| (offset(p, *d), d.opposite(), p, state))
        .collect();
    if entries == expected {
        Ok(())
    } else {
        Err(format!(
            "shape updates were {entries:?}, expected {expected:?}"
        ))
    }
}

#[test]
fn sim_shp_001_a_set_sends_six_shape_updates_in_shape_order() {
    let p = at(8, 8, 8);
    let runner = run_set(filled(&[(0, 0, 0)]), Vec::new(), p, B, 2);
    let entries = shape_entries(&runner.behaviour.log());
    assert_eq!(check_sim_shp_001_order(&entries, p, B), Ok(()));

    let in_neighbour_order: Vec<_> = NEIGHBOUR_ORDER
        .iter()
        .map(|d| (offset(p, *d), d.opposite(), p, B))
        .collect();
    assert!(check_sim_shp_001_order(&in_neighbour_order, p, B).is_err());
}

fn check_sim_shp_001_support(
    flags: u32,
    destroyed: &[Destroyed],
    above: BlockPos,
    final_state: BlockStateId,
) -> Result<(), String> {
    let loses_support = flags & 16 == 0;
    let expected_destroyed: &[Destroyed] = if loses_support {
        &[Destroyed {
            pos: above,
            state: NEEDS_SUPPORT,
            drops: true,
        }]
    } else {
        &[]
    };
    let expected_state = if loses_support { AIR } else { NEEDS_SUPPORT };
    if destroyed != expected_destroyed {
        return Err(format!(
            "with flags {flags}, destroyed {destroyed:?}, expected {expected_destroyed:?}"
        ));
    }
    if final_state == expected_state {
        Ok(())
    } else {
        Err(format!(
            "with flags {flags}, the block ended as {final_state:?}"
        ))
    }
}

#[test]
fn sim_shp_001_a_block_losing_its_support_is_destroyed_unless_keep_shape() {
    let p = at(8, 8, 8);
    let above = offset(p, Direction::Up);
    let run = |flags| {
        let mut host = filled(&[(0, 0, 0)]);
        host.put(above, NEEDS_SUPPORT);
        let mut runner = Runner::new(host, Vec::new());
        let out = runner
            .step(100, Phase::Ingress, &[set_message(0, p, AIR, flags)])
            .expect("owned message");
        (out.destroyed, runner.host.state(above))
    };
    for flags in [3, 18] {
        let (destroyed, state) = run(flags);
        assert_eq!(
            check_sim_shp_001_support(flags, &destroyed, above, state),
            Ok(())
        );
    }
    let (destroyed, state) = run(18);
    assert!(check_sim_shp_001_support(3, &destroyed, above, state).is_err());
}

fn check_sim_shp_001_wire(
    entries: &[(BlockPos, Direction, BlockPos, BlockStateId)],
    p: BlockPos,
    wire: BlockPos,
    flags: u32,
) -> Result<(), String> {
    let updated = entries.iter().any(|(pos, ..)| *pos == wire);
    let others = entries.iter().filter(|(pos, ..)| *pos != wire).count();
    if others != 5 {
        return Err(format!(
            "{others} blocks other than the wire at {wire:?} were updated"
        ));
    }
    match (flags & 128 != 0, updated) {
        (true, true) => Err(format!("with bit 128, the wire next to {p:?} was updated")),
        (false, false) => Err(format!(
            "without bit 128, the wire next to {p:?} was skipped"
        )),
        _ => Ok(()),
    }
}

#[test]
fn sim_shp_001_a_shape_update_carrying_bit_128_skips_a_wire() {
    let p = at(8, 8, 8);
    let wire = offset(p, Direction::West);
    let run = |flags| {
        let mut host = filled(&[(0, 0, 0)]);
        host.put(wire, WIRE);
        let mut runner = Runner::new(host, Vec::new());
        runner
            .step(100, Phase::Ingress, &[set_message(0, p, B, flags)])
            .expect("owned message");
        shape_entries(&runner.behaviour.log())
    };
    assert_eq!(check_sim_shp_001_wire(&run(130), p, wire, 130), Ok(()));
    assert_eq!(check_sim_shp_001_wire(&run(2), p, wire, 2), Ok(()));
    assert!(check_sim_shp_001_wire(&run(2), p, wire, 130).is_err());
}

const STACK_HEIGHT: i32 = 600;

fn stack_host() -> Host {
    let mut host = Host::tall();
    for y in 0..=(STACK_HEIGHT / 16) {
        host.insert(section(0, y, 0), AIR);
    }
    for y in 0..STACK_HEIGHT {
        host.put(at(0, y, 0), STACK);
    }
    host
}

fn changed_heights(host: &Host, from: i32, to: i32) -> Vec<i32> {
    (from..to)
        .filter(|y| host.state(at(0, *y, 0)) == STACK_CHANGED)
        .collect()
}

fn check_sim_shp_002(changed: &[i32]) -> Result<(), String> {
    let expected: Vec<i32> = (1..=512).collect();
    if changed == expected {
        Ok(())
    } else {
        Err(format!(
            "{} blocks changed, from {:?} to {:?}; expected the 512 above the bottom",
            changed.len(),
            changed.first(),
            changed.last()
        ))
    }
}

#[test]
fn sim_shp_002_a_shape_cascade_stops_after_512_generations() {
    let mut runner = Runner::new(stack_host(), Vec::new());
    runner
        .step(100, Phase::Ingress, &[set_message(0, at(0, 0, 0), B, 2)])
        .expect("owned message");
    let changed = changed_heights(&runner.host, 1, STACK_HEIGHT);
    assert_eq!(check_sim_shp_002(&changed), Ok(()));

    for twin in [513, STACK_HEIGHT - 1, 15] {
        let heights: Vec<i32> = (1..=twin).collect();
        assert!(check_sim_shp_002(&heights).is_err());
    }
}

fn check_sim_shp_004(
    entries: &[(BlockPos, Direction, BlockPos, BlockStateId)],
    target: BlockPos,
) -> Result<(), String> {
    let seen: Vec<BlockStateId> = entries
        .iter()
        .filter(|(pos, ..)| *pos == target)
        .map(|(.., state)| *state)
        .collect();
    if seen == [B] {
        Ok(())
    } else {
        Err(format!(
            "the update at {target:?} carried neighbour states {seen:?}, expected the captured {B:?}"
        ))
    }
}

#[test]
fn sim_shp_004_a_shape_update_passes_the_neighbour_state_captured_at_request() {
    let (x, p) = (at(8, 8, 8), at(12, 8, 12));
    let rules = vec![
        neighbour_rule(
            offset(x, Direction::West),
            vec![
                Action::Set {
                    pos: p,
                    state: B,
                    flags: 2,
                    limit: 512,
                },
                Action::Set {
                    pos: p,
                    state: C,
                    flags: 18,
                    limit: 512,
                },
            ],
        )
        .once(),
    ];
    let runner = run_set(filled(&[(0, 0, 0)]), rules, x, B, 1);
    assert_eq!(runner.host.state(p), C);
    let entries = shape_entries(&runner.behaviour.log());
    let target = offset(p, Direction::West);
    assert_eq!(check_sim_shp_004(&entries, target), Ok(()));

    let current: Vec<_> = entries
        .iter()
        .map(|(pos, from, neighbour, state)| {
            (
                *pos,
                *from,
                *neighbour,
                if *pos == target { C } else { *state },
            )
        })
        .collect();
    assert!(check_sim_shp_004(&current, target).is_err());
}

fn check_sim_model_103_limit(
    requests: &[UpdateRequest<ProbeKinds>],
    target: BlockPos,
    limit: u16,
) -> Result<(), String> {
    match requests {
        [
            UpdateRequest::Shape {
                target: at,
                from: Direction::Down,
                limit: carried,
                ..
            },
        ] if *at == target && *carried == limit => Ok(()),
        other => Err(format!(
            "expected one shape update into {target:?} with limit {limit}, got {other:?}"
        )),
    }
}

fn check_sim_model_103_cascade(changed: &[i32], first: i32, limit: u16) -> Result<(), String> {
    let expected: Vec<i32> = (first..=first + i32::from(limit)).collect();
    if changed == expected {
        Ok(())
    } else {
        Err(format!(
            "blocks at heights {changed:?} changed, expected {expected:?}"
        ))
    }
}

#[test]
fn sim_model_103_the_update_limit_travels_inside_a_shape_message() {
    let p = at(8, 15, 8);
    let above = offset(p, Direction::Up);
    let mut sender_host = filled(&[(0, 0, 0)]);
    sender_host.insert(section(0, 1, 0), A);
    sender_host.set_access(section(0, 1, 0), Access::Read);
    let mut sender = Runner::new(sender_host, Vec::new());
    let sent = sender
        .step(100, Phase::Ingress, &[limited_set_message(0, p, B, 2, 5)])
        .expect("owned message");
    let messages = update_messages(&sent);
    assert_eq!(
        check_sim_model_103_limit(&update_requests(&messages), above, 4),
        Ok(())
    );

    let mut receiver_host = Host::new();
    receiver_host.insert(section(0, 0, 0), A);
    receiver_host.set_access(section(0, 0, 0), Access::Read);
    for y in 1..=3 {
        receiver_host.insert(section(0, y, 0), AIR);
    }
    for y in 16..56 {
        receiver_host.put(at(8, y, 8), STACK);
    }
    let mut receiver = Runner::new(receiver_host, Vec::new());
    receiver
        .step_with(101, Phase::Ingress, &messages, &sent.chains)
        .expect("owned message");
    let changed: Vec<i32> = (16..56)
        .filter(|y| receiver.host.state(at(8, *y, 8)) == STACK_CHANGED)
        .collect();
    assert_eq!(check_sim_model_103_cascade(&changed, 16, 4), Ok(()));

    let whole_stack: Vec<i32> = (16..56).collect();
    assert!(check_sim_model_103_cascade(&whole_stack, 16, 4).is_err());
    let reset = [UpdateRequest::Shape {
        target: above,
        from: Direction::Down,
        neighbour_state: B,
        flags: BlockUpdateFlags::from_bits_retain(2),
        limit: 512,
    }];
    assert!(check_sim_model_103_limit(&reset, above, 4).is_err());
}

fn check_sim_model_103_count(
    log: &[Entry],
    report: &StepReport,
    served: [BlockPos; 2],
    dropped: BlockPos,
) -> Result<(), String> {
    let hits = neighbour_hits(log);
    for pos in served {
        if !hits.contains(&pos) {
            return Err(format!("the continuation at {pos:?} never ran"));
        }
    }
    if hits.contains(&dropped) {
        return Err(format!(
            "the request for {dropped:?} ran past the chain limit"
        ));
    }
    if report.chain_overflows == 1 && report.first_chain_overflow == Some(dropped) {
        Ok(())
    } else {
        Err(format!(
            "the overflow was {} at {:?}",
            report.chain_overflows, report.first_chain_overflow
        ))
    }
}

#[test]
fn sim_model_103_a_chain_count_travels_with_its_continuations() {
    let p = at(8, 8, 8);
    let remote = [at(2, 8, 20), at(3, 8, 20), at(4, 8, 20)];
    let sender_rules = [Direction::West, Direction::East, Direction::Down]
        .iter()
        .zip(remote)
        .map(|(d, target)| {
            neighbour_rule(
                offset(p, *d),
                vec![Action::UpdateNeighbour {
                    target,
                    source: BLOCK_A,
                }],
            )
        })
        .collect();
    let mut sender_host = filled(&[(0, 0, 0), (0, 0, 1)]);
    sender_host.set_access(section(0, 0, 1), Access::Read);
    let mut sender = Runner::new(sender_host, sender_rules);
    sender.config.chain_limit = ChainLimit::from_property(3);
    let sent = sender
        .step(100, Phase::Ingress, &[set_message(0, p, B, 1)])
        .expect("owned message");
    let messages = update_messages(&sent);
    assert_eq!(messages.len(), 2);
    let origin = messages.first().map(|message| message.origin);
    assert_eq!(
        sent.chains.as_slice(),
        origin
            .map(|origin| ChainLedger { origin, offered: 4 })
            .as_slice()
    );

    let further = at(8, 8, 24);
    let receive = |ledgers: &[ChainLedger]| {
        let mut host = filled(&[(0, 0, 0), (0, 0, 1)]);
        host.set_access(section(0, 0, 0), Access::Read);
        let rules = vec![neighbour_rule(
            remote[0],
            vec![Action::UpdateNeighbour {
                target: further,
                source: BLOCK_A,
            }],
        )];
        let mut receiver = Runner::new(host, rules);
        receiver.config.chain_limit = ChainLimit::from_property(3);
        let out = receiver
            .step_with(101, Phase::Ingress, &messages, ledgers)
            .expect("owned messages");
        (receiver.behaviour.log(), out.report)
    };
    let served = [remote[0], remote[1]];
    let (log, report) = receive(&sent.chains);
    assert_eq!(
        check_sim_model_103_count(&log, &report, served, further),
        Ok(())
    );

    let (log, report) = receive(&[]);
    assert!(check_sim_model_103_count(&log, &report, served, further).is_err());
}

fn check_sim_model_007_update(
    sender_hits: &[BlockPos],
    sent: &[UpdateRequest<ProbeKinds>],
    receiver_hits: &[BlockPos],
    target: BlockPos,
) -> Result<(), String> {
    if sender_hits.contains(&target) {
        return Err(format!("the writer updated {target:?} locally"));
    }
    let singles: Vec<_> = sent
        .iter()
        .filter(|request| matches!(request, UpdateRequest::Single { target: at, block } if *at == target && *block == BLOCK_A))
        .collect();
    if singles.len() != 1 {
        return Err(format!(
            "{} single updates for {target:?} left",
            singles.len()
        ));
    }
    let runs = receiver_hits.iter().filter(|pos| **pos == target).count();
    if runs == 1 {
        Ok(())
    } else {
        Err(format!(
            "the update for {target:?} ran {runs} times at its writer"
        ))
    }
}

#[test]
fn sim_model_007_a_neighbour_update_into_a_read_section_leaves_as_a_single_update() {
    let p = at(8, 8, 15);
    let target = offset(p, Direction::South);
    let mut sender_host = filled(&[(0, 0, 0), (0, 0, 1)]);
    sender_host.set_access(section(0, 0, 1), Access::Read);
    let mut sender = Runner::new(sender_host, Vec::new());
    let sent = sender
        .step(100, Phase::Ingress, &[set_message(0, p, B, 1)])
        .expect("owned message");
    let messages = update_messages(&sent);
    let sender_hits = neighbour_hits(&sender.behaviour.log());

    let mut receiver_host = filled(&[(0, 0, 0), (0, 0, 1)]);
    receiver_host.set_access(section(0, 0, 0), Access::Read);
    let mut receiver = Runner::new(receiver_host, Vec::new());
    receiver
        .step_with(101, Phase::Ingress, &messages, &sent.chains)
        .expect("owned messages");
    let receiver_hits = neighbour_hits(&receiver.behaviour.log());
    let requests = update_requests(&messages);
    assert_eq!(
        check_sim_model_007_update(&sender_hits, &requests, &receiver_hits, target),
        Ok(())
    );

    let mut local = sender_hits.clone();
    local.push(target);
    assert!(check_sim_model_007_update(&local, &requests, &receiver_hits, target).is_err());
    assert!(check_sim_model_007_update(&sender_hits, &requests, &[], target).is_err());
}

#[test]
fn a_cell_continues_a_chain_that_spills_into_it_and_refuses_queued_work() {
    let target = at(8, 8, 8);
    let spill = Message {
        stamp: stamp(1, 0),
        origin: stamp(1, 0),
        body: MessageBody::Update(UpdateRequest::Single {
            target,
            block: BLOCK_A,
        }),
    };
    let mut cell = Runner::new(filled(&[(0, 0, 0)]), Vec::new());
    cell.kind = PartitionKind::Cell;
    cell.step(100, Phase::Ingress, &[spill])
        .expect("a cell takes chain continuations");
    assert_eq!(neighbour_hits(&cell.behaviour.log()), [target]);

    let tick = block_tick_message(1, target, BLOCK_A, 100);
    assert_eq!(
        cell.step(100, Phase::Ingress, &[tick]),
        Err(StepError::NoQueues {
            section: section(0, 0, 0)
        })
    );
}
