use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, HolderSet, Mirror, ResourceLocation};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, states_of,
};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen_feature::template::{
    FrozenTemplate, TemplateManifest, data_markers, transform,
};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::template::{
    CompiledChain, CompiledProcessor, Placement, SettingsRandom, place_template,
};
use mcrs_minecraft_worldgen_structure::hardcoded::igloo::IglooTemplate;
use mcrs_minecraft_worldgen_structure::piece::{IglooPiece, ShipwreckPiece};

use crate::{block_mask, state};

fn ignore_blocks(
    blocks: &dyn BlockResolver,
    names: &[&str],
) -> Result<CompiledChain, FeatureCompileError> {
    let ignored = HolderSet::List(
        names
            .iter()
            .map(|n| ResourceLocation::minecraft(n))
            .collect(),
    );
    Ok(vec![CompiledProcessor::BlockIgnore(states_of(
        blocks,
        StateQuery::Blocks(&ignored),
    )?)])
}

/// `BlockIgnoreProcessor.STRUCTURE_AND_AIR` as a chain of its own.
pub fn ignore_structure_and_air(
    blocks: &dyn BlockResolver,
) -> Result<CompiledChain, FeatureCompileError> {
    ignore_blocks(blocks, &["structure_block", "air"])
}

/// `RandomizableContainer.setBlockEntityLootTable`: the container this run
/// placed at `pos`, if any, takes the table and a seed from the stream.
// ponytail: the chest-shaped containers only; a hopper, decorated pot or
// crafter under a marker needs its own arm.
pub fn seed_container_loot(
    entities: &mut [GeneratedBlockEntity],
    pos: BlockPos,
    loot_table: &str,
    rng: &mut XoroshiroRandom,
) {
    let container = entities
        .iter_mut()
        .rev()
        .filter(|entity| entity.position() == pos)
        .find_map(|entity| match entity {
            GeneratedBlockEntity::Chest(container)
            | GeneratedBlockEntity::TrappedChest(container)
            | GeneratedBlockEntity::Barrel(container)
            | GeneratedBlockEntity::Dispenser(container) => Some(container),
            _ => None,
        });
    if let Some(container) = container {
        container.loot_table = Some(loot_table.to_owned());
        container.loot_table_seed = rng.next_i64();
    }
}

/// `ShipwreckPieces.MARKERS_TO_LOOT`.
fn shipwreck_loot(marker: &str) -> Option<&'static str> {
    Some(match marker {
        "map_chest" => "minecraft:chests/shipwreck_map",
        "treasure_chest" => "minecraft:chests/shipwreck_treasure",
        "supply_chest" => "minecraft:chests/shipwreck_supply",
        _ => return None,
    })
}

/// `ShipwreckPiece.postProcess` for one column, the piece already at the
/// height its layout fixed.
#[allow(clippy::too_many_arguments)]
pub fn paint_shipwreck<W: WorldGenVolume>(
    chain: &CompiledChain,
    template: &FrozenTemplate,
    manifest: &TemplateManifest,
    piece: &ShipwreckPiece,
    reference: IVec3,
    clip: BoundingBox,
    region: &mut W,
    rng: &mut XoroshiroRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) {
    if template.palettes.is_empty() {
        return;
    }
    let position = piece.placed_position();
    let palette = LegacyRandom::new(block_pos_seed(position))
        .next_i32_bound(template.palettes.len() as i32) as usize;
    let placed = place_template(
        &Placement {
            template,
            jigsaws: &[],
            palette,
            position,
            reference,
            rotation: piece.rotation,
            mirror: Mirror::None,
            pivot: ShipwreckPiece::PIVOT,
            random: SettingsRandom::Positional,
            clip: Some(clip),
            chain,
            waterlog: true,
            place_entities: true,
        },
        region,
        rng,
        entities,
        spawns,
    );
    if !placed {
        return;
    }
    let markers = manifest.markers.get(palette).map_or(&[][..], Vec::as_slice);
    for (pos, marker) in data_markers(
        markers,
        position,
        Mirror::None,
        piece.rotation,
        ShipwreckPiece::PIVOT,
        Some(clip),
    ) {
        if let Some(loot) = shipwreck_loot(marker) {
            seed_container_loot(entities, (pos - IVec3::Y).into(), loot, rng);
        }
    }
}

#[derive(Clone)]
pub struct IglooBlocks {
    chain: CompiledChain,
    snow_block: VoxelId,
    air: VoxelId,
    ladder: StateMask,
}

impl IglooBlocks {
    pub fn compile(blocks: &dyn BlockResolver) -> Result<Self, FeatureCompileError> {
        Ok(IglooBlocks {
            chain: ignore_blocks(blocks, &["structure_block"])?,
            snow_block: state(blocks, "minecraft:snow_block", &[])?,
            air: state(blocks, "minecraft:air", &[])?,
            ladder: block_mask(blocks, &["minecraft:ladder"])?,
        })
    }
}

/// `IglooPieces.IglooPiece.postProcess` for one column, the piece already at
/// the height its layout fixed: the template with its villagers, the chest
/// under the marker seeded, and over the top's trapdoor a snow block wherever
/// the shaft's ladder is not.
///
/// The trapdoor write is not clipped: the reference writes it from every
/// chunk the top touches, and a neighbour that never placed the shaft sees
/// terrain below the trapdoor and writes snow into the entrance column.
#[allow(clippy::too_many_arguments)]
pub fn paint_igloo<W: WorldGenVolume>(
    b: &IglooBlocks,
    template: &FrozenTemplate,
    manifest: &TemplateManifest,
    piece: &IglooPiece,
    reference: IVec3,
    clip: BoundingBox,
    region: &mut W,
    rng: &mut XoroshiroRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) {
    if template.palettes.is_empty() {
        return;
    }
    let position = piece.placed_position();
    let pivot = piece.template.pivot();
    let palette = LegacyRandom::new(block_pos_seed(position))
        .next_i32_bound(template.palettes.len() as i32) as usize;
    let placed = place_template(
        &Placement {
            template,
            jigsaws: &[],
            palette,
            position,
            reference,
            rotation: piece.rotation,
            mirror: Mirror::None,
            pivot,
            random: SettingsRandom::Positional,
            clip: Some(clip),
            chain: &b.chain,
            waterlog: false,
            place_entities: true,
        },
        region,
        rng,
        entities,
        spawns,
    );
    if placed {
        let markers = manifest.markers.get(palette).map_or(&[][..], Vec::as_slice);
        for (pos, marker) in data_markers(
            markers,
            position,
            Mirror::None,
            piece.rotation,
            pivot,
            Some(clip),
        ) {
            if marker == "chest" {
                region.set(pos.into(), b.air);
                seed_container_loot(
                    entities,
                    (pos - IVec3::Y).into(),
                    "minecraft:chests/igloo_chest",
                    rng,
                );
            }
        }
    }
    if piece.template == IglooTemplate::Top {
        let trapdoor =
            position + transform(IVec3::new(3, 0, 5), Mirror::None, piece.rotation, pivot);
        let below = BlockPos::from(trapdoor - IVec3::Y);
        if !region.is_air(below) && !region.holds(&b.ladder, below) {
            region.set(trapdoor.into(), b.snow_block);
        }
    }
}
