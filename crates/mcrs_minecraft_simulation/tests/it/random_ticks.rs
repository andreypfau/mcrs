use crate::probe::{A, AIR, B, C, Host, ProbeFacts, ProbeKinds, STATES, at, draw_hits, section};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockShard, BlockUpdateFlags, Cx, Env, Footprints, GameTick,
    KernelConfig, PartitionKind, RandomPicks, RandomTickTable, RandomTicks, SemanticOrder, Serial,
    ShardId, SimulationVolume, StepOutput, StreamSeed, random_step,
};
use mcrs_minecraft_volume::{BlockVolume, BlockVolumeMut, SectionBlocksMut};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// Block `A` takes random ticks. A random tick of `A` turns it into `B` when `replace` is set,
/// and counts it either way.
struct Picks {
    replace: bool,
    ticks: AtomicU32,
}

impl BlockBehaviour<ProbeKinds> for Picks {
    fn random_ticks(&self, state: BlockStateId) -> RandomTicks {
        if state == A {
            RandomTicks::BLOCK
        } else {
            RandomTicks::empty()
        }
    }

    fn random_tick<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        at: BlockPos,
        _state: BlockStateId,
        _random: &mut LegacyRandom,
    ) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
        if self.replace {
            cx.set(at, B, BlockUpdateFlags::CLIENTS, 512);
        }
    }
}

fn table(rules: &Picks) -> RandomTickTable {
    let states = (0..STATES).filter_map(|number| {
        BlockStateId::checked(u16::try_from(number).expect("a small table"), STATES)
    });
    RandomTickTable::new(states, |state| rules.random_ticks(state))
}

fn run(
    host: &mut Host,
    rules: &Picks,
    sections: &[SectionPos],
    speed: u32,
    ticks: i64,
) -> StepOutput<ProbeKinds> {
    let table = table(rules);
    host.set_random_ticks(table.clone());
    let config = KernelConfig::default();
    let env = Env {
        partition: ShardId(0),
        facts: &ProbeFacts,
        behaviour: rules,
        policy: &SemanticOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    for now in 0..ticks {
        let hits = host.draw_hits(sections, speed, now);
        let picks = RandomPicks {
            columns: &[],
            hits: &hits,
            speed,
            table: &table,
            footprints: &Footprints::default(),
            seed: StreamSeed::new(4, "minecraft:overworld"),
        };
        random_step(
            &mut shard,
            GameTick(now),
            host,
            &env,
            picks,
            &mut out,
            &Serial,
        );
    }
    out
}

fn count(host: &Host, at: SectionPos, state: BlockStateId) -> usize {
    (0..16)
        .flat_map(|y| (0..16).flat_map(move |z| (0..16).map(move |x| (x, y, z))))
        .filter(|(x, y, z)| host.state(at_in(at, *x, *y, *z)) == state)
        .count()
}

fn at_in(section: SectionPos, x: i32, y: i32, z: i32) -> BlockPos {
    at(section.x * 16 + x, section.y * 16 + y, section.z * 16 + z)
}

/// Only a section that holds a state that takes random ticks gets picks, and it gets `speed` of
/// them in each step.
#[test]
fn picks_reach_only_sections_that_hold_a_randomly_ticking_state() {
    let (ticking, inert) = (section(0, 0, 0), section(1, 0, 0));
    let mut host = Host::new();
    host.insert(ticking, A);
    host.insert(inert, C);
    let rules = Picks {
        replace: true,
        ticks: AtomicU32::new(0),
    };

    run(&mut host, &rules, &[ticking, inert], 0, 5);
    assert_eq!(count(&host, ticking, B), 0, "speed 0 makes no pick");

    run(&mut host, &rules, &[ticking, inert], 3, 1);
    let replaced = count(&host, ticking, B);
    assert!(
        (1..=3).contains(&replaced),
        "three picks of a section of A replace one to three blocks, not {replaced}"
    );
    assert_eq!(count(&host, inert, C), 4096, "a section of C takes no tick");
}

/// SIM-RND-002: one probe in a section gets a mean of 3 * 10000 / 4096 = 7.32 ticks over 10,000
/// steps at speed 3. 64 probes, one in each section, sum to 469 with a deviation of 21.6; a pick
/// that skips the y offset gives about 117 for each probe.
#[test]
fn a_single_block_gets_its_share_of_the_picks() {
    let sections: Vec<SectionPos> = (0..64).map(|i| section(i % 8, i / 8 - 4, 0)).collect();
    let mut host = Host::new();
    for (i, &target) in (0..).zip(&sections) {
        host.insert(target, AIR);
        host.put(at_in(target, i % 16, (i * 7) % 16, (i * 3) % 16), A);
    }
    let rules = Picks {
        replace: false,
        ticks: AtomicU32::new(0),
    };

    run(&mut host, &rules, &sections, 3, 10_000);

    let ticks = rules.ticks.load(Ordering::Relaxed);
    assert!(
        (382..=556).contains(&ticks),
        "a mean of 7.32 ticks for each of 64 probes, within 4 deviations of 469, not {ticks}"
    );
}

proptest::proptest! {
    /// The listed positions of a section are the positions whose state ticks, as a model of the
    /// section finds them, after any sequence of sets and fills.
    #[test]
    fn the_listed_positions_are_the_ticking_positions_of_the_section(
        writes in proptest::collection::vec((0usize..4096, 0usize..4, 0u8..8), 1..300),
    ) {
        let target = section(0, 0, 0);
        let mut host = Host::new();
        host.insert(target, C);
        host.set_random_ticks(table(&Picks { replace: false, ticks: AtomicU32::new(0) }));
        let states = [AIR, A, B, C];
        let mut model = [C; 4096];
        for (index, state, kind) in writes {
            let local = LocalPos::from_index(index);
            let state = states[state];
            let mut blocks = host.section_mut(target).expect("a writable section");
            let written = |at: LocalPos| match kind {
                0 => true,
                1 => at.x() <= local.x() && at.y() <= local.y() && at.z() <= local.z(),
                _ => at == local,
            };
            for at in LocalPos::all().filter(|&at| written(at)) {
                model[at.index()] = state;
            }
            match kind {
                0 => blocks.fill(state),
                1 => blocks.fill_box(LocalPos::new(0, 0, 0), local, state),
                _ => {
                    blocks.set(local, state);
                }
            }
        }
        let mut listed = host.random_ticking(target).to_vec();
        listed.sort_unstable();
        for local in LocalPos::all() {
            let at = at_in(target, local.x().into(), local.y().into(), local.z().into());
            proptest::prop_assert_eq!(host.block(at), Ok(model[local.index()]));
        }
        let modelled: Vec<u16> = LocalPos::all()
            .filter(|&local| model[local.index()] == A)
            .map(|local| u16::try_from(local.index()).expect("a local index"))
            .collect();
        proptest::prop_assert_eq!(listed, modelled);
    }
}

/// The picks of a section do not depend on the other sections of the step or their order, so a
/// world of many cells picks as a world of one cell.
#[test]
fn the_picks_of_a_section_do_not_depend_on_the_other_sections() {
    let (first, second) = (section(0, 0, 0), section(1, 0, 0));
    let picked = |sections: &[SectionPos]| {
        let mut host = Host::new();
        host.insert(first, A);
        host.insert(second, A);
        let rules = Picks {
            replace: true,
            ticks: AtomicU32::new(0),
        };
        let out = run(&mut host, &rules, sections, 3, 20);
        let mut writes: Vec<(i32, i32, i32)> = out
            .writes
            .iter()
            .filter(|write| SectionPos::from(write.pos) == first)
            .map(|write| (write.pos.x, write.pos.y, write.pos.z))
            .collect();
        writes.sort_unstable();
        writes
    };

    let alone = picked(&[first]);
    assert!(!alone.is_empty(), "the section takes picks");
    assert_eq!(picked(&[second, first]), alone);
}

/// Records the columns that precipitation picks hit.
#[derive(Default)]
struct Rain {
    hits: Mutex<Vec<(i32, i32)>>,
}

impl BlockBehaviour<ProbeKinds> for Rain {
    fn precipitation<V: SimulationVolume, F: BlockFacts<ProbeKinds>>(
        &self,
        _cx: &mut Cx<'_, ProbeKinds, V, F, Self>,
        x: i32,
        z: i32,
        _random: &mut LegacyRandom,
    ) {
        self.hits.lock().expect("one step at a time").push((x, z));
    }
}

/// SIM-RND-004: a column takes precipitation picks whatever its blocks are, and the hits of a
/// column do not depend on the columns of other block tiles or their order.
#[test]
fn precipitation_hits_of_a_column_do_not_depend_on_other_tiles() {
    let (first, far) = (ColumnPos::new(0, 0), ColumnPos::new(5, 0));
    let rained_on_first = |columns: &[ColumnPos]| {
        let mut host = Host::new();
        host.insert(section(0, 0, 0), AIR);
        host.insert(section(5, 0, 0), AIR);
        let rules = Rain::default();
        let config = KernelConfig::default();
        let env = Env {
            partition: ShardId(0),
            facts: &ProbeFacts,
            behaviour: &rules,
            policy: &SemanticOrder,
            kind: PartitionKind::Shard,
            config: &config,
        };
        let table = RandomTickTable::default();
        let mut shard = BlockShard::new();
        let mut out = StepOutput::default();
        for now in 0..400 {
            let picks = RandomPicks {
                columns,
                hits: &[],
                speed: 3,
                table: &table,
                footprints: &Footprints::default(),
                seed: StreamSeed::new(4, "minecraft:overworld"),
            };
            random_step(
                &mut shard,
                GameTick(now),
                &mut host,
                &env,
                picks,
                &mut out,
                &Serial,
            );
        }
        let hits = rules.hits.into_inner().expect("one step at a time");
        let first_hits: Vec<(i32, i32)> = hits
            .into_iter()
            .filter(|&(x, z)| (0..16).contains(&x) && (0..16).contains(&z))
            .collect();
        first_hits
    };

    let alone = rained_on_first(&[first]);
    assert!(
        (5..=50).contains(&alone.len()),
        "about 400 * 3 / 48 = 25 hits, not {}",
        alone.len()
    );
    assert_eq!(rained_on_first(&[far, first]), alone);
}

/// SIM-RND-001..003: the picks of a section with a fixed seed keep their positions. Half the
/// section ticks, so draws both hit and miss, and the draws of a step index the list of the start
/// of the step, though each hit shrinks the list.
#[test]
fn the_picks_of_a_section_keep_their_positions_for_a_fixed_seed() {
    let target = section(1, -1, 2);
    let mut host = Host::new();
    host.insert(target, C);
    for local in LocalPos::all().filter(|local| local.y() < 8) {
        host.put(
            at_in(target, local.x().into(), local.y().into(), local.z().into()),
            A,
        );
    }
    let rules = Picks {
        replace: true,
        ticks: AtomicU32::new(0),
    };

    let out = run(&mut host, &rules, &[target], 3, 10);

    let picked: Vec<(i32, i32, i32)> = out
        .writes
        .iter()
        .map(|write| (write.pos.x, write.pos.y, write.pos.z))
        .collect();
    assert_eq!(
        picked,
        [
            (16, -15, 41),
            (17, -14, 33),
            (16, -14, 46),
            (27, -15, 32),
            (28, -9, 35),
            (17, -11, 43),
            (22, -10, 41),
            (24, -11, 37),
            (19, -11, 36),
            (20, -11, 42),
            (23, -13, 37),
            (19, -10, 33),
            (19, -11, 47),
        ]
    );
}

/// SIM-MODEL-106(a), SIM-RND-002: each position of a section takes `k` uniform picks in each
/// step. The hits of a section where every position ticks spread evenly over its positions; a
/// section where half the positions tick gets half the hits, and the hits of two such sections
/// do not correlate.
#[test]
fn the_drawn_hits_are_uniform_over_the_positions_and_independent_across_sections() {
    const STEPS: i64 = 20_000;
    let (full, even, odd) = (section(0, 0, 0), section(1, 0, 0), section(0, 1, 0));
    let every: Vec<u16> = (0..4096).collect();
    let evens: Vec<u16> = (0..4096).step_by(2).collect();
    let odds: Vec<u16> = (1..4096).step_by(2).collect();
    let seed = StreamSeed::new(4, "minecraft:overworld");
    let mut per_position = vec![0u32; 4096];
    let mut halves = Vec::new();
    let mut hits = Vec::new();
    for now in 0..STEPS {
        let streams = seed.sections(GameTick(now));
        hits.clear();
        draw_hits(full, &every, 3, streams, &mut hits);
        assert_eq!(hits.len(), 3, "every pick of a full section hits");
        for hit in &hits {
            per_position[LocalPos::from(hit.at).index()] += 1;
        }
        hits.clear();
        draw_hits(even, &evens, 3, streams, &mut hits);
        let on_even = hits.len();
        draw_hits(odd, &odds, 3, streams, &mut hits);
        assert!(
            hits.iter()
                .enumerate()
                .all(|(n, hit)| LocalPos::from(hit.at).index() % 2 == usize::from(n >= on_even)),
            "a hit lands on a listed position of its section"
        );
        halves.push((on_even as f64, (hits.len() - on_even) as f64));
    }

    let expected = 3.0 * STEPS as f64 / 4096.0;
    let chi_square: f64 = per_position
        .iter()
        .map(|&count| (f64::from(count) - expected).powi(2) / expected)
        .sum();
    assert!(
        (chi_square - 4095.0).abs() < 5.0 * (2.0 * 4095.0_f64).sqrt(),
        "the hits of a full section are uniform: chi-square {chi_square:.0} on 4095 degrees"
    );
    let steps = STEPS as f64;
    // Binomial(3, 1/2) in one step: a mean of 1.5 and a variance of 0.75.
    let bound = 5.0 * 0.75_f64.sqrt() / steps.sqrt();
    let mean_even = halves.iter().map(|half| half.0).sum::<f64>() / steps;
    let mean_odd = halves.iter().map(|half| half.1).sum::<f64>() / steps;
    assert!(
        (mean_even - 1.5).abs() < bound && (mean_odd - 1.5).abs() < bound,
        "half the positions get half the picks: {mean_even}, {mean_odd}"
    );
    let covariance = halves
        .iter()
        .map(|(a, b)| (a - mean_even) * (b - mean_odd))
        .sum::<f64>()
        / steps;
    assert!(
        covariance.abs() < 5.0 * 0.75 / steps.sqrt(),
        "the hits of two sections do not correlate: covariance {covariance}"
    );
}
