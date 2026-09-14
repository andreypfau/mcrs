use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
pub use mcrs_minecraft_worldgen::feature::proto::EndSpike;

use crate::block_entity::{EndGatewayData, GeneratedBlockEntity};
use mcrs_voxel_storage::VoxelId;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEndPlatform {
    pub obsidian: VoxelId,
}

pub fn place_end_platform<W: WorldGenVolume>(
    config: &CompiledEndPlatform,
    volume: &mut W,
    at: BlockPos,
) -> bool {
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..3 {
                let pos = at + IVec3::new(dx, dy, dz);
                let target = if dy == -1 {
                    config.obsidian
                } else {
                    volume.world().air
                };
                if volume.get(pos) != target {
                    volume.set(pos, target);
                }
            }
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledVoidStartPlatform {
    pub stone: VoxelId,
    pub cobblestone: VoxelId,
}

const VOID_PLATFORM_OFFSET: IVec3 = IVec3::new(8, 3, 8);
const VOID_PLATFORM_RADIUS: i32 = 16;

fn checkerboard_distance(xa: i32, za: i32, xb: i32, zb: i32) -> i32 {
    (xa - xb).abs().max((za - zb).abs())
}

pub fn place_void_start_platform<W: WorldGenVolume>(
    config: &CompiledVoidStartPlatform,
    volume: &mut W,
    at: BlockPos,
) -> bool {
    let (chunk_x, chunk_z) = (at.x >> 4, at.z >> 4);
    if checkerboard_distance(chunk_x, chunk_z, 0, 0) > 1 {
        return true;
    }
    let y = at.y + VOID_PLATFORM_OFFSET.y;
    for z in chunk_z * 16..=chunk_z * 16 + 15 {
        for x in chunk_x * 16..=chunk_x * 16 + 15 {
            if checkerboard_distance(VOID_PLATFORM_OFFSET.x, VOID_PLATFORM_OFFSET.z, x, z)
                <= VOID_PLATFORM_RADIUS
            {
                let state = if x == VOID_PLATFORM_OFFSET.x && z == VOID_PLATFORM_OFFSET.z {
                    config.cobblestone
                } else {
                    config.stone
                };
                volume.set(BlockPos::new(x, y, z), state);
            }
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEndPodium {
    pub active: bool,
    pub bedrock: VoxelId,
    pub end_stone: VoxelId,
    pub end_portal: VoxelId,
    /// The four wall torch states, in `Direction::HORIZONTAL` order.
    pub wall_torch: [VoxelId; 4],
}

const PODIUM_RIM_RADIUS_SQR: f64 = 2.5 * 2.5;
const PODIUM_RADIUS_SQR: f64 = 3.5 * 3.5;

pub fn place_end_podium<W: WorldGenVolume>(
    config: &CompiledEndPodium,
    volume: &mut W,
    at: BlockPos,
) -> bool {
    for z in at.z - 4..=at.z + 4 {
        for y in at.y - 1..=at.y + 32 {
            for x in at.x - 4..=at.x + 4 {
                let pos = BlockPos::new(x, y, z);
                let distance = (pos - at).length_squared() as f64;
                let inside_rim = distance < PODIUM_RIM_RADIUS_SQR;
                if !inside_rim && distance >= PODIUM_RADIUS_SQR {
                    continue;
                }
                if y < at.y {
                    if inside_rim {
                        volume.set(pos, config.bedrock);
                    } else {
                        set_replacing(config.active, volume, pos, config.end_stone);
                    }
                } else if y > at.y {
                    set_replacing(config.active, volume, pos, volume.world().air);
                } else if !inside_rim {
                    volume.set(pos, config.bedrock);
                } else if config.active {
                    set_replacing(true, volume, pos, config.end_portal);
                } else {
                    volume.set(pos, volume.world().air);
                }
            }
        }
    }

    for dy in 0..4 {
        volume.set(at + IVec3::new(0, dy, 0), config.bedrock);
    }

    let center_of_pillar = at + IVec3::new(0, 2, 0);
    for (face, side) in Direction::HORIZONTAL.iter().enumerate() {
        volume.set(center_of_pillar + side.normal(), config.wall_torch[face]);
    }
    true
}

/// `dropPreviousAndSetBlock` when `replacing`, which skips a cell that already
/// holds the block; the drop itself is a level effect worldgen never sees.
fn set_replacing<W: WorldGenVolume>(
    replacing: bool,
    volume: &mut W,
    pos: BlockPos,
    state: VoxelId,
) {
    if replacing && volume.get(pos) == state {
        return;
    }
    volume.set(pos, state);
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEndGateway {
    pub exit: Option<[i32; 3]>,
    pub exact: bool,
    pub gateway: VoxelId,
    pub bedrock: VoxelId,
}

pub fn place_end_gateway<W: WorldGenVolume>(
    config: &CompiledEndGateway,
    volume: &mut W,
    block_entities: &mut Vec<GeneratedBlockEntity>,
    at: BlockPos,
) -> bool {
    for z in at.z - 1..=at.z + 1 {
        for y in at.y - 2..=at.y + 2 {
            for x in at.x - 1..=at.x + 1 {
                let same_x = x == at.x;
                let same_y = y == at.y;
                let same_z = z == at.z;
                let end = (y - at.y).abs() == 2;
                if same_x && same_y && same_z {
                    volume.set(BlockPos::new(x, y, z), config.gateway);
                    if let Some(exit) = config.exit {
                        block_entities.push(GeneratedBlockEntity::EndGateway(EndGatewayData {
                            x,
                            y,
                            z,
                            age: 0,
                            exit_portal: Some(exit),
                            exact_teleport: config.exact,
                        }));
                    }
                } else if same_y {
                    volume.set(BlockPos::new(x, y, z), volume.world().air);
                } else if (end && same_x && same_z) || ((same_x || same_z) && !end) {
                    volume.set(BlockPos::new(x, y, z), config.bedrock);
                } else {
                    volume.set(BlockPos::new(x, y, z), volume.world().air);
                }
            }
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEndIsland {
    pub end_stone: VoxelId,
}

pub fn place_end_island<W: WorldGenVolume>(
    config: &CompiledEndIsland,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let mut size = rng.next_i32_bound(3) as f32 + 4.0;
    let mut y = 0;
    while size > 0.5 {
        let low = (-size).floor() as i32;
        let high = size.ceil() as i32;
        for x in low..=high {
            for z in low..=high {
                if (x * x + z * z) as f32 <= (size + 1.0) * (size + 1.0) {
                    volume.set(at + IVec3::new(x, y, z), config.end_stone);
                }
            }
        }
        size -= rng.next_i32_bound(2) as f32 + 0.5;
        y -= 1;
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledEndSpikes {
    pub spikes: Vec<EndSpike>,
    pub obsidian: VoxelId,
    pub bedrock: VoxelId,
    pub fire: VoxelId,
    /// Iron bars indexed by `north << 3 | south << 2 | west << 1 | east`.
    pub iron_bars: [VoxelId; 16],
}

const NUMBER_OF_SPIKES: i32 = 10;
const SPIKE_DISTANCE: f64 = 42.0;

/// The ring the End generates when the feature carries no explicit list: ten
/// sizes shuffled from the world seed, laid on a circle of radius 42.
///
/// The shuffle is keyed by `nextLong() & 65535` of a legacy source over the
/// world seed, so only 65536 rings exist however many seeds there are.
pub fn seed_spikes(world_seed: i64) -> Vec<EndSpike> {
    let key = LegacyRandom::new(world_seed as u64).next_java_long() & 65535;
    let mut rng = LegacyRandom::new(key as u64);
    let mut sizes: Vec<i32> = (0..NUMBER_OF_SPIKES).collect();
    mcrs_minecraft_random::shuffle(&mut sizes, &mut rng);

    (0..NUMBER_OF_SPIKES)
        .map(|index| {
            let angle = 2.0
                * (-std::f64::consts::PI
                    + (std::f64::consts::PI / NUMBER_OF_SPIKES as f64) * index as f64);
            let size = sizes[index as usize];
            EndSpike {
                center_x: (SPIKE_DISTANCE * angle.cos()).floor() as i32,
                center_z: (SPIKE_DISTANCE * angle.sin()).floor() as i32,
                radius: 2 + size / 3,
                height: 76 + size * 3,
                guarded: size == 1 || size == 2,
            }
        })
        .collect()
}

pub fn place_end_spike<W: WorldGenVolume>(
    config: &CompiledEndSpikes,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    for spike in &config.spikes {
        if spike.center_x >> 4 == at.x >> 4 && spike.center_z >> 4 == at.z >> 4 {
            place_spike(config, spike, volume, rng);
        }
    }
    true
}

fn place_spike<W: WorldGenVolume>(
    config: &CompiledEndSpikes,
    spike: &EndSpike,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
) {
    let radius = spike.radius;
    let min_y = volume.extent().min_y.min(spike.height + 10);
    let max_y = volume.extent().min_y.max(spike.height + 10);
    for z in spike.center_z - radius..=spike.center_z + radius {
        for y in min_y..=max_y {
            for x in spike.center_x - radius..=spike.center_x + radius {
                let dx = (x - spike.center_x) as f64;
                let dz = (z - spike.center_z) as f64;
                if dx * dx + dz * dz <= (radius * radius + 1) as f64 && y < spike.height {
                    volume.set(BlockPos::new(x, y, z), config.obsidian);
                } else if y > 65 {
                    volume.set(BlockPos::new(x, y, z), volume.world().air);
                }
            }
        }
    }

    if spike.guarded {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                for dy in 0..=3 {
                    let top = dy == 3;
                    if dx.abs() != 2 && dz.abs() != 2 && !top {
                        continue;
                    }
                    let x_edge = dx == -2 || dx == 2 || top;
                    let z_edge = dz == -2 || dz == 2 || top;
                    let bars = iron_bars_index(
                        x_edge && dz != -2,
                        x_edge && dz != 2,
                        z_edge && dx != -2,
                        z_edge && dx != 2,
                    );
                    volume.set(
                        BlockPos::new(spike.center_x + dx, spike.height + dy, spike.center_z + dz),
                        config.iron_bars[bars],
                    );
                }
            }
        }
    }

    // The crystal's yaw: an entity worldgen has no channel for, whose draw
    // still has to be spent so nothing after it moves.
    let _yaw = rng.next_f32() * 360.0;
    volume.set(
        BlockPos::new(spike.center_x, spike.height, spike.center_z),
        config.bedrock,
    );
    // The cell below the fire is the bedrock written on the line above, which is
    // sturdy upwards, so `BaseFireBlock.getState` never reaches its neighbour scan.
    volume.set(
        BlockPos::new(spike.center_x, spike.height + 1, spike.center_z),
        config.fire,
    );
}

pub fn iron_bars_index(north: bool, south: bool, west: bool, east: bool) -> usize {
    (north as usize) << 3 | (south as usize) << 2 | (west as usize) << 1 | (east as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_voxel_storage::Blocks;

    const OBSIDIAN: VoxelId = VoxelId(1);
    const BEDROCK: VoxelId = VoxelId(2);
    const END_STONE: VoxelId = VoxelId(3);
    const END_PORTAL: VoxelId = VoxelId(4);
    const GATEWAY: VoxelId = VoxelId(5);
    const STONE: VoxelId = VoxelId(6);
    const COBBLESTONE: VoxelId = VoxelId(7);
    const FIRE: VoxelId = VoxelId(8);
    const TORCH: [VoxelId; 4] = [VoxelId(10), VoxelId(11), VoxelId(12), VoxelId(13)];

    fn bars() -> [VoxelId; 16] {
        std::array::from_fn(|i| VoxelId(100 + i as u16))
    }

    /// The stamp is a 5×5 obsidian floor one below the origin with three air
    /// layers over it, and a cell already holding the target is left alone —
    /// so over empty space only the floor is written.
    #[test]
    fn an_end_platform_floors_five_by_five_and_skips_what_already_matches() {
        let config = CompiledEndPlatform { obsidian: OBSIDIAN };
        let mut volume = FakeVolume::default();
        volume.blocks.insert((1, -1, 1), OBSIDIAN);
        volume.blocks.insert((0, 1, 0), STONE);
        assert!(place_end_platform(
            &config,
            &mut volume,
            BlockPos::new(0, 0, 0)
        ));

        assert_eq!(volume.get(BlockPos::new(2, -1, -2)), OBSIDIAN);
        assert_eq!(
            volume.get(BlockPos::new(0, 1, 0)),
            AIR,
            "the stone over the floor is cleared"
        );
        assert_eq!(
            volume.writes.len(),
            25,
            "twenty-four floor cells plus the one air cell that differed"
        );
        assert!(
            volume
                .writes
                .windows(2)
                .all(|pair| (pair[0].0.2, pair[0].0.0, pair[0].0.1)
                    < (pair[1].0.2, pair[1].0.0, pair[1].0.1)),
            "z outermost, then x, then y: {:?}",
            volume.writes
        );
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, y, z), _)| (-2..=2).contains(x)
                    && (-2..=2).contains(z)
                    && (-1..3).contains(y))
        );
    }

    /// Every write stays inside one column of the origin, the footprint the
    /// scheduler bounds a feature to.
    #[test]
    fn the_fixed_stamps_stay_inside_one_column_of_their_origin() {
        let mut volume = FakeVolume::default();
        let at = BlockPos::new(100, 49, 0);
        place_end_platform(&CompiledEndPlatform { obsidian: OBSIDIAN }, &mut volume, at);
        place_end_podium(&podium(false), &mut volume, at);
        for ((x, _, z), _) in &volume.writes {
            assert!((x - at.x).abs() <= 15 && (z - at.z).abs() <= 15, "{x} {z}");
        }
    }

    fn podium(active: bool) -> CompiledEndPodium {
        CompiledEndPodium {
            active,
            bedrock: BEDROCK,
            end_stone: END_STONE,
            end_portal: END_PORTAL,
            wall_torch: TORCH,
        }
    }

    /// The pillar is four bedrock high with a torch on each side of its third
    /// cell, the rim is bedrock at the origin's level, and an inactive podium
    /// leaves air where an active one puts the portal.
    #[test]
    fn an_end_podium_builds_its_pillar_rim_and_torches() {
        let mut volume = FakeVolume::default();
        assert!(place_end_podium(
            &podium(false),
            &mut volume,
            BlockPos::new(0, 0, 0)
        ));

        for y in 0..4 {
            assert_eq!(volume.get(BlockPos::new(0, y, 0)), BEDROCK, "pillar at {y}");
        }
        for (face, side) in Direction::HORIZONTAL.iter().enumerate() {
            let pos = BlockPos::new(0, 2, 0) + side.normal();
            assert_eq!(volume.get(pos), TORCH[face]);
        }
        assert_eq!(volume.get(BlockPos::new(3, 0, 0)), BEDROCK, "the rim ring");
        assert_eq!(
            volume.get(BlockPos::new(1, 0, 0)),
            AIR,
            "inactive: air inside the rim"
        );
        assert_eq!(
            volume.get(BlockPos::new(1, -1, 0)),
            BEDROCK,
            "the bedrock floor under it"
        );
        assert_eq!(
            volume.get(BlockPos::new(3, -1, 0)),
            END_STONE,
            "end stone under the rim"
        );

        let mut active = FakeVolume::default();
        place_end_podium(&podium(true), &mut active, BlockPos::new(0, 0, 0));
        assert_eq!(active.get(BlockPos::new(1, 0, 0)), END_PORTAL);
    }

    /// An active podium re-reads before writing, so a cell that already holds
    /// the target block is skipped; an inactive one writes unconditionally.
    #[test]
    fn an_active_podium_skips_cells_that_already_match() {
        let mut volume = FakeVolume::default();
        volume.blocks.insert((1, 3, 0), AIR);
        place_end_podium(&podium(true), &mut volume, BlockPos::new(0, 0, 0));
        let rewrote = volume.writes.iter().any(|(pos, _)| *pos == (1, 3, 0));
        assert!(!rewrote, "an air cell above an active podium stays put");

        let mut inactive = FakeVolume::default();
        place_end_podium(&podium(false), &mut inactive, BlockPos::new(0, 0, 0));
        assert!(inactive.writes.iter().any(|(pos, _)| *pos == (1, 3, 0)));
    }

    fn gateway(exit: Option<[i32; 3]>, exact: bool) -> CompiledEndGateway {
        CompiledEndGateway {
            exit,
            exact,
            gateway: GATEWAY,
            bedrock: BEDROCK,
        }
    }

    /// The 3×5×3 stamp: the gateway block at the origin, bedrock in the arms
    /// above and below it, air on the origin's own layer and in the corners.
    #[test]
    fn an_end_gateway_stamps_its_frame_and_records_its_exit() {
        let mut volume = FakeVolume::default();
        let mut gateways = Vec::new();
        assert!(place_end_gateway(
            &gateway(Some([100, 50, 0]), true),
            &mut volume,
            &mut gateways,
            BlockPos::new(0, 0, 0)
        ));

        assert_eq!(volume.get(BlockPos::new(0, 0, 0)), GATEWAY);
        assert_eq!(
            volume.get(BlockPos::new(1, 0, 0)),
            AIR,
            "the origin's own layer is air"
        );
        assert_eq!(volume.get(BlockPos::new(0, 2, 0)), BEDROCK, "the cap");
        assert_eq!(volume.get(BlockPos::new(0, -2, 0)), BEDROCK, "the base");
        assert_eq!(
            volume.get(BlockPos::new(1, 1, 0)),
            BEDROCK,
            "an arm one off the axis"
        );
        assert_eq!(
            volume.get(BlockPos::new(1, 2, 1)),
            AIR,
            "a corner of the cap"
        );
        assert_eq!(volume.writes.len(), 3 * 5 * 3);
        assert_eq!(
            gateways,
            vec![GeneratedBlockEntity::EndGateway(EndGatewayData {
                x: 0,
                y: 0,
                z: 0,
                age: 0,
                exit_portal: Some([100, 50, 0]),
                exact_teleport: true,
            })]
        );
    }

    /// A gateway with no exit writes the same blocks and no block entity: the
    /// level searches for its exit when the gateway first ticks.
    #[test]
    fn a_delayed_gateway_records_no_block_entity() {
        let mut volume = FakeVolume::default();
        let mut gateways = Vec::new();
        place_end_gateway(
            &gateway(None, false),
            &mut volume,
            &mut gateways,
            BlockPos::new(0, 0, 0),
        );
        assert_eq!(volume.writes.len(), 3 * 5 * 3);
        assert!(gateways.is_empty());
    }

    /// `Age` always, `exit_portal` only when known, `ExactTeleport` only when
    /// set, through the encoder the save and the chunk packet both use.
    #[test]
    fn the_gateway_block_entity_encodes_the_reference_keys() {
        let known = EndGatewayData {
            x: 1,
            y: 2,
            z: 3,
            age: 0,
            exit_portal: Some([100, 50, 0]),
            exact_teleport: true,
        };
        let encoded = mcrs_minecraft_nbt::tag_serializer::to_nbt_compound(&known).unwrap();
        assert_eq!(encoded.get_long("Age"), Some(0));
        assert_eq!(encoded.get_bool("ExactTeleport"), Some(true));
        assert!(encoded.get("exit_portal").is_some());

        let delayed = EndGatewayData {
            exit_portal: None,
            exact_teleport: false,
            ..known.clone()
        };
        let encoded = mcrs_minecraft_nbt::tag_serializer::to_nbt_compound(&delayed).unwrap();
        assert_eq!(encoded.get_long("Age"), Some(0));
        assert!(encoded.get("exit_portal").is_none());
        assert!(encoded.get("ExactTeleport").is_none());
    }

    /// One draw for the starting radius and one per layer, the layers descending
    /// from the origin, and every cell inside the radius the layer carries.
    #[test]
    fn an_end_island_shrinks_a_layer_per_draw() {
        let config = CompiledEndIsland {
            end_stone: END_STONE,
        };
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(4242);
        assert!(place_end_island(
            &config,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 60, 0)
        ));

        let mut replay = XoroshiroRandom::new(4242);
        let mut size = replay.next_i32_bound(3) as f32 + 4.0;
        let mut layers = 0;
        while size > 0.5 {
            layers += 1;
            size -= replay.next_i32_bound(2) as f32 + 0.5;
        }
        assert_eq!(rng, replay, "one draw for the radius, one per layer");
        assert!(layers >= 2);

        let written: std::collections::BTreeSet<i32> =
            volume.writes.iter().map(|((_, y, _), _)| *y).collect();
        assert_eq!(
            written,
            (60 - layers + 1..=60).collect(),
            "one layer per iteration, descending"
        );
        assert_eq!(volume.get(BlockPos::new(0, 60, 0)), END_STONE);
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, _, z), _)| x.abs() <= 7 && z.abs() <= 7)
        );
    }

    /// The reference's ring for a world seed of zero, which is a pure function
    /// of the seed rather than of the feature's own source.
    #[test]
    fn the_seeded_spikes_lie_on_the_radius_forty_two_ring() {
        let spikes = seed_spikes(0);
        assert_eq!(spikes.len(), 10);
        let ring: Vec<(i32, i32)> = spikes.iter().map(|s| (s.center_x, s.center_z)).collect();
        assert_eq!(
            ring,
            vec![
                (42, 0),
                (33, 24),
                (12, 39),
                (-13, 39),
                (-34, 24),
                (-42, -1),
                (-34, -25),
                (-13, -40),
                (12, -40),
                (33, -25),
            ]
        );

        let mut sizes: Vec<i32> = spikes.iter().map(|s| (s.height - 76) / 3).collect();
        sizes.sort();
        assert_eq!(sizes, (0..10).collect::<Vec<_>>(), "a permutation of 0..10");
        for spike in &spikes {
            let size = (spike.height - 76) / 3;
            assert_eq!(spike.radius, 2 + size / 3);
            assert_eq!(spike.guarded, size == 1 || size == 2);
        }
        assert_ne!(seed_spikes(1), spikes, "and it moves with the seed");
    }

    fn spikes_config(spikes: Vec<EndSpike>) -> CompiledEndSpikes {
        CompiledEndSpikes {
            spikes,
            obsidian: OBSIDIAN,
            bedrock: BEDROCK,
            fire: FIRE,
            iron_bars: bars(),
        }
    }

    /// A spike raises an obsidian column to its height, caps it with bedrock and
    /// fire under the crystal, cages it in bars when guarded, and spends exactly
    /// one float on the crystal's yaw.
    #[test]
    fn a_guarded_spike_raises_its_column_and_draws_one_float() {
        let spike = EndSpike {
            center_x: 4,
            center_z: 4,
            radius: 2,
            height: 80,
            guarded: true,
        };
        let config = spikes_config(vec![spike]);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(4242);
        assert!(place_end_spike(
            &config,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 0, 0)
        ));

        let mut replay = XoroshiroRandom::new(4242);
        replay.next_f32();
        assert_eq!(rng, replay, "one float, for the crystal's yaw");

        assert_eq!(
            volume.get(BlockPos::new(4, 0, 4)),
            OBSIDIAN,
            "down to the dimension floor"
        );
        assert_eq!(volume.get(BlockPos::new(4, 79, 4)), OBSIDIAN);
        assert_eq!(
            volume.get(BlockPos::new(4, 80, 4)),
            BEDROCK,
            "under the crystal"
        );
        assert_eq!(volume.get(BlockPos::new(4, 81, 4)), FIRE);
        assert!(
            volume.writes.contains(&((6, 70, 6), AIR)),
            "inside the box but outside the radius, above 65"
        );
        assert_eq!(
            volume.get(BlockPos::new(6, 80, 6)),
            config.iron_bars[iron_bars_index(true, false, true, false)],
            "a cage corner"
        );
        assert_eq!(
            volume.get(BlockPos::new(4, 83, 4)),
            config.iron_bars[15],
            "the cage top"
        );
    }

    /// A spike whose centre falls in another chunk is skipped whole, draws
    /// included, and an ungarded one writes no bars.
    #[test]
    fn a_spike_outside_the_origins_chunk_is_skipped_entirely() {
        let config = spikes_config(vec![EndSpike {
            center_x: 40,
            center_z: 4,
            radius: 2,
            height: 80,
            guarded: false,
        }]);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(4242);
        let before = rng.clone();
        assert!(place_end_spike(
            &config,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 0, 0)
        ));
        assert!(volume.writes.is_empty());
        assert_eq!(rng, before);

        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(4242);
        place_end_spike(&config, &mut volume, &mut rng, BlockPos::new(32, 0, 0));
        assert_ne!(rng, before, "the spike in this chunk draws its yaw");
        assert!(
            !volume
                .writes
                .iter()
                .any(|(_, state)| config.iron_bars.contains(state)),
            "an unguarded spike has no cage"
        );
    }

    /// The platform covers the nine chunks around the origin chunk and nothing
    /// further, with one cobblestone marker at (8, 8).
    #[test]
    fn the_void_start_platform_covers_its_nine_chunks() {
        let config = CompiledVoidStartPlatform {
            stone: STONE,
            cobblestone: COBBLESTONE,
        };
        let mut volume = FakeVolume::default();
        assert!(place_void_start_platform(
            &config,
            &mut volume,
            BlockPos::new(0, 60, 0)
        ));
        assert_eq!(volume.writes.len(), 16 * 16);
        assert_eq!(volume.get(BlockPos::new(8, 63, 8)), COBBLESTONE);
        assert_eq!(volume.get(BlockPos::new(0, 63, 0)), STONE);

        let mut edge = FakeVolume::default();
        place_void_start_platform(&config, &mut edge, BlockPos::new(16, 60, 16));
        assert_eq!(
            edge.writes.len(),
            9 * 9,
            "the checkerboard radius clips the far corner chunk"
        );

        let mut far = FakeVolume::default();
        place_void_start_platform(&config, &mut far, BlockPos::new(32, 60, 0));
        assert!(far.writes.is_empty());
    }
}
