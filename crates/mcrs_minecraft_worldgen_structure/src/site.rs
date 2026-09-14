use mcrs_minecraft_core::ColumnPos;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::PoolAlias;
use bevy_math::IVec3;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::value_provider::{HeightContext, pick_weighted_by};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, block_pos_seed, shuffle};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;
use mcrs_minecraft_worldgen_feature::template::{JigsawBlock, Joint, bounding_box, transform};

use super::frozen::{
    ElementId, FrozenElement, FrozenStructures, PoolId, StructureId, StructureKind,
};

pub trait SiteWorld {
    fn biome_at(&mut self, block: IVec3) -> Option<u32>;
    fn column_admits(&mut self, x: i32, z: i32, biomes: &BiomeMask) -> bool;
    fn free_height(&mut self, x: i32, z: i32, heightmap: HeightmapName) -> i32;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CentrePiece {
    pub element: ElementId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    pub ground_level_delta: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    pub position: IVec3,
    pub biome_ok: bool,
    pub centre: CentrePiece,
    pub rng: LegacyRandom,
    pub aliases: BTreeMap<ResourceLocation, PoolId>,
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

#[allow(clippy::too_many_arguments)]
pub fn site(
    frozen: &FrozenStructures,
    structure: StructureId,
    chunk: ColumnPos,
    seed: i64,
    height: HeightContext,
    accessor_min_y: i32,
    accessor_height: i32,
    world: &mut dyn SiteWorld,
) -> Option<Site> {
    let structure = &frozen.structures[structure.0 as usize];
    let StructureKind::Jigsaw { start_pool, config } = &structure.kind else {
        return None;
    };
    let mut rng = LegacyRandom::large_feature(seed, chunk.x, chunk.z);
    let start_y = config.start_height.sample(&mut rng, height);
    let start_pos = IVec3::new(chunk.min_block_x(), start_y, chunk.min_block_z());

    let mut aliases = BTreeMap::new();
    if !config.pool_aliases.is_empty() {
        let mut alias_rng = LegacyRandom::new(seed as u64).fork_at(start_pos);
        resolve_aliases(frozen, &config.pool_aliases, &mut alias_rng, &mut aliases);
    }

    let rotation = Rotation::ALL[rng.next_i32_bound(4) as usize];
    let pool = aliases
        .get(&config.start_pool)
        .copied()
        .unwrap_or(*start_pool);
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
            shuffled_jigsaws(frozen, element, start_pos, rotation, &mut rng)
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
            if !world.column_admits(centre_x, centre_z, &structure.biomes) {
                return None;
            }
            start_pos.y + world.free_height(centre_x, centre_z, heightmap)
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
        let floor = accessor_min_y + padding.bottom();
        let ceiling = accessor_min_y + accessor_height - 1 - padding.top();
        if centre.bounds.min.y < floor || centre.bounds.max.y > ceiling {
            return None;
        }
    }

    let position = IVec3::new(centre_x, bottom_y + local_anchor.y, centre_z);
    let biome_ok = world
        .biome_at(position)
        .is_some_and(|biome| structure.biomes.contains(biome as usize));
    Some(Site {
        position,
        biome_ok,
        centre,
        rng,
        aliases,
    })
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
                    pos: transform(IVec3::from(block.pos.map(i32::from)), rotation, IVec3::ZERO)
                        + position,
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
        )),
        FrozenElement::List { elements, .. } => elements
            .iter()
            .filter_map(|inner| element_bounds(frozen, *inner, position, rotation))
            .reduce(BoundingBox::union),
        FrozenElement::Feature { .. } => Some(BoundingBox::point(position.into())),
        FrozenElement::Empty => None,
    }
}
