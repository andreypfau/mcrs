use bevy_math::IVec3;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos, SectionPos};
use mcrs_minecraft_decoration::feature::template::{Placement, place_template};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen::structure::LiquidSettings;

use crate::world::generate::feature_program::{CompiledElement, FeatureProgram, Run};
use crate::world::generate::structures::index::StructureIndex;
use mcrs_minecraft_worldgen::structure::frozen::{ElementId, StructureKind};
use mcrs_minecraft_worldgen::structure::jigsaw::{Piece, Start};

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
// ponytail: no `Structure.afterPlace` hook; nothing jigsaw needs one, and the
// two structures that do (desert pyramid, mansion) have no generator yet.
#[allow(clippy::too_many_arguments)]
pub fn place_structures<W: WorldGenVolume>(
    index: &StructureIndex,
    program: &FeatureProgram,
    run: &mut Run,
    region: &mut W,
    starts: &[(ColumnPos, Start)],
    step: usize,
    clip: BoundingBox,
    decoration_seed: i64,
) {
    let frozen = &index.tables().frozen;
    for group in starts.chunk_by(|(_, a), (_, b)| a.structure == b.structure) {
        let structure = &frozen.structures[group[0].1.structure.0 as usize];
        if structure.step as usize != step {
            continue;
        }
        let liquid = match &structure.kind {
            StructureKind::Jigsaw { config, .. } => config.liquid_settings,
            StructureKind::Hardcoded => LiquidSettings::default(),
        };
        let seed = decoration_seed
            .wrapping_add(structure.step_index as i64)
            .wrapping_add(10_000 * step as i64);
        let mut rng = XoroshiroRandom::new(seed as u64);
        for (_, start) in group {
            let first = start.pieces[0].bounds();
            let centre = *first.min + (*first.max - *first.min + IVec3::ONE) / 2;
            let reference = IVec3::new(centre.x, first.min.y, centre.z);
            for piece in &start.pieces {
                if !piece.bounds().intersects(clip) {
                    continue;
                }
                let Piece::Jigsaw(jigsaw) = piece;
                place_element(
                    program,
                    run,
                    region,
                    jigsaw.element,
                    jigsaw.position,
                    reference,
                    jigsaw.rotation,
                    Some(clip),
                    &mut rng,
                    liquid,
                );
            }
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
                    clip,
                    chain,
                    waterlog: over.unwrap_or(liquid) == LiquidSettings::ApplyWaterlogging,
                },
                region,
                rng,
                &mut run.entities,
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
