use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::KeyedRandom;
use mcrs_minecraft_brain::Part;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity_key::EntityKey;

fn first_values(seed: u64, key: u128, tick: u64, part: u32) -> [u64; 4] {
    let mut random = KeyedRandom::new(WorldSeed(seed), &EntityKey(key), tick, Part(part));
    [(); 4].map(|()| random.next_u64())
}

#[test]
fn a_stream_depends_on_each_of_its_keys_and_on_nothing_else() {
    let base = first_values(7, 3, 100, 1);
    assert_eq!(base, first_values(7, 3, 100, 1));
    for other in [
        first_values(8, 3, 100, 1),
        first_values(7, 4, 100, 1),
        first_values(7, 3, 101, 1),
        first_values(7, 3, 100, 2),
    ] {
        assert!(
            base.iter().zip(&other).all(|(a, b)| a != b),
            "a change of one key changes every value"
        );
    }
}

#[test]
fn a_stream_absorbs_the_high_word_of_the_key_before_the_low_word() {
    assert_eq!(
        first_values(7, 1 << 64 | 2, 100, 1)[0],
        0x6C33_1228_048B_6B72,
        "SplitMix64 over the seed, the words 1 and 2, the tick and the part"
    );
}

#[test]
fn trials_to_success_follow_the_geometric_law() {
    let chance = 1.0 / 60.0;
    let samples = 200_000_u32;
    let mut sum = 0.0;
    let mut first = 0_u32;
    for draw in 0..samples {
        let mut random = KeyedRandom::new(WorldSeed(5), &EntityKey(u128::from(draw)), 0, Part(0));
        let trials = random.trials_to_success(chance);
        sum += f64::from(trials);
        first += u32::from(trials == 1);
    }
    let mean = sum / f64::from(samples);
    assert!(
        (mean - 60.0).abs() < 0.6,
        "the mean of a geometric law is 1 / p; got {mean}"
    );
    let first_share = f64::from(first) / f64::from(samples);
    assert!(
        (first_share - chance).abs() < 0.0015,
        "P(1) is p; got {first_share}"
    );
}

#[test]
fn full_evaluations_start_at_the_first_step_and_follow_the_phase() {
    let even = Cadence::new(100, 0);
    let evaluations: Vec<u64> = (99..108)
        .filter(|tick| even.is_full_evaluation(*tick))
        .collect();
    assert_eq!(
        evaluations,
        [100, 101, 103, 105, 107],
        "tick counts 1, 2, 4, 6, 8"
    );
    let odd = Cadence::new(100, 1);
    let evaluations: Vec<u64> = (99..108)
        .filter(|tick| odd.is_full_evaluation(*tick))
        .collect();
    assert_eq!(evaluations, [100, 102, 104, 106], "tick counts 1, 3, 5, 7");
    assert_eq!(odd.nth_evaluation(100, 3), 104);
    assert_eq!(odd.nth_evaluation(101, 1), 102);
    assert_eq!(even.nth_evaluation(100, 4), 105);
}
