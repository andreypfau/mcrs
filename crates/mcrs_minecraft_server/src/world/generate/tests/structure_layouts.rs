use std::collections::HashMap;
use std::path::PathBuf;

use bevy_math::IVec3;
use bytes::Buf;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen::corpus::{dump_string, open_dump};
use mcrs_minecraft_worldgen::feature::proto::{Holder, Rotation};
use mcrs_minecraft_worldgen::structure::Projection;
use mcrs_minecraft_worldgen::structure::template::BoundingBox;

use super::structure_sites::{build_index, dimension};
use super::structures::frozen_shared;
use crate::world::generate::structures::jigsaw::{Junction, Piece};
use crate::world::generate::structures::{ElementId, FrozenElement, FrozenStructures, TemplateId};

const MAGIC: &[u8; 8] = b"MCJIGSW0";

struct DumpPiece {
    element: String,
    projection: Projection,
    position: IVec3,
    rotation: Rotation,
    bounds: BoundingBox,
    ground_level_delta: i32,
    junctions: Vec<Junction>,
}

struct DumpStart {
    bounds: BoundingBox,
    pieces: Vec<DumpPiece>,
}

struct DumpCase {
    chunk: (i32, i32),
    start: Option<DumpStart>,
}

struct DumpStructure {
    id: String,
    cases: Vec<DumpCase>,
}

struct DumpSeed {
    seed: i64,
    dimensions: Vec<(String, Vec<DumpStructure>)>,
}

fn read_projection(r: &mut impl Buf) -> Projection {
    match r.get_u8() {
        0 => Projection::Rigid,
        1 => Projection::TerrainMatching,
        other => panic!("unknown projection {other}"),
    }
}

fn read_box(r: &mut impl Buf) -> BoundingBox {
    let min = IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
    let max = IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
    BoundingBox { min, max }
}

fn read_piece(r: &mut impl Buf) -> DumpPiece {
    DumpPiece {
        element: dump_string(r),
        projection: read_projection(r),
        position: IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le()),
        rotation: Rotation::ALL[r.get_u8() as usize],
        bounds: read_box(r),
        ground_level_delta: r.get_i32_le(),
        junctions: (0..r.get_u32_le())
            .map(|_| Junction {
                source_x: r.get_i32_le(),
                source_ground_y: r.get_i32_le(),
                source_z: r.get_i32_le(),
                delta_y: r.get_i32_le(),
                dest_projection: read_projection(r),
            })
            .collect(),
    }
}

fn read_dump() -> Vec<DumpSeed> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/world/generate/tests/fixtures/structure_layouts.bin");
    let mut r = open_dump(&path, MAGIC);
    let seeds = (0..r.get_u32_le())
        .map(|_| DumpSeed {
            seed: r.get_i64_le(),
            dimensions: (0..r.get_u32_le())
                .map(|_| {
                    let dimension = dump_string(&mut r);
                    let structures = (0..r.get_u32_le())
                        .map(|_| {
                            let id = dump_string(&mut r);
                            let _set = dump_string(&mut r);
                            let cases = (0..r.get_u32_le())
                                .map(|_| {
                                    let chunk = (r.get_i32_le(), r.get_i32_le());
                                    let start = (r.get_u8() == 1).then(|| {
                                        let bounds = read_box(&mut r);
                                        let pieces = (0..r.get_u32_le())
                                            .map(|_| read_piece(&mut r))
                                            .collect();
                                        DumpStart { bounds, pieces }
                                    });
                                    DumpCase { chunk, start }
                                })
                                .collect();
                            DumpStructure { id, cases }
                        })
                        .collect();
                    (dimension, structures)
                })
                .collect(),
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    seeds
}

fn render(
    frozen: &FrozenStructures,
    template_names: &HashMap<TemplateId, &ResourceLocation>,
    element: ElementId,
) -> String {
    match &frozen.elements[element.0 as usize] {
        FrozenElement::Single {
            template, legacy, ..
        } => {
            let location = template_names[template];
            format!("{}:{location}", if *legacy { "legacy" } else { "single" })
        }
        FrozenElement::List { elements, .. } => {
            let inner: Vec<String> = elements
                .iter()
                .map(|e| render(frozen, template_names, *e))
                .collect();
            format!("list[{}]", inner.join(","))
        }
        FrozenElement::Feature {
            feature: Holder::Reference(id),
            ..
        } => format!("feature:{id}"),
        FrozenElement::Feature {
            feature: Holder::Inline(_),
            ..
        } => panic!("the oracle renders a feature by its registry key"),
        FrozenElement::Empty => "empty".to_string(),
    }
}

#[test]
fn structure_layouts_match_the_oracle() {
    let dump = read_dump();
    assert_eq!(dump.len(), 5, "the dump lost seeds");
    let frozen = frozen_shared();
    let template_names: HashMap<TemplateId, &ResourceLocation> = frozen
        .template_ids
        .iter()
        .map(|(location, id)| (*id, location))
        .collect();
    let mut cases = 0;
    let mut present = 0;
    let mut pieces = 0;
    for entry in &dump {
        let seed = entry.seed;
        assert_eq!(entry.dimensions.len(), 2);
        for (dimension_id, structures) in &entry.dimensions {
            let index = build_index(&dimension(dimension_id), seed);
            for structure in structures {
                let id = frozen.structure_ids[&ResourceLocation::parse(&structure.id).unwrap()];
                assert_eq!(structure.cases.len(), 16, "{}: cases", structure.id);
                for case in &structure.cases {
                    let (x, z) = case.chunk;
                    let label = format!("seed {seed} {} at chunk ({x}, {z})", structure.id);
                    cases += 1;
                    let starts = index.starts_at(case.chunk);
                    let ours: Vec<_> = starts.iter().filter(|s| s.structure == id).collect();
                    assert_eq!(
                        ours.len(),
                        case.start.is_some() as usize,
                        "{label}: present"
                    );
                    let Some(expected) = &case.start else {
                        continue;
                    };
                    let start = ours[0];
                    present += 1;
                    assert_eq!(start.bounds, expected.bounds, "{label}: start box");
                    assert_eq!(
                        start.pieces.len(),
                        expected.pieces.len(),
                        "{label}: piece count"
                    );
                    for (i, (piece, want)) in start.pieces.iter().zip(&expected.pieces).enumerate()
                    {
                        let Piece::Jigsaw(piece) = piece;
                        let label = format!("{label} piece #{i} {}", want.element);
                        assert_eq!(
                            render(frozen, &template_names, piece.element),
                            want.element,
                            "{label}: element"
                        );
                        assert_eq!(piece.projection, want.projection, "{label}: projection");
                        assert_eq!(piece.position, want.position, "{label}: position");
                        assert_eq!(piece.rotation, want.rotation, "{label}: rotation");
                        assert_eq!(piece.bounds, want.bounds, "{label}: box");
                        assert_eq!(
                            piece.ground_level_delta, want.ground_level_delta,
                            "{label}: ground_level_delta"
                        );
                        assert_eq!(piece.junctions, want.junctions, "{label}: junctions");
                        pieces += 1;
                    }
                }
            }
        }
    }
    assert_eq!((cases, present, pieces), (5 * 28 * 16, 206, 28011));
}
