use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::compile::BlockResolver;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen::proto::BlockState;

use crate::feature::tree::provider::{SharedNoise, StateProvider};

/// One `inner_placements` entry in every form it is written in: indexed by
/// face, then by whether the block it grows into holds a source fluid.
#[derive(Clone, Debug)]
pub struct GeodeCrystal {
    pub states: [[VoxelId; 2]; 6],
}

impl GeodeCrystal {
    /// `trySetValue` of `facing` and `waterlogged`: a crystal without either
    /// property is the same state in every slot, which is what the reference's
    /// `hasProperty` guards produce.
    pub fn resolve(state: &BlockState, blocks: &dyn BlockResolver) -> Option<Self> {
        const FACES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
        let try_with = |from: &BlockState, property: &str, value: &str| -> BlockState {
            let mut candidate = from.clone();
            candidate
                .properties
                .get_or_insert_with(Default::default)
                .insert(property.to_owned(), value.to_owned());
            match blocks.state(&candidate) {
                Some(_) => candidate,
                None => from.clone(),
            }
        };
        let mut states = [[VoxelId(0); 2]; 6];
        for (face, name) in FACES.into_iter().enumerate() {
            let facing = try_with(state, "facing", name);
            for (waterlogged, flag) in ["false", "true"].into_iter().enumerate() {
                states[face][waterlogged] =
                    blocks.state(&try_with(&facing, "waterlogged", flag))?;
            }
        }
        Some(GeodeCrystal { states })
    }
}

#[derive(Clone, Debug)]
pub struct CompiledGeode {
    pub filling: StateProvider,
    pub inner_layer: StateProvider,
    pub alternate_inner_layer: StateProvider,
    pub middle_layer: StateProvider,
    pub outer_layer: StateProvider,
    pub inner_placements: Vec<GeodeCrystal>,
    pub cannot_replace: StateMask,
    pub invalid_blocks: StateMask,
    /// `BuddingAmethystBlock.canClusterGrowAtState`: air, or water at full
    /// height.
    pub cluster_growable: StateMask,
    pub use_potential_placements_chance: f64,
    pub use_alternate_layer0_chance: f64,
    pub placements_require_layer0_alternate: bool,
    pub outer_wall_distance: IntProvider,
    pub outer_wall_distance_max: i32,
    pub distribution_points: IntProvider,
    pub point_offset: IntProvider,
    pub min_gen_offset: i32,
    pub max_gen_offset: i32,
    pub noise_multiplier: f64,
    pub invalid_blocks_threshold: i32,
    pub filling_thickness: f64,
    pub inner_layer_thickness: f64,
    pub middle_layer_thickness: f64,
    pub outer_layer_thickness: f64,
    pub base_crack_size: f64,
    pub generate_crack_chance: f64,
    pub crack_point_offset: i32,
    /// `NormalNoise.createParity(-4, 1.0)` over the world seed, so it belongs
    /// to the dimension rather than to the object and draws nothing here.
    pub noise: SharedNoise,
}

fn inv_sqrt(value: f64) -> f64 {
    1.0 / value.sqrt()
}

/// `GeodeFeature.place`: a distance field over budded points, shelled in four
/// layers, optionally cracked, and then studded with crystals.
pub fn place_geode<W: WorldGenVolume>(
    config: &CompiledGeode,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) -> bool {
    let point_count = config.distribution_points.sample(rng);
    let crack_adjustment = point_count as f64 / config.outer_wall_distance_max as f64;
    let inner_air = inv_sqrt(config.filling_thickness);
    let innermost_layer = inv_sqrt(config.inner_layer_thickness + crack_adjustment);
    let inner_crust = inv_sqrt(config.middle_layer_thickness + crack_adjustment);
    let outer_crust = inv_sqrt(config.outer_layer_thickness + crack_adjustment);
    let crack_size = inv_sqrt(
        config.base_crack_size
            + rng.next_f64() / 2.0
            + if point_count > 3 {
                crack_adjustment
            } else {
                0.0
            },
    );
    let crack = (rng.next_f32() as f64) < config.generate_crack_chance;

    let mut points = Vec::with_capacity(point_count.max(0) as usize);
    let mut invalid = 0;
    for _ in 0..point_count {
        let offset = IVec3::new(
            config.outer_wall_distance.sample(rng),
            config.outer_wall_distance.sample(rng),
            config.outer_wall_distance.sample(rng),
        );
        let pos = origin + offset;
        let state = volume.get(pos).0 as usize;
        if volume.world().air_states.contains(state) || config.invalid_blocks.contains(state) {
            invalid += 1;
            if invalid > config.invalid_blocks_threshold {
                return false;
            }
        }
        points.push((pos, config.point_offset.sample(rng)));
    }

    let mut crack_points = Vec::new();
    if crack {
        let arm = rng.next_i32_bound(4);
        let reach = point_count * 2 + 1;
        let (dx, dz) = match arm {
            0 => (reach, 0),
            1 => (0, reach),
            2 => (reach, reach),
            _ => (0, 0),
        };
        for y in [7, 5, 1] {
            crack_points.push(origin + IVec3::new(dx, y, dz));
        }
    }

    let mut crystal_spots = Vec::new();
    for z in config.min_gen_offset..=config.max_gen_offset {
        for y in config.min_gen_offset..=config.max_gen_offset {
            for x in config.min_gen_offset..=config.max_gen_offset {
                let pos = origin + IVec3::new(x, y, z);
                let noise = config.noise.get(pos.x as f64, pos.y as f64, pos.z as f64) as f64
                    * config.noise_multiplier;
                let mut shell = 0.0;
                for (point, bud) in &points {
                    shell +=
                        inv_sqrt((pos - *point).as_dvec3().length_squared() + *bud as f64) + noise;
                }
                if shell < outer_crust {
                    continue;
                }
                if shell >= inner_air {
                    let state = config.filling.state(volume, rng, pos);
                    volume.set_unless(&config.cannot_replace, pos, state);
                    continue;
                }
                let mut crack_distance = 0.0;
                for point in &crack_points {
                    crack_distance += inv_sqrt(
                        (pos - *point).as_dvec3().length_squared()
                            + config.crack_point_offset as f64,
                    ) + noise;
                }
                if crack && crack_distance >= crack_size {
                    volume.set_unless(&config.cannot_replace, pos, volume.world().air);
                } else if shell >= innermost_layer {
                    let alternate = (rng.next_f32() as f64) < config.use_alternate_layer0_chance;
                    let provider = if alternate {
                        &config.alternate_inner_layer
                    } else {
                        &config.inner_layer
                    };
                    let state = provider.state(volume, rng, pos);
                    volume.set_unless(&config.cannot_replace, pos, state);
                    if (!config.placements_require_layer0_alternate || alternate)
                        && (rng.next_f32() as f64) < config.use_potential_placements_chance
                    {
                        crystal_spots.push(pos);
                    }
                } else if shell >= inner_crust {
                    let state = config.middle_layer.state(volume, rng, pos);
                    volume.set_unless(&config.cannot_replace, pos, state);
                } else if shell >= outer_crust {
                    let state = config.outer_layer.state(volume, rng, pos);
                    volume.set_unless(&config.cannot_replace, pos, state);
                }
            }
        }
    }

    for spot in crystal_spots {
        let crystal = &config.inner_placements
            [rng.next_i32_bound(config.inner_placements.len() as i32) as usize];
        for (face, direction) in Direction::all().into_iter().enumerate() {
            let at = spot + direction.normal();
            let target = volume.get(at).0 as usize;
            if !config.cluster_growable.contains(target) {
                continue;
            }
            let waterlogged = usize::from(volume.world().any_source_fluid.contains(target));
            volume.set_unless(
                &config.cannot_replace,
                at,
                crystal.states[face][waterlogged],
            );
            break;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;
    use std::sync::Arc;

    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::noise::normal;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const STONE: VoxelId = VoxelId(1);
    const CALCITE: VoxelId = VoxelId(2);
    const SMOOTH_BASALT: VoxelId = VoxelId(3);
    const AMETHYST: VoxelId = VoxelId(4);
    const BUDDING: VoxelId = VoxelId(5);
    const CLUSTER: VoxelId = VoxelId(6);

    fn amethyst_geode() -> CompiledGeode {
        CompiledGeode {
            filling: StateProvider::Simple(AIR),
            inner_layer: StateProvider::Simple(AMETHYST),
            alternate_inner_layer: StateProvider::Simple(BUDDING),
            middle_layer: StateProvider::Simple(CALCITE),
            outer_layer: StateProvider::Simple(SMOOTH_BASALT),
            inner_placements: vec![GeodeCrystal {
                states: [[CLUSTER; 2]; 6],
            }],
            cannot_replace: StateMask::default(),
            invalid_blocks: StateMask::default(),
            cluster_growable: mask_of([AIR]),
            use_potential_placements_chance: 0.35,
            use_alternate_layer0_chance: 0.083,
            placements_require_layer0_alternate: true,
            outer_wall_distance: IntProvider::uniform(4, 6),
            outer_wall_distance_max: 6,
            distribution_points: IntProvider::uniform(3, 4),
            point_offset: IntProvider::uniform(1, 2),
            min_gen_offset: -16,
            max_gen_offset: 16,
            noise_multiplier: 0.05,
            invalid_blocks_threshold: 1,
            filling_thickness: 1.7,
            inner_layer_thickness: 2.2,
            middle_layer_thickness: 3.2,
            outer_layer_thickness: 4.2,
            base_crack_size: 2.0,
            generate_crack_chance: 0.95,
            crack_point_offset: 2,
            noise: Arc::new(normal::create_parity(
                -4,
                &[1.0],
                &mut LegacyRandom::new(12345),
            )),
        }
    }

    const ORIGIN: BlockPos = BlockPos::new(0, 30, 0);

    fn solid_rock() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in -24..=24 {
            for z in -24..=24 {
                for y in 0..=60 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        volume
    }

    /// Two invalid distribution points end the run without a `point_offset`
    /// draw for the second: three per point, then the offset only when the
    /// point survives.
    #[test]
    fn an_invalid_point_ends_the_run_before_its_offset() {
        let mut volume = FakeVolume::default();
        let config = amethyst_geode();
        let mut rng = XoroshiroRandom::new(11);
        let mut replay = rng.clone();
        assert!(!place_geode(&config, &mut volume, &mut rng, ORIGIN));

        let points = config.distribution_points.sample(&mut replay);
        assert!(points >= 3);
        replay.next_f64();
        replay.next_f32();
        // The first point is air and inside the threshold, so it still buds.
        for _ in 0..3 {
            config.outer_wall_distance.sample(&mut replay);
        }
        config.point_offset.sample(&mut replay);
        // The second is air too, which trips the threshold three draws in.
        for _ in 0..3 {
            config.outer_wall_distance.sample(&mut replay);
        }
        assert_eq!(rng, replay, "the rejected point spends no offset draw");
    }

    #[test]
    fn a_geode_in_rock_lays_all_four_layers() {
        let mut volume = solid_rock();
        let mut rng = XoroshiroRandom::new(0x0009_e0de);
        assert!(place_geode(
            &amethyst_geode(),
            &mut volume,
            &mut rng,
            ORIGIN
        ));
        for expected in [SMOOTH_BASALT, CALCITE, AMETHYST, AIR] {
            assert!(
                volume.writes.iter().any(|(_, state)| *state == expected),
                "no {expected:?} in the shell"
            );
        }
    }

    #[test]
    fn crystals_only_grow_where_the_alternate_layer_went_in() {
        let mut volume = solid_rock();
        let mut config = amethyst_geode();
        config.use_alternate_layer0_chance = 0.0;
        config.placements_require_layer0_alternate = true;
        let mut rng = XoroshiroRandom::new(0x0009_e0de);
        place_geode(&config, &mut volume, &mut rng, ORIGIN);
        assert!(
            !volume.writes.iter().any(|(_, state)| *state == CLUSTER),
            "no budding block means no crystal"
        );
    }

    /// Self-recorded: the ladder is read off `GeodeFeature`, the number off
    /// this implementation.
    #[test]
    fn geode_draw_count_anchor() {
        let mut volume = solid_rock();
        let mut rng = XoroshiroRandom::new(0x0009_e0de);
        place_geode(&amethyst_geode(), &mut volume, &mut rng, ORIGIN);
        assert_eq!(rng.next_java_long(), GEODE_PIN);
    }

    const GEODE_PIN: i64 = 5359407492119935935;
}
