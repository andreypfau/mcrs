//! Kernel and property tests of the push between entities.

use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::ListedBodies;
use mcrs_minecraft_physics::PushOutcome;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::push_vector;

const CASES: u32 = if cfg!(debug_assertions) { 300 } else { 20_000 };

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
fn psh_005_swapped_roles_negate_the_push_bit_for_bit() {
    let mut rng = TestRng(3);
    for index in 0..CASES * 10 {
        let a = TestFixture::mob(0, rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
        let b = TestFixture::mob(index + 1, rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
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

fn crowd(rng: &mut TestRng, count: u32) -> Vec<GridRow> {
    (0..count)
        .map(|index| {
            let mut row = TestFixture::mob(index, rng.range(0.0, 1.5), rng.range(0.0, 1.5));
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
    let mut rng = TestRng(12);
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
    let mut rng = TestRng(19);
    let rules = PushRules {
        max_entity_cramming: 0,
        cap: 24,
    };
    for _ in 0..CASES / 10 {
        let rows: Vec<GridRow> = (0..40)
            .map(|index| TestFixture::mob(index, rng.range(0.0, 0.6), rng.range(0.0, 0.6)))
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

fn bits(vector: DVec3) -> [u64; 3] {
    vector.to_array().map(f64::to_bits)
}

#[test]
fn cmp_027_a_pair_that_is_too_near_gets_the_push_of_the_earlier_entity() {
    let rules = PushRules::default();
    let stood = TestFixture::mob(0, 0.5, 0.5);
    let came = GridRow {
        previous: DVec3::new(0.2, 0.0, 0.5),
        ..TestFixture::mob(1, 0.5, 0.5)
    };
    let before = GridRow {
        position: came.previous,
        ..came
    };
    let expected = push_vector(&stood, &before);
    assert!(expected.is_some(), "the two were 0.3 apart");
    let rows = [stood, came];
    let (on_stood, outcome) = pushed(&rows, &stood, &rules);
    assert!(outcome.pushed);
    assert_eq!(Some(bits(on_stood)), expected.map(bits));
    assert_eq!(
        Some(pushed(&rows, &came, &rules).0),
        expected.map(|push| -push),
        "the later entity adds the negative"
    );

    // The earlier entity moved first, onto the later one that still stood there.
    let came_first = GridRow {
        previous: DVec3::new(0.2, 0.0, 0.5),
        ..stood
    };
    let stood_later = TestFixture::mob(1, 0.5, 0.5);
    // The later entity came from a place where the two boxes did not intersect.
    let from_far = GridRow {
        previous: DVec3::new(5.0, 0.0, 0.5),
        ..TestFixture::mob(1, 0.5, 0.5)
    };
    for rows in [[came_first, stood_later], [stood, from_far]] {
        for row in &rows {
            let (velocity, outcome) = pushed(&rows, row, &rules);
            assert_eq!(velocity, DVec3::ZERO);
            assert!(!outcome.pushed);
        }
    }
}

#[test]
fn cmp_027_the_pushes_of_a_stack_cancel_in_any_row_order() {
    let mut rng = TestRng(27);
    let rules = PushRules::default();
    for _ in 0..CASES / 10 {
        let rows: Vec<GridRow> = (0..20)
            .map(|index| GridRow {
                previous: DVec3::new(rng.range(0.0, 1.0), 0.0, rng.range(0.0, 1.0)),
                ..TestFixture::mob(index, 0.5, 0.5)
            })
            .collect();
        let mut shuffled = rows.clone();
        for index in (1..shuffled.len()).rev() {
            shuffled.swap(index, rng.below(index + 1));
        }
        let mut total = DVec3::ZERO;
        let mut moved = 0;
        for row in &rows {
            let (velocity, _) = pushed(&rows, row, &rules);
            assert_eq!(bits(velocity), bits(pushed(&shuffled, row, &rules).0));
            total += velocity;
            moved += usize::from(velocity != DVec3::ZERO);
        }
        assert!(moved > rows.len() / 2, "{moved} of the stack get a push");
        assert!(
            total.abs().max_element() < 1.0e-12,
            "the pushes of the stack do not cancel: {total}"
        );
    }
}

#[test]
fn psh_007_crowd_damage_draws_one_in_four_from_the_keyed_stream() {
    let rows: Vec<GridRow> = (0..30)
        .map(|index| TestFixture::mob(index, 0.5 + f64::from(index) * 0.001, 0.5))
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
