use mcrs_minecraft_random::Random;
use mcrs_minecraft_simulation::{GameTick, SequenceClock, StreamKey, StreamSeed, SubTick};
use mcrs_voxel::{BlockPos, SectionPos};

#[test]
fn one_clock_orders_stamps_as_a_plain_call_counter_would() {
    let mut clock = SequenceClock::new();
    let mut stamps = Vec::new();
    for (tick, calls) in [(0, 3), (1, 1), (4, 5), (5, 2)] {
        for _ in 0..calls {
            stamps.push(clock.stamp(GameTick(tick)));
        }
    }
    let mut sorted = stamps.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, stamps);
}

#[test]
fn restored_sequences_sort_before_every_runtime_one_in_list_order() {
    let restored: Vec<_> = (0..4).filter_map(|k| SubTick::restored(k, 4)).collect();
    assert_eq!(SubTick::restored(4, 4), None);
    assert_eq!(
        restored.iter().map(|s| s.get()).collect::<Vec<_>>(),
        [-4, -3, -2, -1]
    );
    let first_runtime = SequenceClock::new().stamp(GameTick(0));
    assert!(restored.iter().all(|s| *s < first_runtime));
}

#[test]
fn a_stamp_made_after_receiving_a_message_sorts_after_the_message() {
    let mut sender = SequenceClock::new();
    for _ in 0..10 {
        sender.stamp(GameTick(7));
    }
    let message = sender.stamp(GameTick(7));

    let mut receiver = SequenceClock::new();
    let before = receiver.stamp(GameTick(7));
    assert!(before < message);
    receiver.observe(message);
    assert!(receiver.stamp(GameTick(7)) > message);

    let mut joined = SequenceClock::new();
    joined.stamp(GameTick(7));
    joined.join(&sender);
    assert!(joined.stamp(GameTick(7)) > message);
}

#[test]
fn a_lagging_clock_moves_past_a_newer_stamp_and_ignores_a_restored_one() {
    let message = SubTick::runtime(GameTick(7), 2);
    let lagging = || {
        let mut clock = SequenceClock::new();
        clock.stamp(GameTick(3));
        clock
    };
    let mut observed = lagging();
    observed.observe(message);
    assert!(observed.stamp(GameTick(3)) > message);

    let mut sender = SequenceClock::new();
    for _ in 0..3 {
        sender.stamp(GameTick(7));
    }
    let mut joined = lagging();
    joined.join(&sender);
    assert!(joined.stamp(GameTick(3)) > message);

    let mut untouched = lagging();
    untouched.observe(SubTick::restored(0, 5).expect("index below count"));
    assert_eq!(untouched, lagging());
}

#[test]
fn a_clock_ahead_of_an_older_stamp_does_not_move_backwards() {
    let ahead = || {
        let mut clock = SequenceClock::new();
        let mut last = clock.stamp(GameTick(7));
        for _ in 0..5 {
            last = clock.stamp(GameTick(7));
        }
        (clock, last)
    };
    for older in [
        SubTick::runtime(GameTick(3), 40),
        SubTick::runtime(GameTick(7), 2),
    ] {
        let (mut observed, last) = ahead();
        observed.observe(older);
        assert!(observed.stamp(GameTick(7)) > last, "{older:?}");
    }
    for behind in [3, 7] {
        let mut other = SequenceClock::new();
        other.stamp(GameTick(behind));
        let (mut joined, last) = ahead();
        joined.join(&other);
        assert!(joined.stamp(GameTick(7)) > last, "tick {behind}");
    }
}

#[test]
fn every_stream_key_field_selects_its_own_stream() {
    let draws = |seed: i64, dimension: &str, now: i64, key: StreamKey| {
        let mut random = StreamSeed::new(seed, dimension).stream(GameTick(now), key);
        (0..4).map(|_| random.next_i32()).collect::<Vec<_>>()
    };
    let stamp = |counter| SubTick::runtime(GameTick(3), counter);
    let tick = |x, counter| StreamKey::Tick {
        pos: BlockPos::new(x, 64, 0),
        sequence: stamp(counter),
    };
    let event = |x, counter| StreamKey::Event {
        pos: BlockPos::new(x, 64, 0),
        sequence: stamp(counter),
    };
    let chain = |x, step| StreamKey::Chain {
        pos: BlockPos::new(x, 64, 0),
        origin: stamp(0),
        step,
    };
    let overworld = "minecraft:overworld";
    let streams = [
        draws(42, overworld, 3, tick(0, 0)),
        draws(43, overworld, 3, tick(0, 0)),
        draws(42, "minecraft:the_nether", 3, tick(0, 0)),
        draws(42, overworld, 4, tick(0, 0)),
        draws(42, overworld, 3, tick(1, 0)),
        draws(42, overworld, 3, tick(0, 1)),
        draws(42, overworld, 3, tick(1 << 26, 0)),
        draws(42, overworld, 3, event(0, 0)),
        draws(42, overworld, 3, event(0, 1)),
        draws(42, overworld, 3, event(1, 0)),
        draws(42, overworld, 3, chain(0, 0)),
        draws(42, overworld, 3, chain(0, 1)),
        draws(42, overworld, 3, chain(1, 0)),
        draws(
            42,
            overworld,
            3,
            StreamKey::Section {
                section: SectionPos::new(1, 4, 0),
            },
        ),
        draws(
            42,
            overworld,
            3,
            StreamKey::Section {
                section: SectionPos::new(0, 4, 1),
            },
        ),
        draws(
            42,
            overworld,
            3,
            StreamKey::Section {
                section: SectionPos::new(1, 5, 0),
            },
        ),
        draws(
            42,
            overworld,
            3,
            StreamKey::Visit {
                anchor: SectionPos::new(1, 4, 0),
            },
        ),
    ];
    assert_eq!(streams[0], draws(42, overworld, 3, tick(0, 0)));
    for (i, a) in streams.iter().enumerate() {
        for b in &streams[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
