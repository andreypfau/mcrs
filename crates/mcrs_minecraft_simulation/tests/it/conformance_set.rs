use crate::probe::*;
use mcrs_minecraft_core::{BlockPos, BlockStateId, Direction};
use mcrs_minecraft_simulation::{BlockUpdateFlags, MessageBody, Phase, StepOutput};

fn run_set(host: Host, rules: Vec<Rule>, pos: BlockPos, state: BlockStateId, flags: u32) -> Runner {
    run_set_out(host, rules, pos, state, flags).0
}

fn run_set_out(
    host: Host,
    rules: Vec<Rule>,
    pos: BlockPos,
    state: BlockStateId,
    flags: u32,
) -> (Runner, StepOutput<ProbeKinds>) {
    let mut runner = Runner::new(host, rules);
    let out = runner
        .step(100, Phase::Ingress, &[set_message(0, pos, state, flags)])
        .expect("owned message");
    (runner, out)
}

type Row = (u32, usize, usize, bool, bool);
type Observed = (usize, usize, bool, bool);

const ROWS: [Row; 9] = [
    (3, 6, 6, true, true),
    (2, 0, 6, true, true),
    (18, 0, 0, true, true),
    (1, 6, 6, false, true),
    (515, 6, 6, true, false),
    (816, 0, 0, false, false),
    (11, 6, 6, true, true),
    (35, 6, 6, true, true),
    (260, 0, 6, false, true),
];

fn check_sim_set_001(row: Row, observed: Observed) -> Result<(), String> {
    let (flags, neighbours, shapes, marked, placed) = row;
    let (seen_neighbours, seen_shapes, seen_marked, seen_placed) = observed;
    let mut wrong = Vec::new();
    if seen_neighbours != neighbours {
        wrong.push(format!(
            "{seen_neighbours} neighbour updates, expected {neighbours}"
        ));
    }
    if seen_shapes != shapes {
        wrong.push(format!("{seen_shapes} shape updates, expected {shapes}"));
    }
    if seen_marked != marked {
        wrong.push(format!("marked {seen_marked}, expected {marked}"));
    }
    if seen_placed != placed {
        wrong.push(format!("placed {seen_placed}, expected {placed}"));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!("flags {flags}: {}", wrong.join("; ")))
    }
}

fn observe_set(flags: u32) -> Observed {
    let p = at(8, 8, 8);
    let message = inbound(
        stamp(1, 0),
        MessageBody::Set {
            pos: p,
            state: B,
            flags: BlockUpdateFlags::from_bits_retain(flags),
            limit: 512,
        },
    );
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), Vec::new());
    let out = runner
        .step(100, Phase::Ingress, &[message])
        .expect("owned message");
    let log = runner.behaviour.log();
    let placed = log
        .iter()
        .any(|entry| matches!(entry, Entry::Placed { pos, new, .. } if *pos == p && *new == B));
    (
        neighbour_entries(&log).len(),
        shape_entries(&log).len(),
        out.marked.contains(&p),
        placed,
    )
}

#[test]
fn sim_set_001_each_flag_word_has_its_effects() {
    for row in ROWS {
        assert_eq!(check_sim_set_001(row, observe_set(row.0)), Ok(()));
    }

    let ignores_bit_0 = ROWS.iter().find(|row| row.0 == 2).copied();
    assert!(ignores_bit_0.is_some_and(|row| check_sim_set_001(row, (6, 6, true, true)).is_err()));
}

fn describe(entry: &Entry) -> String {
    match entry {
        Entry::Removed { pos, old, .. } => format!("removal at {pos:?} of {old:?}"),
        Entry::Placed { pos, new, .. } => format!("placement at {pos:?} of {new:?}"),
        Entry::Neighbour { pos, source, .. } => {
            format!("direct neighbour update at {pos:?} with source {source:?}")
        }
        Entry::Shape {
            pos,
            neighbour_state,
            ..
        } => format!("direct shape update at {pos:?} carrying {neighbour_state:?}"),
        other => format!("{other:?}"),
    }
}

fn step_three_entries(log: &[Entry]) -> Vec<Entry> {
    log.iter()
        .filter(|entry| {
            matches!(
                entry,
                Entry::Removed { .. }
                    | Entry::Placed { .. }
                    | Entry::Neighbour { .. }
                    | Entry::Shape { .. }
            )
        })
        .copied()
        .collect()
}

fn direct_neighbour(pos: BlockPos, source: ProbeBlock) -> Entry {
    Entry::Neighbour {
        pos,
        state: A,
        source,
        moved_by_piston: false,
    }
}

fn direct_shape(p: BlockPos, direction: Direction, carried: BlockStateId) -> Entry {
    Entry::Shape {
        pos: offset(p, direction),
        state: A,
        from: direction.opposite(),
        neighbour: p,
        neighbour_state: carried,
    }
}

fn first_difference(seen: &[Entry], expected: &[Entry]) -> Result<(), String> {
    for index in 0..seen.len().max(expected.len()) {
        match (seen.get(index), expected.get(index)) {
            (Some(found), Some(wanted)) if found == wanted => {}
            (found, wanted) => {
                return Err(format!(
                    "found {}, expected {}",
                    found.map_or("nothing".to_owned(), describe),
                    wanted.map_or("nothing".to_owned(), describe)
                ));
            }
        }
    }
    Ok(())
}

fn check_sim_set_002_direct_updates_after_a_replacement(
    log: &[Entry],
    p: BlockPos,
) -> Result<(), String> {
    let mut expected = vec![
        Entry::Removed {
            now: 100,
            pos: p,
            old: A,
            moved_by_piston: false,
        },
        Entry::Placed {
            now: 100,
            pos: p,
            new: B,
            old: A,
            moved_by_piston: false,
        },
    ];
    expected.extend(
        NEIGHBOUR_ORDER
            .iter()
            .map(|d| direct_neighbour(offset(p, *d), BLOCK_A)),
    );
    expected.extend(SHAPE_ORDER.iter().map(|d| direct_shape(p, *d, B)));
    first_difference(&step_three_entries(log), &expected)
}

#[test]
fn sim_set_002_direct_updates_follow_removal_and_placement_and_carry_the_old_block() {
    let p = at(8, 8, 8);
    let runner = run_set(filled(&[(0, 0, 0)]), Vec::new(), p, B, 3);
    let log = runner.behaviour.log();
    assert_eq!(
        check_sim_set_002_direct_updates_after_a_replacement(&log, p),
        Ok(())
    );

    let new_block_as_source: Vec<Entry> = log
        .iter()
        .map(|entry| match entry {
            Entry::Neighbour {
                pos,
                state,
                moved_by_piston,
                ..
            } => Entry::Neighbour {
                pos: *pos,
                state: *state,
                source: BLOCK_B,
                moved_by_piston: *moved_by_piston,
            },
            other => *other,
        })
        .collect();
    assert!(check_sim_set_002_direct_updates_after_a_replacement(&new_block_as_source, p).is_err());
}

fn check_sim_set_002_same_state(log: &[Entry], writes: usize, marked: usize) -> Result<(), String> {
    let sets: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches!(entry, Entry::Set { .. }))
        .map(|(index, _)| index)
        .collect();
    let [first, second] = sets.as_slice() else {
        return Err(format!("{} sets were logged, expected two", sets.len()));
    };
    if *second != first + 1 {
        return Err(format!(
            "the second set logged {} entries before it returned",
            second - first - 1
        ));
    }
    let results: Vec<bool> = [first, second]
        .iter()
        .filter_map(|index| match log.get(**index) {
            Some(Entry::Set { result, .. }) => Some(*result),
            _ => None,
        })
        .collect();
    if results != [true, false] {
        return Err(format!("the sets answered {results:?}"));
    }
    if writes == 1 && marked == 1 {
        Ok(())
    } else {
        Err(format!(
            "{writes} writes and {marked} marks, expected one of each"
        ))
    }
}

#[test]
fn sim_set_002_setting_the_same_state_again_does_nothing() {
    let (p, tick) = (at(8, 8, 8), at(12, 8, 12));
    let set = Action::Set {
        pos: p,
        state: B,
        flags: 3,
        limit: 512,
    };
    let rules = vec![Rule::on(Hook::BlockTick, tick, vec![set, set]).once()];
    let mut runner = Runner::new(filled(&[(0, 0, 0)]), rules);
    let trigger = [block_tick_message(0, tick, BLOCK_A, 100)];
    let out = runner
        .step(100, Phase::BlockTicks, &trigger)
        .expect("owned message");
    let log = runner.behaviour.log();
    assert_eq!(
        check_sim_set_002_same_state(&log, out.writes.len(), out.marked.len()),
        Ok(())
    );

    let mut noisy = log.clone();
    if let Some(second) = noisy
        .iter()
        .rposition(|entry| matches!(entry, Entry::Set { .. }))
    {
        noisy.insert(
            second,
            direct_neighbour(offset(p, Direction::West), BLOCK_A),
        );
    }
    assert!(check_sim_set_002_same_state(&noisy, out.writes.len(), out.marked.len()).is_err());
}

fn check_sim_set_002_direct_updates_after_a_second_set(
    log: &[Entry],
    p: BlockPos,
) -> Result<(), String> {
    let neighbours = neighbour_entries(log);
    if let Some((pos, _, _)) = neighbours.iter().find(|(_, _, source)| *source == BLOCK_A) {
        return Err(format!(
            "found the first set's direct neighbour update at {pos:?} with source {BLOCK_A:?}"
        ));
    }
    let second_set: Vec<Entry> = NEIGHBOUR_ORDER
        .iter()
        .map(|d| direct_neighbour(offset(p, *d), BLOCK_B))
        .collect();
    let found: Vec<Entry> = log
        .iter()
        .filter(|entry| matches!(entry, Entry::Neighbour { .. }))
        .copied()
        .collect();
    first_difference(&found, &second_set)?;
    let shapes = shape_entries(log);
    match shapes.iter().find(|(_, _, _, carried)| *carried != C) {
        Some((pos, ..)) => Err(format!(
            "found a direct shape update at {pos:?} that does not carry the second set's state"
        )),
        None if shapes.len() == 6 => Ok(()),
        None => Err(format!(
            "found {} direct shape updates, expected the second set's six",
            shapes.len()
        )),
    }
}

#[test]
fn sim_set_002_a_placement_that_sets_again_skips_the_first_sets_direct_updates() {
    let p = at(8, 8, 8);
    let rules = vec![
        Rule::on(
            Hook::Placed,
            p,
            vec![Action::Set {
                pos: p,
                state: C,
                flags: 3,
                limit: 512,
            }],
        )
        .once(),
    ];
    let runner = run_set(filled(&[(0, 0, 0)]), rules, p, B, 3);
    assert_eq!(runner.host.state(p), C);
    let log = runner.behaviour.log();
    assert_eq!(
        check_sim_set_002_direct_updates_after_a_second_set(&log, p),
        Ok(())
    );

    let mut with_first_set = log.clone();
    with_first_set.extend(
        NEIGHBOUR_ORDER
            .iter()
            .map(|d| direct_neighbour(offset(p, *d), BLOCK_A)),
    );
    assert!(check_sim_set_002_direct_updates_after_a_second_set(&with_first_set, p).is_err());
}

fn check_sim_set_002_removal(
    log: &[Entry],
    p: BlockPos,
    old: BlockStateId,
    runs: bool,
) -> Result<(), String> {
    let removals = log
        .iter()
        .filter(|entry| matches!(entry, Entry::Removed { pos, old: was, .. } if *pos == p && *was == old))
        .count();
    match (runs, removals) {
        (true, 1) | (false, 0) => Ok(()),
        _ => Err(format!(
            "{removals} removals of {old:?} at {p:?}, runs expected: {runs}"
        )),
    }
}

#[test]
fn sim_set_002_a_rail_replaced_by_another_rail_shape_runs_its_removal() {
    let p = at(8, 8, 8);
    let mut host = filled(&[(0, 0, 0)]);
    host.put(p, RAIL_FLAT);
    let runner = run_set(host, Vec::new(), p, RAIL_SLOPED, 1);
    let rail_log = runner.behaviour.log();
    assert_eq!(
        check_sim_set_002_removal(&rail_log, p, RAIL_FLAT, true),
        Ok(())
    );

    let runner = run_set(filled(&[(0, 0, 0)]), Vec::new(), p, A_OTHER, 1);
    assert_eq!(
        check_sim_set_002_removal(&runner.behaviour.log(), p, A, false),
        Ok(())
    );

    let without_removal: Vec<Entry> = rail_log
        .iter()
        .filter(|entry| !matches!(entry, Entry::Removed { .. }))
        .copied()
        .collect();
    assert!(check_sim_set_002_removal(&without_removal, p, RAIL_FLAT, true).is_err());
}

fn check_sim_set_002_piston_move(log: &[Entry], p: BlockPos) -> Result<(), String> {
    let removed = log.iter().any(
        |entry| matches!(entry, Entry::Removed { pos, moved_by_piston: true, .. } if *pos == p),
    );
    let placed = log.iter().any(
        |entry| matches!(entry, Entry::Placed { pos, moved_by_piston: true, .. } if *pos == p),
    );
    if !removed {
        return Err("the removal did not run as moved by a piston".to_owned());
    }
    if !placed {
        return Err("the placement did not run as moved by a piston".to_owned());
    }
    match neighbour_entries(log).first() {
        Some((pos, ..)) => Err(format!(
            "found a neighbour update at {pos:?} without notify-neighbours"
        )),
        None => Ok(()),
    }
}

#[test]
fn sim_set_002_piston_move_runs_the_removal_without_notify_neighbours() {
    let p = at(8, 8, 8);
    let runner = run_set(filled(&[(0, 0, 0)]), Vec::new(), p, B, 64);
    let log = runner.behaviour.log();
    assert_eq!(check_sim_set_002_piston_move(&log, p), Ok(()));

    let without_removal: Vec<Entry> = log
        .iter()
        .filter(|entry| !matches!(entry, Entry::Removed { .. }))
        .copied()
        .collect();
    assert!(check_sim_set_002_piston_move(&without_removal, p).is_err());
}

fn check_sim_set_004(
    log: &[Entry],
    above: BlockPos,
    final_state: BlockStateId,
    expected: BlockStateId,
) -> Result<(), String> {
    let updated: Vec<BlockPos> = neighbour_entries(log)
        .into_iter()
        .filter(|(_, _, source)| *source == BLOCK_NEEDS_SUPPORT)
        .map(|(pos, _, _)| pos)
        .collect();
    let around_destroyed = around(above, &NEIGHBOUR_ORDER);
    if updated != around_destroyed {
        return Err(format!(
            "the destroyed block's neighbours were updated at {updated:?}, expected {around_destroyed:?}"
        ));
    }
    if final_state == expected {
        Ok(())
    } else {
        Err(format!(
            "the destroyed block left {final_state:?}, expected {expected:?}"
        ))
    }
}

#[test]
fn sim_set_004_a_block_a_shape_update_turns_to_air_updates_its_neighbours() {
    let p = at(8, 8, 8);
    let above = offset(p, Direction::Up);
    for (held, left) in [(NEEDS_SUPPORT, AIR), (NEEDS_SUPPORT_WET, SOURCE)] {
        let mut host = filled(&[(0, 0, 0)]);
        host.put(above, held);
        let runner = run_set(host, Vec::new(), p, AIR, 3);
        let log = runner.behaviour.log();
        let state = runner.host.state(above);
        assert_eq!(check_sim_set_004(&log, above, state, left), Ok(()));

        let silent: Vec<Entry> = log
            .iter()
            .filter(|entry| !matches!(entry, Entry::Neighbour { source, .. } if *source == BLOCK_NEEDS_SUPPORT))
            .copied()
            .collect();
        assert!(check_sim_set_004(&silent, above, state, left).is_err());
    }
}
