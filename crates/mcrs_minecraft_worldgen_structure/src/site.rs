use mcrs_minecraft_core::ColumnPos;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::PoolAlias;
use crate::hardcoded;
use crate::piece::Piece;
use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::value_provider::{HeightContext, pick_weighted_by};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, block_pos_seed, shuffle};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{BiomeMask, WorldStates};
use mcrs_minecraft_worldgen_feature::template::{JigsawBlock, Joint, bounding_box, transform};

use super::frozen::{
    ElementId, FrozenElement, FrozenStructure, FrozenStructures, PoolId, StructureKind, TemplateId,
};

pub trait SiteWorld {
    fn biome_at(&mut self, block: IVec3) -> Option<u32>;
    /// `couldStructureExistInColumn`: some biome sampled at quart resolution
    /// in the column between the two block heights, inclusive, is in `biomes`.
    fn column_admits(
        &mut self,
        x: i32,
        z: i32,
        min_block_y: i32,
        max_block_y: i32,
        biomes: &BiomeMask,
    ) -> bool;
    /// `getBiomesWithin`: every biome sampled at quart resolution in the cube
    /// of `radius` around `centre` is in `biomes`.
    fn all_biomes_within(&mut self, centre: IVec3, radius: i32, biomes: &BiomeMask) -> bool;
    fn free_height(&mut self, x: i32, z: i32, heightmap: HeightmapName) -> i32;
    /// `getBaseColumn`: the unfilled terrain column, bottom up.
    fn base_column(&mut self, x: i32, z: i32) -> BaseColumn<'_>;
    /// `Heightmap.Types.isOpaque` of `heightmap` on one state.
    fn opaque(&self, state: VoxelId, heightmap: HeightmapName) -> bool;
    fn states(&self) -> &WorldStates;
}

/// `NoiseColumn`: states from `min_y` up, air outside.
#[derive(Debug, Clone, Copy)]
pub struct BaseColumn<'a> {
    pub min_y: i32,
    pub states: &'a [VoxelId],
    pub air: VoxelId,
}

impl BaseColumn<'_> {
    pub fn block(&self, y: i32) -> VoxelId {
        usize::try_from(y - self.min_y)
            .ok()
            .and_then(|index| self.states.get(index))
            .copied()
            .unwrap_or(self.air)
    }
}

/// `Structure.GenerationContext`: what a site or layout may read.
pub struct Context<'a> {
    pub frozen: &'a FrozenStructures,
    pub structure: &'a FrozenStructure,
    pub chunk: ColumnPos,
    pub seed: i64,
    pub height: HeightContext,
    pub accessor_min_y: i32,
    pub accessor_height: i32,
    pub world: &'a mut dyn SiteWorld,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CentrePiece {
    pub element: ElementId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    pub ground_level_delta: i32,
}

/// What a type's site step decides before the biome test and hands to its
/// layout: the reference's deferred piece builder, as data.
#[derive(Debug, Clone, PartialEq)]
pub enum Stub {
    Jigsaw {
        centre: CentrePiece,
        aliases: BTreeMap<ResourceLocation, PoolId>,
    },
    Rotated(Rotation),
    Portal {
        setup: usize,
        air_pocket: bool,
        template: TemplateId,
        rotation: Rotation,
        mirrored: bool,
    },
    Fossil,
    Mineshaft(Vec<Piece>),
    Plain,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    pub position: IVec3,
    pub biome_ok: bool,
    pub rng: LegacyRandom,
    pub stub: Stub,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedJigsaw<'a> {
    pub block: &'a JigsawBlock,
    pub pos: IVec3,
    pub front: Direction,
    pub top: Direction,
}

impl PlacedJigsaw<'_> {
    /// `None` for the feature element's synthetic jigsaw, which any target matches.
    pub fn name(&self) -> Option<&ResourceLocation> {
        (!std::ptr::eq(self.block, &*FEATURE_JIGSAW)).then_some(&self.block.name)
    }
}

static FEATURE_JIGSAW: LazyLock<JigsawBlock> = LazyLock::new(|| {
    let empty = ResourceLocation::parse("minecraft:empty").unwrap();
    JigsawBlock {
        pos: [0; 3],
        front: Direction::Down,
        top: Direction::South,
        joint: Joint::Rollable,
        name: empty.clone(),
        pool: empty.clone(),
        target: empty,
        placement_priority: 0,
        selection_priority: 0,
        final_state: None,
    }
});

/// `Structure.findValidGenerationPoint`: the type's site step on the chunk's
/// large-feature stream, then the biome of the chosen position.
pub fn site(ctx: &mut Context<'_>) -> Option<Site> {
    let mut rng = LegacyRandom::large_feature(ctx.seed, ctx.chunk.x, ctx.chunk.z);
    let structure = ctx.structure;
    let (position, stub) = match &structure.kind {
        StructureKind::Jigsaw { start_pool, config } => {
            jigsaw_site(ctx, *start_pool, config, &mut rng)?
        }
        StructureKind::BuriedTreasure => hardcoded::buried_treasure::site(ctx, &mut rng)?,
        StructureKind::DesertPyramid => hardcoded::desert_pyramid::site(ctx, &mut rng)?,
        StructureKind::EndCity => hardcoded::end_city::site(ctx, &mut rng)?,
        StructureKind::Fortress => hardcoded::fortress::site(ctx, &mut rng)?,
        StructureKind::Igloo => hardcoded::igloo::site(ctx, &mut rng)?,
        StructureKind::JungleTemple => hardcoded::jungle_temple::site(ctx, &mut rng)?,
        StructureKind::Mineshaft { mineshaft_type, .. } => {
            hardcoded::mineshaft::site(*mineshaft_type, ctx, &mut rng)?
        }
        StructureKind::NetherFossil { height } => {
            hardcoded::nether_fossil::site(height, ctx, &mut rng)?
        }
        StructureKind::OceanMonument { surrounding } => {
            hardcoded::ocean_monument::site(surrounding, ctx, &mut rng)?
        }
        StructureKind::OceanRuin(config) => hardcoded::ocean_ruin::site(config, ctx, &mut rng)?,
        StructureKind::RuinedPortal {
            setups,
            portals,
            giant_portals,
        } => hardcoded::ruined_portal::site(setups, portals, giant_portals, ctx, &mut rng)?,
        StructureKind::Shipwreck { is_beached } => {
            hardcoded::shipwreck::site(*is_beached, ctx, &mut rng)?
        }
        StructureKind::Stronghold => hardcoded::stronghold::site(ctx, &mut rng)?,
        StructureKind::SwampHut => hardcoded::swamp_hut::site(ctx, &mut rng)?,
        StructureKind::WoodlandMansion => hardcoded::woodland_mansion::site(ctx, &mut rng)?,
    };
    let biome_ok = ctx
        .world
        .biome_at(position)
        .is_some_and(|biome| structure.biomes.contains(biome as usize));
    Some(Site {
        position,
        biome_ok,
        rng,
        stub,
    })
}

/// The type's deferred piece builder run on a site that passed.
pub fn layout(ctx: &mut Context<'_>, site: Site) -> Vec<Piece> {
    let structure = ctx.structure;
    match &structure.kind {
        StructureKind::Jigsaw { config, .. } => crate::jigsaw::layout(ctx, config, site)
            .into_iter()
            .map(Piece::Jigsaw)
            .collect(),
        StructureKind::BuriedTreasure => hardcoded::buried_treasure::layout(ctx, site),
        StructureKind::DesertPyramid => hardcoded::desert_pyramid::layout(ctx, site),
        StructureKind::EndCity => hardcoded::end_city::layout(ctx, site),
        StructureKind::Fortress => hardcoded::fortress::layout(ctx, site),
        StructureKind::Igloo => hardcoded::igloo::layout(ctx, site),
        StructureKind::JungleTemple => hardcoded::jungle_temple::layout(ctx, site),
        StructureKind::Mineshaft { mineshaft_type, .. } => {
            hardcoded::mineshaft::layout(*mineshaft_type, ctx, site)
        }
        StructureKind::NetherFossil { .. } => hardcoded::nether_fossil::layout(ctx, site),
        StructureKind::OceanMonument { .. } => hardcoded::ocean_monument::layout(ctx, site),
        StructureKind::OceanRuin(config) => hardcoded::ocean_ruin::layout(config, ctx, site),
        StructureKind::RuinedPortal { setups, .. } => {
            hardcoded::ruined_portal::layout(setups, ctx, site)
        }
        StructureKind::Shipwreck { is_beached } => {
            hardcoded::shipwreck::layout(*is_beached, ctx, site)
        }
        StructureKind::Stronghold => hardcoded::stronghold::layout(ctx, site),
        StructureKind::SwampHut => hardcoded::swamp_hut::layout(ctx, site),
        StructureKind::WoodlandMansion => hardcoded::woodland_mansion::layout(ctx, site),
    }
}

/// Whether a site that passed always yields a piece, so a search need not run
/// the layout; `None` for a type this build has no generator for, whose layout
/// is empty and whose site never passes.
pub fn site_implies_piece(kind: &StructureKind) -> Option<bool> {
    match kind {
        StructureKind::Jigsaw { .. } => Some(true),
        StructureKind::BuriedTreasure => hardcoded::buried_treasure::SITE_IMPLIES_PIECE,
        StructureKind::DesertPyramid => hardcoded::desert_pyramid::SITE_IMPLIES_PIECE,
        StructureKind::EndCity => hardcoded::end_city::SITE_IMPLIES_PIECE,
        StructureKind::Fortress => hardcoded::fortress::SITE_IMPLIES_PIECE,
        StructureKind::Igloo => hardcoded::igloo::SITE_IMPLIES_PIECE,
        StructureKind::JungleTemple => hardcoded::jungle_temple::SITE_IMPLIES_PIECE,
        StructureKind::Mineshaft { .. } => hardcoded::mineshaft::SITE_IMPLIES_PIECE,
        StructureKind::NetherFossil { .. } => hardcoded::nether_fossil::SITE_IMPLIES_PIECE,
        StructureKind::OceanMonument { .. } => hardcoded::ocean_monument::SITE_IMPLIES_PIECE,
        StructureKind::OceanRuin(_) => hardcoded::ocean_ruin::SITE_IMPLIES_PIECE,
        StructureKind::RuinedPortal { .. } => hardcoded::ruined_portal::SITE_IMPLIES_PIECE,
        StructureKind::Shipwreck { .. } => hardcoded::shipwreck::SITE_IMPLIES_PIECE,
        StructureKind::Stronghold => hardcoded::stronghold::SITE_IMPLIES_PIECE,
        StructureKind::SwampHut => hardcoded::swamp_hut::SITE_IMPLIES_PIECE,
        StructureKind::WoodlandMansion => hardcoded::woodland_mansion::SITE_IMPLIES_PIECE,
    }
}

fn jigsaw_site(
    ctx: &mut Context<'_>,
    start_pool: PoolId,
    config: &crate::JigsawConfig,
    rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let frozen = ctx.frozen;
    let structure = ctx.structure;
    let chunk = ctx.chunk;
    let start_y = config.start_height.sample(rng, ctx.height);
    let start_pos = IVec3::new(chunk.min_block_x(), start_y, chunk.min_block_z());

    let mut aliases = BTreeMap::new();
    if !config.pool_aliases.is_empty() {
        let mut alias_rng = LegacyRandom::new(ctx.seed as u64).fork_at(start_pos);
        resolve_aliases(frozen, &config.pool_aliases, &mut alias_rng, &mut aliases);
    }

    let rotation = Rotation::ALL[rng.next_i32_bound(4) as usize];
    let pool = aliases
        .get(&config.start_pool)
        .copied()
        .unwrap_or(start_pool);
    let expanded = &frozen.pools[pool.0 as usize].expanded;
    if expanded.is_empty() {
        return None;
    }
    let element = expanded[rng.next_i32_bound(expanded.len() as i32) as usize];
    if matches!(frozen.elements[element.0 as usize], FrozenElement::Empty) {
        return None;
    }

    let anchored = match &config.start_jigsaw_name {
        Some(name) => {
            shuffled_jigsaws(frozen, element, start_pos, rotation, rng)
                .into_iter()
                .find(|jigsaw| jigsaw.name() == Some(name))?
                .pos
        }
        None => start_pos,
    };
    let local_anchor = anchored - start_pos;
    let adjusted = start_pos - local_anchor;
    let bounds = element_bounds(frozen, element, adjusted, rotation)?;
    let centre_x = (bounds.max.x + bounds.min.x) / 2;
    let centre_z = (bounds.max.z + bounds.min.z) / 2;
    let bottom_y = match config.project_start_to_heightmap {
        None => adjusted.y,
        Some(heightmap) => {
            let max_y = ctx.accessor_min_y + ctx.accessor_height - 1;
            if !ctx.world.column_admits(
                centre_x,
                centre_z,
                ctx.accessor_min_y,
                max_y,
                &structure.biomes,
            ) {
                return None;
            }
            start_pos.y + ctx.world.free_height(centre_x, centre_z, heightmap)
        }
    };
    let ground_level_delta = 1;
    let lift = IVec3::new(0, bottom_y - (bounds.min.y + ground_level_delta), 0);
    let centre = CentrePiece {
        element,
        position: adjusted + lift,
        rotation,
        bounds: bounds.moved(lift),
        ground_level_delta,
    };

    let padding = &config.dimension_padding;
    if padding.bottom() != 0 || padding.top() != 0 {
        let floor = ctx.accessor_min_y + padding.bottom();
        let ceiling = ctx.accessor_min_y + ctx.accessor_height - 1 - padding.top();
        if centre.bounds.min.y < floor || centre.bounds.max.y > ceiling {
            return None;
        }
    }

    let position = IVec3::new(centre_x, bottom_y + local_anchor.y, centre_z);
    Some((position, Stub::Jigsaw { centre, aliases }))
}

fn resolve_aliases(
    frozen: &FrozenStructures,
    bindings: &[PoolAlias],
    rng: &mut LegacyRandom,
    out: &mut BTreeMap<ResourceLocation, PoolId>,
) {
    for binding in bindings {
        match binding {
            PoolAlias::Direct { alias, target } => {
                out.insert(alias.clone(), frozen.pool_ids[target]);
            }
            PoolAlias::Random { alias, targets } => {
                let target = pick_weighted_by(targets, |w| w.weight.0, rng)
                    .expect("the freeze keeps only alias lists with weight");
                out.insert(alias.clone(), frozen.pool_ids[&target.data]);
            }
            PoolAlias::RandomGroup { groups } => {
                let group = pick_weighted_by(groups, |w| w.weight.0, rng)
                    .expect("the freeze keeps only alias lists with weight");
                resolve_aliases(frozen, &group.data, rng, out);
            }
        }
    }
}

/// The element's jigsaw blocks at `position` under `rotation`, shuffled by the
/// stream and then stably ordered by selection priority, highest first.
pub fn shuffled_jigsaws<'a>(
    frozen: &'a FrozenStructures,
    element: ElementId,
    position: IVec3,
    rotation: Rotation,
    rng: &mut LegacyRandom,
) -> Vec<PlacedJigsaw<'a>> {
    let mut jigsaws: Vec<PlacedJigsaw<'a>> = match &frozen.elements[element.0 as usize] {
        FrozenElement::Single { template, .. } => {
            let palettes = &frozen.manifests[template.0 as usize].jigsaws;
            if palettes.is_empty() {
                return Vec::new();
            }
            let palette = LegacyRandom::new(block_pos_seed(position))
                .next_i32_bound(palettes.len() as i32) as usize;
            palettes[palette]
                .iter()
                .map(|block| PlacedJigsaw {
                    block,
                    pos: transform(
                        IVec3::from(block.pos.map(i32::from)),
                        Mirror::None,
                        rotation,
                        IVec3::ZERO,
                    ) + position,
                    front: rotation.rotate(block.front),
                    top: rotation.rotate(block.top),
                })
                .collect()
        }
        FrozenElement::List { elements, .. } => {
            return shuffled_jigsaws(frozen, elements[0], position, rotation, rng);
        }
        FrozenElement::Feature { .. } => {
            return vec![PlacedJigsaw {
                block: &FEATURE_JIGSAW,
                pos: position,
                front: Direction::Down,
                top: Direction::South,
            }];
        }
        FrozenElement::Empty => return Vec::new(),
    };
    shuffle(&mut jigsaws, rng);
    jigsaws.sort_by_key(|jigsaw| std::cmp::Reverse(jigsaw.block.selection_priority));
    jigsaws
}

/// The element's box at `position` under `rotation`; `None` for an empty
/// element, whose box the reference refuses to compute.
pub fn element_bounds(
    frozen: &FrozenStructures,
    element: ElementId,
    position: IVec3,
    rotation: Rotation,
) -> Option<BoundingBox> {
    match &frozen.elements[element.0 as usize] {
        FrozenElement::Single { template, .. } => Some(bounding_box(
            frozen.manifests[template.0 as usize].size,
            position,
            rotation,
            Mirror::None,
            IVec3::ZERO,
        )),
        FrozenElement::List { elements, .. } => elements
            .iter()
            .filter_map(|inner| element_bounds(frozen, *inner, position, rotation))
            .reduce(BoundingBox::union),
        FrozenElement::Feature { .. } => Some(BoundingBox::point(position.into())),
        FrozenElement::Empty => None,
    }
}
