//! Random-spread structure placement against the reference's
//! `getPotentialStructureChunk` and `isStructureChunk` over two chunk squares.

use bytes::Buf;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_worldgen_structure::StructureSet;
use mcrs_minecraft_worldgen_structure::placement::{SpreadPlacement, frequency_gate};
use mcrs_minecraft_worldgen_testing::{self as corpus, dump_string};
use mcrs_voxel::ColumnPos;
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"MCPLACE0";

pub(crate) struct DumpGrid {
    x0: i32,
    z0: i32,
    side: i32,
    pub(crate) cells: Vec<((i32, i32), bool)>,
}

pub(crate) struct DumpSet {
    id: String,
    spacing: i32,
    separation: i32,
    pub(crate) grids: Vec<DumpGrid>,
}

pub(crate) struct DumpDim {
    pub(crate) sets: Vec<DumpSet>,
}

pub(crate) struct DumpSeed {
    seed: i64,
    pub(crate) dims: Vec<DumpDim>,
}

fn read_dump() -> Vec<DumpSeed> {
    let source = fixture("structure_placement").file("structure_cells.bin");
    let mut r = bytes::Bytes::from(source.dump(MAGIC));
    let seeds = (0..r.get_u32_le())
        .map(|_| DumpSeed {
            seed: r.get_i64_le(),
            dims: (0..r.get_u32_le())
                .map(|_| {
                    dump_string(&mut r);
                    DumpDim {
                        sets: (0..r.get_u32_le())
                            .map(|_| DumpSet {
                                id: dump_string(&mut r),
                                spacing: r.get_i32_le(),
                                separation: r.get_i32_le(),
                                grids: (0..r.get_u32_le())
                                    .map(|_| {
                                        let (x0, z0, side) =
                                            (r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
                                        let cells = (0..side * side)
                                            .map(|_| {
                                                let potential = (r.get_i32_le(), r.get_i32_le());
                                                (potential, r.get_u8() != 0)
                                            })
                                            .collect();
                                        DumpGrid {
                                            x0,
                                            z0,
                                            side,
                                            cells,
                                        }
                                    })
                                    .collect(),
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", source.name());
    seeds
}

struct Placements {
    sets: BTreeMap<ResourceLocation, StructureSet>,
}

impl Placements {
    fn load() -> Self {
        Self {
            sets: corpus::registry("structure_set"),
        }
    }

    fn spread(&self, id: &str) -> (SpreadPlacement, Option<SpreadPlacement>) {
        let set = &self.sets[&ResourceLocation::read(id).unwrap()];
        let placement = SpreadPlacement::of(&set.placement)
            .unwrap_or_else(|| panic!("{id} is not a random-spread set"));
        let excluded = match &set.placement {
            mcrs_minecraft_worldgen_structure::StructurePlacement::RandomSpread {
                spreading,
                ..
            } => spreading.exclusion_zone.as_ref().map(|zone| {
                let other = SpreadPlacement::of(&self.sets[&zone.other_set].placement)
                    .unwrap_or_else(|| panic!("{} is not a random-spread set", zone.other_set));
                assert!(
                    other.exclusion_chunks.is_none(),
                    "{} excludes {}, which itself excludes a third set",
                    id,
                    zone.other_set
                );
                other
            }),
            _ => None,
        };
        (placement, excluded)
    }

    fn is_structure_chunk(&self, id: &str, seed: i64, x: i32, z: i32) -> bool {
        let (placement, excluded) = self.spread(id);
        placement.is_structure_chunk(seed, ColumnPos::new(x, z), |test| {
            excluded.is_some_and(|other| other.is_structure_chunk(seed, test, |_| false))
        })
    }
}

mod exhaustive {
    use super::*;

    #[test]
    fn potential_chunks_match_the_reference() {
        let placements = Placements::load();
        for seed in read_dump() {
            for set in seed.dims.iter().flat_map(|dim| &dim.sets) {
                let (placement, _) = placements.spread(&set.id);
                assert_eq!(
                    (placement.spacing, placement.separation),
                    (set.spacing, set.separation),
                    "{}",
                    set.id
                );
                for grid in &set.grids {
                    for (i, (potential, _)) in grid.cells.iter().enumerate() {
                        let (x, z) = (
                            grid.x0 + i as i32 % grid.side,
                            grid.z0 + i as i32 / grid.side,
                        );
                        assert_eq!(
                            placement.potential_chunk(seed.seed, ColumnPos::new(x, z)),
                            ColumnPos::from(*potential),
                            "{} seed {} chunk ({x}, {z})",
                            set.id,
                            seed.seed
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn structure_chunks_match_the_reference() {
        let placements = Placements::load();
        let mut hits = 0usize;
        for seed in read_dump() {
            for set in seed.dims.iter().flat_map(|dim| &dim.sets) {
                for grid in &set.grids {
                    for (i, (_, is_structure)) in grid.cells.iter().enumerate() {
                        let (x, z) = (
                            grid.x0 + i as i32 % grid.side,
                            grid.z0 + i as i32 / grid.side,
                        );
                        assert_eq!(
                            placements.is_structure_chunk(&set.id, seed.seed, x, z),
                            *is_structure,
                            "{} seed {} chunk ({x}, {z})",
                            set.id,
                            seed.seed
                        );
                        hits += *is_structure as usize;
                    }
                }
            }
        }
        assert!(hits > 0);
    }

    #[test]
    fn placement_ignores_the_top_sixteen_seed_bits() {
        let placements = Placements::load();
        let dump = read_dump();
        let sets: Vec<&str> = dump[0]
            .dims
            .iter()
            .flat_map(|d| &d.sets)
            .map(|s| s.id.as_str())
            .collect();
        for seed in dump.iter().map(|s| s.seed) {
            let flipped = seed ^ (0xFFFF << 48);
            assert_ne!(seed, flipped);
            for id in &sets {
                let (placement, _) = placements.spread(id);
                for (x, z) in (-30..30)
                    .step_by(7)
                    .flat_map(|x| (-30..30).step_by(5).map(move |z| (x, z)))
                {
                    assert_eq!(
                        placement.potential_chunk(seed, ColumnPos::new(x, z)),
                        placement.potential_chunk(flipped, ColumnPos::new(x, z)),
                        "{id} seed {seed} ({x}, {z})"
                    );
                    for ColumnPos { x, z } in [
                        ColumnPos::new(x, z),
                        placement.potential_chunk(seed, ColumnPos::new(x, z)),
                    ] {
                        assert_eq!(
                            frequency_gate(
                                seed,
                                placement.salt,
                                placement.frequency,
                                placement.reduction,
                                ColumnPos::new(x, z),
                            ),
                            frequency_gate(
                                flipped,
                                placement.salt,
                                placement.frequency,
                                placement.reduction,
                                ColumnPos::new(x, z),
                            ),
                            "{id} seed {seed} ({x}, {z})"
                        );
                    }
                }
            }
        }
    }
}
