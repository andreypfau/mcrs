use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos, SectionPos};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature_place::template::{Placement, SettingsRandom, place_template};
use mcrs_minecraft_worldgen_structure::LiquidSettings;
use mcrs_minecraft_worldgen_structure::hardcoded::mineshaft::decorates_first;

use crate::feature_program::{CompiledElement, CompiledStructure, FeatureProgram, Run};
use crate::stages::decoration_seed;
use mcrs_minecraft_worldgen_structure::frozen::{
    ElementId, FrozenStructure, FrozenStructures, StructureKind,
};
use mcrs_minecraft_worldgen_structure::piece::{DesertPyramidPiece, Piece, Start};
use mcrs_minecraft_worldgen_structure_place::after_place;
use mcrs_minecraft_worldgen_structure_place::buried_treasure::paint_buried_treasure;
use mcrs_minecraft_worldgen_structure_place::canvas::PieceCanvas;
use mcrs_minecraft_worldgen_structure_place::end_city::place_end_city_piece;
use mcrs_minecraft_worldgen_structure_place::fortress::paint_fortress;
use mcrs_minecraft_worldgen_structure_place::jungle_temple::paint_jungle_temple;
use mcrs_minecraft_worldgen_structure_place::mineshaft::paint_mineshaft;
use mcrs_minecraft_worldgen_structure_place::nether_fossil::paint_nether_fossil;
use mcrs_minecraft_worldgen_structure_place::ocean_monument::paint_ocean_monument;
use mcrs_minecraft_worldgen_structure_place::portal::place_ruined_portal;
use mcrs_minecraft_worldgen_structure_place::scattered::{paint_desert_pyramid, paint_swamp_hut};
use mcrs_minecraft_worldgen_structure_place::stronghold::paint_stronghold;
use mcrs_minecraft_worldgen_structure_place::template::place_ocean_ruin;
use mcrs_minecraft_worldgen_structure_place::template_piece::{paint_igloo, paint_shipwreck};
use mcrs_minecraft_worldgen_structure_place::woodland_mansion::place_woodland_mansion_piece;

/// `ChunkGenerator.getWritableArea`: the column's footprint from one above the
/// dimension floor to its ceiling.
pub fn column_clip(col: ColumnPos, y_sections: &[i32]) -> BoundingBox {
    let (first, last) = y_sections
        .first()
        .zip(y_sections.last())
        .map_or((0, 0), |(&first, &last)| (first, last));
    let min_y = first << SectionPos::BITS;
    let max_y = (last << SectionPos::BITS) + SectionPos::MASK as i32;
    BoundingBox {
        min: BlockPos::new(col.x * 16, min_y + 1, col.z * 16),
        max: BlockPos::new(col.x * 16 + 15, max_y, col.z * 16 + 15),
    }
}

/// `StructureStart.placeInChunk` for every start of `step` reaching the column:
/// one stream per structure, shared by all of its starts and pieces here.
#[allow(clippy::too_many_arguments)]
pub fn place_structures<W: WorldGenVolume>(
    frozen: &FrozenStructures,
    program: &FeatureProgram,
    run: &mut Run,
    region: &mut W,
    starts: &[(ColumnPos, Start)],
    step: usize,
    clip: BoundingBox,
    decoration_seed: i64,
) {
    for group in starts.chunk_by(|(_, a), (_, b)| a.structure == b.structure) {
        let structure = &frozen.structures[group[0].1.structure.0 as usize];
        if structure.step as usize != step {
            continue;
        }
        let liquid = match &structure.kind {
            StructureKind::Jigsaw { config, .. } => config.liquid_settings,
            _ => LiquidSettings::default(),
        };
        let seed = decoration_seed
            .wrapping_add(structure.step_index as i64)
            .wrapping_add(10_000 * step as i64);
        let mut rng = WorldgenRandom::new(seed as u64);
        for (_, start) in group {
            place_start(frozen, program, run, region, start, clip, &mut rng, liquid);
        }
    }
}

/// The reference sinks a desert pyramid by `nextInt(3)` of whichever column
/// decorates it first and every later column only spends the draw. The column
/// nearest the origin among those the box meets stands in for that chunk, as
/// it does for a mineshaft's spawner, so the columns agree whichever runs first.
fn desert_pyramid_sink(
    world_seed: i64,
    piece: &DesertPyramidPiece,
    structure: &FrozenStructure,
) -> i32 {
    let BoundingBox { min, max } = piece.bounds;
    let host = (min.x >> 4..=max.x >> 4)
        .flat_map(|x| (min.z >> 4..=max.z >> 4).map(move |z| ColumnPos::new(x, z)))
        .min_by(|a, b| decorates_first(*a, *b))
        .expect("a box meets at least one column");
    let seed = decoration_seed(world_seed, host.x * 16, host.z * 16)
        .wrapping_add(structure.step_index as i64)
        .wrapping_add(10_000 * structure.step as i64);
    WorldgenRandom::new(seed as u64).next_i32_bound(3)
}

fn sunk_bounds(piece: &DesertPyramidPiece, sink: i32) -> BoundingBox {
    piece.bounds.moved(IVec3::new(
        0,
        piece.height_position - sink - piece.bounds.min.y,
        0,
    ))
}

/// `StructureStart.placeInChunk` for one start: every piece whose box meets
/// the clip, in piece order, with the first piece's box centre at its floor as
/// the reference position, then the structure's after-place hook.
#[allow(clippy::too_many_arguments)]
pub fn place_start<W: WorldGenVolume>(
    frozen: &FrozenStructures,
    program: &FeatureProgram,
    run: &mut Run,
    region: &mut W,
    start: &Start,
    clip: BoundingBox,
    rng: &mut WorldgenRandom,
    liquid: LiquidSettings,
) {
    let structure = &frozen.structures[start.structure.0 as usize];
    let first = start.pieces[0].bounds();
    let centre = first.center();
    let reference = IVec3::new(centre.x, first.min.y, centre.z);
    for piece in &start.pieces {
        if !piece.bounds().intersects(clip) {
            continue;
        }
        match piece {
            Piece::Jigsaw(jigsaw) => {
                place_element(
                    program,
                    run,
                    region,
                    jigsaw.element,
                    jigsaw.position,
                    reference,
                    jigsaw.rotation,
                    Some(clip),
                    rng,
                    liquid,
                );
            }
            Piece::DesertPyramid(piece) => {
                let Some(CompiledStructure::DesertPyramid(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                let sink = desert_pyramid_sink(blocks.world_seed, piece, structure);
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: sunk_bounds(piece, sink),
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_desert_pyramid(blocks, &mut canvas, rng);
            }
            Piece::JungleTemple(piece) => {
                let Some(CompiledStructure::JungleTemple(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                let raised = piece.bounds.moved(IVec3::new(
                    0,
                    piece.height_position - piece.bounds.min.y,
                    0,
                ));
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: raised,
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_jungle_temple(blocks, &mut canvas, rng);
            }
            Piece::SwampHut(piece) => {
                let Some(CompiledStructure::SwampHut(blocks)) = program.structure(start.structure)
                else {
                    continue;
                };
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: piece.bounds.moved(IVec3::new(
                        0,
                        piece.height_position - piece.bounds.min.y,
                        0,
                    )),
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_swamp_hut(
                    blocks,
                    &mut canvas,
                    rng,
                    start.structure.0,
                    &frozen.variants,
                );
            }
            Piece::BuriedTreasure(piece) => {
                let Some(CompiledStructure::BuriedTreasure(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: piece.bounds,
                    orientation: None,
                    clip,
                    keep: None,
                };
                paint_buried_treasure(blocks, &mut canvas, rng);
            }
            Piece::Fortress(piece) => {
                let Some(CompiledStructure::Fortress(blocks)) = program.structure(start.structure)
                else {
                    continue;
                };
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: piece.bounds,
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_fortress(blocks, piece.kind, &mut canvas, rng);
            }
            Piece::Shipwreck(piece) => {
                let Some(CompiledStructure::Shipwreck(chain)) = program.structure(start.structure)
                else {
                    continue;
                };
                paint_shipwreck(
                    chain,
                    &frozen.templates[piece.template.0 as usize],
                    &frozen.manifests[piece.template.0 as usize],
                    piece,
                    reference,
                    clip,
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                );
            }
            Piece::OceanRuin(piece) => {
                let (Some(CompiledStructure::OceanRuin(blocks)), StructureKind::OceanRuin(config)) =
                    (program.structure(start.structure), &structure.kind)
                else {
                    continue;
                };
                place_ocean_ruin(
                    blocks,
                    frozen,
                    config,
                    start.structure,
                    piece,
                    reference,
                    clip,
                    region,
                    &mut run.entities,
                    &mut run.spawns,
                    rng,
                );
            }
            Piece::RuinedPortal(piece) => {
                let Some(CompiledStructure::RuinedPortal(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                place_ruined_portal(
                    blocks,
                    piece,
                    &frozen.templates[piece.template.0 as usize],
                    region,
                    &mut run.entities,
                    &mut run.spawns,
                    reference,
                    clip,
                    rng,
                );
            }
            Piece::OceanMonument(piece) => {
                let Some(CompiledStructure::OceanMonument(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: piece.bounds,
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_ocean_monument(blocks, piece, &mut canvas, rng);
            }
            Piece::Mineshaft(piece) => {
                let Some(CompiledStructure::Mineshaft(blocks)) = program.structure(start.structure)
                else {
                    continue;
                };
                paint_mineshaft(
                    blocks,
                    piece,
                    region,
                    &mut run.entities,
                    &mut run.spawns,
                    clip,
                    rng,
                );
            }
            Piece::Igloo(piece) => {
                let Some(CompiledStructure::Igloo(blocks)) = program.structure(start.structure)
                else {
                    continue;
                };
                let template = piece.template.id(frozen).0 as usize;
                paint_igloo(
                    blocks,
                    &frozen.templates[template],
                    &frozen.manifests[template],
                    piece,
                    reference,
                    clip,
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                );
            }
            Piece::NetherFossil(piece) => {
                let Some(CompiledStructure::NetherFossil(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                paint_nether_fossil(
                    blocks,
                    &frozen.templates[piece.template.0 as usize],
                    piece,
                    reference,
                    clip,
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                );
            }
            Piece::Stronghold(piece) => {
                let Some(CompiledStructure::Stronghold(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    spawns: &mut run.spawns,
                    bounds: piece.bounds,
                    orientation: Some(piece.orientation),
                    clip,
                    keep: None,
                };
                paint_stronghold(blocks, piece.kind, piece.entry_door, &mut canvas, rng);
            }
            Piece::EndCity(piece) => {
                let Some(CompiledStructure::EndCity(chains)) = program.structure(start.structure)
                else {
                    continue;
                };
                place_end_city_piece(
                    chains,
                    frozen,
                    piece,
                    reference,
                    clip,
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                );
            }
            Piece::WoodlandMansion(piece) => {
                let Some(CompiledStructure::WoodlandMansion(blocks)) =
                    program.structure(start.structure)
                else {
                    continue;
                };
                place_woodland_mansion_piece(
                    blocks,
                    &frozen.templates[piece.template.0 as usize],
                    &frozen.manifests[piece.template.0 as usize],
                    piece,
                    reference,
                    clip,
                    region,
                    rng,
                    &mut run.entities,
                    &mut run.spawns,
                );
            }
        }
    }
    match program.structure(start.structure) {
        Some(CompiledStructure::DesertPyramid(blocks)) => {
            for piece in &start.pieces {
                let Piece::DesertPyramid(piece) = piece else {
                    continue;
                };
                let sink = desert_pyramid_sink(blocks.world_seed, piece, structure);
                after_place::desert_pyramid(
                    blocks,
                    region,
                    &mut run.entities,
                    clip,
                    sunk_bounds(piece, sink),
                    piece.orientation,
                );
            }
        }
        Some(CompiledStructure::WoodlandMansion(blocks)) => {
            let piece_bounds: Vec<BoundingBox> = start.pieces.iter().map(Piece::bounds).collect();
            after_place::woodland_mansion(blocks, region, clip, &piece_bounds);
        }
        _ => {}
    }
}

/// `StructurePoolElement.place` over a compiled element.
#[allow(clippy::too_many_arguments)]
pub fn place_element<W: WorldGenVolume>(
    program: &FeatureProgram,
    run: &mut Run,
    region: &mut W,
    element: ElementId,
    position: IVec3,
    reference: IVec3,
    rotation: Rotation,
    clip: Option<BoundingBox>,
    rng: &mut WorldgenRandom,
    liquid: LiquidSettings,
) -> bool {
    match program.element(element) {
        CompiledElement::Single {
            template,
            manifest,
            chain,
            liquid: over,
        } => {
            let Some(chain) = chain else {
                return false;
            };
            if template.palettes.is_empty() {
                return false;
            }
            let palette = LegacyRandom::new(block_pos_seed(position))
                .next_i32_bound(template.palettes.len() as i32) as usize;
            let jigsaws = manifest.jigsaws.get(palette).map_or(&[][..], Vec::as_slice);
            place_template(
                &Placement {
                    template,
                    jigsaws,
                    palette,
                    position,
                    reference,
                    rotation,
                    mirror: Mirror::None,
                    pivot: IVec3::ZERO,
                    random: SettingsRandom::Positional,
                    clip,
                    chain,
                    waterlog: over.unwrap_or(liquid) == LiquidSettings::ApplyWaterlogging,
                    place_entities: false,
                },
                region,
                rng,
                &mut run.entities,
                &mut run.spawns,
            )
        }
        CompiledElement::List(elements) => elements.iter().all(|inner| {
            place_element(
                program, run, region, *inner, position, reference, rotation, clip, rng, liquid,
            )
        }),
        CompiledElement::Feature(nested) => {
            nested.place(run, region, rng, position.into(), &|_| true)
        }
        CompiledElement::Empty => true,
    }
}
