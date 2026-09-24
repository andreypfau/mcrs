use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, Direction, HolderSet, ResourceLocation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, states_of,
};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen_feature::proto::{ProcessorRule, StructureProcessor};
use mcrs_minecraft_worldgen_feature::rule_test::RuleTest;
use mcrs_minecraft_worldgen_feature::template::FrozenTemplate;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::template::{ChainKind, CompiledChain, compile_chain};
use mcrs_minecraft_worldgen_structure::RuinedPortalSetup;
use mcrs_minecraft_worldgen_structure::hardcoded::ruined_portal::heightmap;
use mcrs_minecraft_worldgen_structure::piece::{PortalProperties, RuinedPortalPiece};
use mcrs_minecraft_worldgen_structure::{PortalPlacement, PortalPlacement::OnOceanFloor};

use crate::{block_mask, place_positional, state};

const GOLD_GONE: f32 = 0.3;
const MAGMA_INSTEAD_OF_NETHERRACK: f32 = 0.07;
const MAGMA_INSTEAD_OF_LAVA: f32 = 0.2;
const NETHERRACK_BY_DISTANCE: [f32; 14] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.9, 0.9, 0.8, 0.7, 0.6, 0.4, 0.2,
];
const DRIP_CAP: i32 = 8;
const MAX_Y_DIFF: i32 = 3;

/// One structure's tables: a processor chain per piece shape its setups can
/// yield, and the blocks the netherrack spread writes and tests.
#[derive(Clone, Debug)]
pub struct RuinedPortalBlocks {
    chains: Vec<(PortalPlacement, PortalProperties, CompiledChain)>,
    netherrack: VoxelId,
    magma: VoxelId,
    obsidian: VoxelId,
    persistent_jungle_leaves: VoxelId,
    /// Indexed like `Direction::HORIZONTAL`: the vine hanging on the face that
    /// looks back at the block in that direction.
    vine_facing: [VoxelId; 4],
    vines: StateMask,
    features_cannot_replace: StateMask,
    full_collision_face: [StateMask; 4],
}

impl RuinedPortalBlocks {
    pub fn compile(
        setups: &[RuinedPortalSetup],
        blocks: &dyn BlockResolver,
        world_seed: i64,
    ) -> Result<Self, FeatureCompileError> {
        let mut chains = Vec::with_capacity(setups.len() * 4);
        for setup in setups {
            for cold in [false, true] {
                for air_pocket in [false, true] {
                    let properties = PortalProperties {
                        cold,
                        mossiness: setup.mossiness.0 as f32,
                        air_pocket,
                        overgrown: setup.overgrown,
                        vines: setup.vines,
                        replace_with_blackstone: setup.replace_with_blackstone,
                    };
                    let chain = compile_chain(
                        &processors(setup.placement, &properties),
                        ChainKind::Feature,
                        blocks,
                        world_seed,
                    )?;
                    chains.push((setup.placement, properties, chain));
                }
            }
        }
        let vine = |face: Direction| state(blocks, "minecraft:vine", &[(face.name(), "true")]);
        let full_face = |face: Direction| states_of(blocks, StateQuery::FullCollisionFace(face));
        Ok(RuinedPortalBlocks {
            chains,
            netherrack: state(blocks, "minecraft:netherrack", &[])?,
            magma: state(blocks, "minecraft:magma_block", &[])?,
            obsidian: state(blocks, "minecraft:obsidian", &[])?,
            persistent_jungle_leaves: state(
                blocks,
                "minecraft:jungle_leaves",
                &[("persistent", "true")],
            )?,
            vine_facing: [
                vine(Direction::South)?,
                vine(Direction::West)?,
                vine(Direction::North)?,
                vine(Direction::East)?,
            ],
            vines: block_mask(blocks, &["minecraft:vine"])?,
            features_cannot_replace: states_of(
                blocks,
                StateQuery::BlockTag(&ResourceLocation::minecraft("features_cannot_replace")),
            )?,
            full_collision_face: [
                full_face(Direction::North)?,
                full_face(Direction::East)?,
                full_face(Direction::South)?,
                full_face(Direction::West)?,
            ],
        })
    }

    fn chain(&self, piece: &RuinedPortalPiece) -> Option<&CompiledChain> {
        self.chains
            .iter()
            .find(|(placement, properties, _)| {
                *placement == piece.placement && *properties == piece.properties
            })
            .map(|(_, _, chain)| chain)
    }
}

/// `RuinedPortalPiece.makeSettings`' processor list.
fn processors(placement: PortalPlacement, p: &PortalProperties) -> Vec<StructureProcessor> {
    let block = |name: &str| BlockState {
        name: ResourceLocation::minecraft(name),
        properties: None,
    };
    let replace = |source: &str, probability: Option<f32>, target: &str| ProcessorRule {
        input_predicate: match probability {
            Some(probability) => RuleTest::RandomBlockMatch {
                block: ResourceLocation::minecraft(source),
                probability,
            },
            None => RuleTest::BlockMatch {
                block: ResourceLocation::minecraft(source),
            },
        },
        location_predicate: RuleTest::AlwaysTrue,
        position_predicate: None,
        output_state: block(target),
        block_entity_modifier: None,
    };
    let mut rules = vec![replace("gold_block", Some(GOLD_GONE), "air")];
    rules.push(if placement == OnOceanFloor {
        replace("lava", None, "magma_block")
    } else if p.cold {
        replace("lava", None, "netherrack")
    } else {
        replace("lava", Some(MAGMA_INSTEAD_OF_LAVA), "magma_block")
    });
    if !p.cold {
        rules.push(replace(
            "netherrack",
            Some(MAGMA_INSTEAD_OF_NETHERRACK),
            "magma_block",
        ));
    }
    let mut ignored = vec![block("structure_block")];
    if !p.air_pocket {
        ignored.push(block("air"));
    }
    let mut chain = vec![
        StructureProcessor::BlockIgnore { blocks: ignored },
        StructureProcessor::Rule { rules },
        StructureProcessor::BlockAge {
            mossiness: f64::from(p.mossiness),
        },
        StructureProcessor::ProtectedBlocks {
            value: HolderSet::Tag(ResourceLocation::minecraft("features_cannot_replace")),
        },
        StructureProcessor::LavaSubmergedBlock,
    ];
    if p.replace_with_blackstone {
        chain.push(StructureProcessor::BlackstoneReplace);
    }
    chain
}

/// `RuinedPortalPiece.postProcess`: nothing unless the column holds the box
/// centre, then the whole template from that column, netherrack spread within
/// fourteen blocks of the centre, drip columns under the portal, and vines or
/// leaves over the box.
#[allow(clippy::too_many_arguments)]
pub fn place_ruined_portal<W: WorldGenVolume>(
    b: &RuinedPortalBlocks,
    piece: &RuinedPortalPiece,
    template: &FrozenTemplate,
    volume: &mut W,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
    reference: IVec3,
    clip: BoundingBox,
    rng: &mut WorldgenRandom,
) {
    let bounds = piece.bounds;
    if !clip.is_inside(bounds.center()) {
        return;
    }
    let Some(chain) = b.chain(piece) else {
        debug_assert!(
            false,
            "a ruined portal piece no setup of its structure yields"
        );
        return;
    };
    let size = IVec3::from(template.size.map(i32::from));
    place_positional(
        template,
        piece.position,
        piece.rotation,
        piece.mirror,
        IVec3::new(size.x / 2, 0, size.z / 2),
        clip.union(bounds),
        chain,
        true,
        true,
        reference,
        volume,
        rng,
        entities,
        spawns,
    );
    let mut portal = Portal {
        b,
        piece,
        volume,
        rng,
    };
    portal.spread_netherrack();
    portal.drip_columns_below_portal();
    if piece.properties.vines || piece.properties.overgrown {
        for z in bounds.min.z..=bounds.max.z {
            for y in bounds.min.y..=bounds.max.y {
                for x in bounds.min.x..=bounds.max.x {
                    let pos = BlockPos::new(x, y, z);
                    if piece.properties.vines {
                        portal.maybe_add_vines(pos);
                    }
                    if piece.properties.overgrown {
                        portal.maybe_add_leaves_above(pos);
                    }
                }
            }
        }
    }
}

struct Portal<'a, W: WorldGenVolume> {
    b: &'a RuinedPortalBlocks,
    piece: &'a RuinedPortalPiece,
    volume: &'a mut W,
    rng: &'a mut WorldgenRandom,
}

impl<W: WorldGenVolume> Portal<'_, W> {
    fn spread_netherrack(&mut self) {
        let placement = self.piece.placement;
        let bounds = self.piece.bounds;
        let follows_ground = matches!(
            placement,
            PortalPlacement::OnLandSurface | PortalPlacement::OnOceanFloor
        );
        let centre = bounds.center();
        let max_distance = NETHERRACK_BY_DISTANCE.len() as i32;
        let average_width = (bounds.max.x - bounds.min.x + 1 + bounds.max.z - bounds.min.z + 1) / 2;
        let adjustment = self.rng.next_i32_bound((8 - average_width / 2).max(1));
        for x in centre.x - max_distance..=centre.x + max_distance {
            for z in centre.z - max_distance..=centre.z + max_distance {
                let distance = (x - centre.x).abs() + (z - centre.z).abs();
                let adjusted = (distance + adjustment).max(0);
                if adjusted >= max_distance {
                    continue;
                }
                if self.rng.next_f64() >= f64::from(NETHERRACK_BY_DISTANCE[adjusted as usize]) {
                    continue;
                }
                let surface_y = self.volume.height(heightmap(placement), x, z) - 1;
                let y = if follows_ground {
                    surface_y
                } else {
                    bounds.min.y.min(surface_y)
                };
                let pos = BlockPos::new(x, y, z);
                if (y - bounds.min.y).abs() <= MAX_Y_DIFF && self.replaceable_by_netherrack(pos) {
                    self.place_netherrack_or_magma(pos);
                    if self.piece.properties.overgrown {
                        self.maybe_add_leaves_above(pos);
                    }
                    self.drip_column(pos - IVec3::Y);
                }
            }
        }
    }

    fn replaceable_by_netherrack(&self, pos: BlockPos) -> bool {
        let state = self.volume.get(pos);
        let world = self.volume.world();
        state != world.air
            && state != self.b.obsidian
            && !self.b.features_cannot_replace.contains(state.0 as usize)
            && (self.piece.placement == PortalPlacement::InNether
                || !world.lava_states.contains(state.0 as usize))
    }

    fn place_netherrack_or_magma(&mut self, pos: BlockPos) {
        let state =
            if !self.piece.properties.cold && self.rng.next_f32() < MAGMA_INSTEAD_OF_NETHERRACK {
                self.b.magma
            } else {
                self.b.netherrack
            };
        self.volume.set(pos, state);
    }

    fn drip_column(&mut self, mut pos: BlockPos) {
        self.place_netherrack_or_magma(pos);
        let mut remaining = DRIP_CAP;
        while remaining > 0 && self.rng.next_f32() < 0.5 {
            pos = pos - IVec3::Y;
            remaining -= 1;
            self.place_netherrack_or_magma(pos);
        }
    }

    fn drip_columns_below_portal(&mut self) {
        let bounds = self.piece.bounds;
        for x in bounds.min.x + 1..bounds.max.x {
            for z in bounds.min.z + 1..bounds.max.z {
                let pos = BlockPos::new(x, bounds.min.y, z);
                if self.volume.get(pos) == self.b.netherrack {
                    self.drip_column(pos - IVec3::Y);
                }
            }
        }
    }

    fn maybe_add_vines(&mut self, pos: BlockPos) {
        let state = self.volume.get(pos);
        if self.volume.world().air_states.contains(state.0 as usize)
            || self.b.vines.contains(state.0 as usize)
        {
            return;
        }
        let side = self.rng.next_i32_bound(4) as usize;
        let neighbour = pos + Direction::HORIZONTAL[side].normal();
        if self.volume.is_air(neighbour)
            && self.b.full_collision_face[side].contains(state.0 as usize)
        {
            self.volume.set(neighbour, self.b.vine_facing[side]);
        }
    }

    fn maybe_add_leaves_above(&mut self, pos: BlockPos) {
        let above = pos + IVec3::Y;
        if self.rng.next_f32() < 0.5
            && self.volume.get(pos) == self.b.netherrack
            && self.volume.is_air(above)
        {
            self.volume.set(above, self.b.persistent_jungle_leaves);
        }
    }
}
