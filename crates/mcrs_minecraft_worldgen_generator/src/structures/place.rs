use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos, SectionPos};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature_place::template::{Placement, SettingsRandom, place_template};
use mcrs_minecraft_worldgen_structure::LiquidSettings;

use crate::feature_program::{CompiledElement, CompiledStructure, FeatureProgram, Run};
use crate::stages::decoration_seed;
use mcrs_minecraft_worldgen_structure::frozen::{
    ElementId, FrozenStructure, FrozenStructures, StructureKind,
};
use mcrs_minecraft_worldgen_structure::piece::{DesertPyramidPiece, Piece, Start};
use mcrs_minecraft_worldgen_structure_place::after_place;
use mcrs_minecraft_worldgen_structure_place::buried_treasure::paint_buried_treasure;
use mcrs_minecraft_worldgen_structure_place::canvas::PieceCanvas;
use mcrs_minecraft_worldgen_structure_place::fortress::paint_fortress;
use mcrs_minecraft_worldgen_structure_place::jungle_temple::paint_jungle_temple;
use mcrs_minecraft_worldgen_structure_place::portal::place_ruined_portal;
use mcrs_minecraft_worldgen_structure_place::ocean_monument::paint_ocean_monument;
use mcrs_minecraft_worldgen_structure_place::scattered::paint_desert_pyramid;
use mcrs_minecraft_worldgen_structure_place::template::place_ocean_ruin;
use mcrs_minecraft_worldgen_structure_place::template_piece::paint_shipwreck;

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
        let mut rng = XoroshiroRandom::new(seed as u64);
        for (chunk, start) in group {
            place_start(
                frozen, program, run, region, start, *chunk, clip, &mut rng, liquid,
            );
        }
    }
}

/// The reference sinks a desert pyramid by `nextInt(3)` of whichever column
/// decorates it first and every later column only spends the draw; here the
/// value is the start chunk's own, so the columns agree whichever runs first.
fn desert_pyramid_sink(world_seed: i64, chunk: ColumnPos, structure: &FrozenStructure) -> i32 {
    let seed = decoration_seed(world_seed, chunk.x * 16, chunk.z * 16)
        .wrapping_add(structure.step_index as i64)
        .wrapping_add(10_000 * structure.step as i64);
    XoroshiroRandom::new(seed as u64).next_i32_bound(3)
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
    chunk: ColumnPos,
    clip: BoundingBox,
    rng: &mut XoroshiroRandom,
    liquid: LiquidSettings,
) {
    let structure = &frozen.structures[start.structure.0 as usize];
    let first = start.pieces[0].bounds();
    let centre = *first.min + (*first.max - *first.min + IVec3::ONE) / 2;
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
                let sink = desert_pyramid_sink(blocks.world_seed, chunk, structure);
                let mut canvas = PieceCanvas {
                    volume: region,
                    entities: &mut run.entities,
                    bounds: sunk_bounds(piece, sink),
                    orientation: Some(piece.orientation),
                    clip,
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
                    bounds: raised,
                    orientation: Some(piece.orientation),
                    clip,
                };
                paint_jungle_temple(blocks, &mut canvas, rng);
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
                    bounds: piece.bounds,
                    orientation: None,
                    clip,
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
                    bounds: piece.bounds,
                    orientation: Some(piece.orientation),
                    clip,
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
                    bounds: piece.bounds,
                    orientation: Some(piece.orientation),
                    clip,
                };
                paint_ocean_monument(blocks, piece, &mut canvas, rng, &mut run.spawns);
            }
        }
    }
    if let Some(CompiledStructure::DesertPyramid(blocks)) = program.structure(start.structure) {
        for piece in &start.pieces {
            let Piece::DesertPyramid(piece) = piece else {
                continue;
            };
            let sink = desert_pyramid_sink(blocks.world_seed, chunk, structure);
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
    rng: &mut XoroshiroRandom,
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
