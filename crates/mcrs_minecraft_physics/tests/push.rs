//! Kernel and property tests of the push between entities.

use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::ListedBodies;
use mcrs_minecraft_physics::PushOutcome;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::push_vector;

const CASES: u32 = if cfg!(debug_assertions) { 300 } else { 20_000 };

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * ((self.next() >> 11) as f64 / (1_u64 << 53) as f64)
    }

    fn below(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }
}

fn mob(index: u32, x: f64, z: f64) -> GridRow {
    let id = EntityId(u64::from(index) + 1);
    GridRow {
        position: DVec3::new(x, 0.0, z),
        half_width: 0.3,
        height: 1.95,
        key: BirthKey::new(0, SourceKind::Spawn, 0, index),
        id,
        root_vehicle: id,
        team: TeamSlot::default(),
        flags: GridFlags::PUSHABLE,
        push_targets: PushTargets::Each,
    }
}

fn pushed(rows: &[GridRow], of: &GridRow, rules: &PushRules) -> (DVec3, PushOutcome) {
    let bodies = ListedBodies {
        colliders: Vec::new(),
        rows: rows.to_vec(),
    };
    let mut velocity = DVec3::ZERO;
    let stream = KeyedStream::new(&of.key, 7);
    let outcome = push_entity(
        &bodies,
        of,
        rules,
        &stream,
        &mut velocity,
        &mut Scratch::new(),
    );
    (velocity, outcome)
}

#[test]
fn psh_004_pair_gets_the_vanilla_push() {
    // The vanilla server gives each of two mobs 0.3 apart this velocity in one tick (E34): the
    // push of each direction of the pair.
    for (distance, expected) in [(0.3, 0.05477225656668268), (0.05, 0.02236068010818782)] {
        let a = mob(0, 1632.5, 0.5);
        let b = mob(1, 1632.5 + distance, 0.5);
        let rows = [a, b];
        let (on_a, _) = pushed(&rows, &a, &PushRules::default());
        let (on_b, _) = pushed(&rows, &b, &PushRules::default());
        assert_eq!(on_a, DVec3::new(-expected, 0.0, 0.0), "distance {distance}");
        assert_eq!(on_b, DVec3::new(expected, 0.0, 0.0), "distance {distance}");
    }
    let a = mob(0, 0.5, 0.5);
    let near = mob(1, 0.505, 0.5);
    assert_eq!(
        push_vector(&a, &near),
        None,
        "nearer than 0.01F on both axes"
    );
}

#[test]
fn psh_005_swapped_roles_negate_the_push_bit_for_bit() {
    let mut rng = Rng(3);
    for index in 0..CASES * 10 {
        let a = mob(0, rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        let b = mob(index + 1, rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        match (push_vector(&a, &b), push_vector(&b, &a)) {
            (Some(forward), Some(back)) => {
                assert_eq!(
                    [forward.x, forward.z].map(f64::to_bits),
                    [-back.x, -back.z].map(f64::to_bits)
                );
                let bound = f64::from(0.05_f32) * (1.0 + 4.0 * f64::EPSILON);
                assert!(forward.x.abs() <= bound && forward.z.abs() <= bound);
            }
            (None, None) => {}
            other => panic!("one direction only: {other:?}"),
        }
    }
}

fn crowd(rng: &mut Rng, count: u32) -> Vec<GridRow> {
    (0..count)
        .map(|index| {
            let mut row = mob(index, rng.range(0.0, 1.5), rng.range(0.0, 1.5));
            if rng.below(8) == 0 {
                row.flags = GridFlags::empty();
            }
            if rng.below(8) == 0 {
                row.push_targets = PushTargets::None;
            }
            row
        })
        .collect()
}

#[test]
fn cmp_012_any_row_order_and_task_split_give_equal_velocities() {
    let mut rng = Rng(12);
    for _ in 0..CASES {
        let count = 2 + rng.below(30) as u32;
        let rows = crowd(&mut rng, count);
        let rules = PushRules {
            max_entity_cramming: [24, 0, 4][rng.below(3)],
            cap: 24,
        };
        let reference: Vec<_> = rows.iter().map(|row| pushed(&rows, row, &rules)).collect();
        let mut shuffled = rows.clone();
        for index in (1..shuffled.len()).rev() {
            shuffled.swap(index, rng.below(index + 1));
        }
        let split = rng.below(rows.len());
        let (first, second) = rows.split_at(split);
        for (row, expected) in second
            .iter()
            .chain(first)
            .zip(reference[split..].iter().chain(&reference[..split]))
        {
            let (velocity, outcome) = pushed(&shuffled, row, &rules);
            assert_eq!(
                velocity.to_array().map(f64::to_bits),
                expected.0.to_array().map(f64::to_bits)
            );
            assert_eq!(outcome, expected.1);
        }
    }
}

#[test]
fn cmp_019_the_cap_keeps_pairs_symmetric() {
    let mut rng = Rng(19);
    let rules = PushRules {
        max_entity_cramming: 0,
        cap: 24,
    };
    for _ in 0..CASES / 10 {
        let rows: Vec<GridRow> = (0..40)
            .map(|index| mob(index, rng.range(0.0, 0.6), rng.range(0.0, 0.6)))
            .collect();
        let total: DVec3 = rows.iter().map(|row| pushed(&rows, row, &rules).0).sum();
        assert!(
            total.abs().max_element() < 1.0e-12,
            "the pushes of the crowd do not cancel: {total}"
        );
        let uncapped: DVec3 = rows
            .iter()
            .map(|row| pushed(&rows, row, &PushRules::default()).0)
            .sum();
        assert!(uncapped.abs().max_element() < 1.0e-12);
    }
}

#[test]
fn psh_007_crowd_damage_draws_one_in_four_from_the_keyed_stream() {
    let rows: Vec<GridRow> = (0..30)
        .map(|index| mob(index, 0.5 + f64::from(index) * 0.001, 0.5))
        .collect();
    let bodies = ListedBodies {
        colliders: Vec::new(),
        rows: rows.clone(),
    };
    let mut damaged = 0;
    let ticks = 4000;
    for tick in 0..ticks {
        let stream = KeyedStream::new(&rows[0].key, tick);
        let mut velocity = DVec3::ZERO;
        let outcome = push_entity(
            &bodies,
            &rows[0],
            &PushRules::default(),
            &stream,
            &mut velocity,
            &mut Scratch::new(),
        );
        assert_eq!(outcome.candidates, 29);
        damaged += u32::from(outcome.crowd_damage);
    }
    let rate = f64::from(damaged) / ticks as f64;
    assert!(
        (rate - 0.25).abs() < 0.03,
        "crowd damage in {rate} of the ticks"
    );
    let few = &rows[..24];
    let (_, outcome) = pushed(few, &few[0], &PushRules::default());
    assert!(
        !outcome.crowd_damage,
        "23 candidates are not more than 24 - 1"
    );
}
