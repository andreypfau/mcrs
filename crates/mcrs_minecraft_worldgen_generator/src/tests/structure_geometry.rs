use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::LazyLock;

use bevy_math::IVec3;
use bytes::Buf;
use mcrs_minecraft_chunk::{BoxVolume, Volume, VoxelId};
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::to_nbt_compound;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{BiomeMask, BoxRegion, WorldStates};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_structure::LiquidSettings;
use mcrs_minecraft_worldgen_structure::frozen::{FrozenStructures, StructureKind};
use mcrs_minecraft_worldgen_structure::piece::Start;
use mcrs_minecraft_worldgen_structure::site::{BaseColumn, Context, SiteWorld, layout, site};
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};

use super::structure_pieces::{read_box, read_nbt};
use super::structures::frozen_shared;
use super::template_manifest::{parse_state, resolve};
use super::template_parity::{canonical, fnv, program, state_named, written};
use super::{biome_index, block_tags, blocks, corpus, corpus_climate};
use crate::feature_program::RunScratch;
use crate::heightmap::{HeightmapPredicates, heightmap_predicates};
use crate::heightmap_kind;
use crate::structures::place::{column_clip, place_start};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::{RAIN_TEMPERATURE, temperature};

const MAGIC: &[u8; 8] = b"MCSTRGE0";

/// Every structure type the oracle places and this build cannot yet.
const UNPORTED_GEOMETRY_TYPES: [&str; 7] = [
    "minecraft:end_city",
    "minecraft:igloo",
    "minecraft:mineshaft",
    "minecraft:nether_fossil",
    "minecraft:stronghold",
    "minecraft:swamp_hut",
    "minecraft:woodland_mansion",
];

/// The oracle's packed entity data follows the tag and is skipped: the server
/// derives that packet from the components delivery builds out of these fields.
struct DumpEntity {
    type_id: String,
    nbt: NbtCompound,
}

struct DumpChunk {
    chunk: ColumnPos,
    stream_seed: i64,
    count: u32,
    hash: u64,
    full: Option<Vec<([i32; 3], u32)>>,
    block_entities: Vec<([i32; 3], String, NbtCompound)>,
    entities: Vec<DumpEntity>,
    rng: [i64; 2],
}

struct DumpStart {
    bounds: BoundingBox,
    pieces: u32,
    step: u8,
    index: u32,
    chunks: Vec<DumpChunk>,
}

struct DumpCase {
    structure: String,
    dimension: String,
    biome: String,
    base: u8,
    seed: i64,
    chunk: ColumnPos,
    start: Option<DumpStart>,
}

struct Dump {
    palette: Vec<String>,
    cases: Vec<DumpCase>,
}

fn read_dump() -> Dump {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/structure_geometry.bin");
    let mut r = open_dump(&path, MAGIC);
    let vec3 = |r: &mut bytes::Bytes| [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
    let palette = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
    let cases = (0..r.get_u32_le())
        .map(|_| {
            let structure = dump_string(&mut r);
            let dimension = dump_string(&mut r);
            let biome = dump_string(&mut r);
            let base = r.get_u8();
            let seed = r.get_i64_le();
            let chunk = ColumnPos::new(r.get_i32_le(), r.get_i32_le());
            let start = (r.get_u8() == 1).then(|| {
                let bounds = read_box(&mut r);
                let pieces = r.get_u32_le();
                let step = r.get_u8();
                let index = r.get_u32_le();
                let chunks = (0..r.get_u32_le())
                    .map(|_| {
                        let chunk = ColumnPos::new(r.get_i32_le(), r.get_i32_le());
                        let stream_seed = r.get_i64_le();
                        let count = r.get_u32_le();
                        let hash = r.get_u64_le();
                        let full = (r.get_u8() == 1).then(|| {
                            (0..count)
                                .map(|_| {
                                    let pos = vec3(&mut r);
                                    (pos, r.get_u32_le())
                                })
                                .collect()
                        });
                        let block_entities = (0..r.get_u32_le())
                            .map(|_| {
                                let pos = vec3(&mut r);
                                let type_id = dump_string(&mut r);
                                let nbt =
                                    read_nbt(&mut r, &format!("{structure} {type_id} at {pos:?}"));
                                (pos, type_id, nbt)
                            })
                            .collect();
                        let entities = (0..r.get_u32_le())
                            .map(|_| {
                                let type_id = dump_string(&mut r);
                                let nbt = read_nbt(&mut r, &format!("{structure} {type_id}"));
                                let len = r.get_u32_le() as usize;
                                r.advance(len);
                                DumpEntity { type_id, nbt }
                            })
                            .collect();
                        let rng = [r.get_i64_le(), r.get_i64_le()];
                        DumpChunk {
                            chunk,
                            stream_seed,
                            count,
                            hash,
                            full,
                            block_entities,
                            entities,
                            rng,
                        }
                    })
                    .collect();
                DumpStart {
                    bounds,
                    pieces,
                    step,
                    index,
                    chunks,
                }
            });
            DumpCase {
                structure,
                dimension,
                biome,
                base,
                seed,
                chunk,
                start,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    Dump { palette, cases }
}

fn dump() -> &'static Dump {
    static DUMP: LazyLock<Dump> = LazyLock::new(read_dump);
    &DUMP
}

fn predicates() -> &'static HeightmapPredicates {
    static PREDICATES: LazyLock<HeightmapPredicates> =
        LazyLock::new(|| heightmap_predicates(blocks(), block_tags()));
    &PREDICATES
}

/// The oracle's dimensions: the height range every accessor spans and the
/// noise settings' sea level, which the flat generator answers with.
fn dimension(id: &str) -> (i32, i32, i32) {
    match id {
        "minecraft:overworld" => (-64, 384, 63),
        "minecraft:the_nether" => (0, 256, 32),
        "minecraft:the_end" => (0, 256, 0),
        other => panic!("the dump names an unexpected dimension {other}"),
    }
}

/// The oracle's flat bases: the layers from the dimension floor up to y 63,
/// `None` being air.
fn layers(base: u8, min_y: i32) -> Vec<Option<VoxelId>> {
    let state = |block: &str| VoxelId::from(corpus().default_state(block));
    let stone = state("minecraft:stone");
    (min_y..=63)
        .map(|y| match base {
            0 => Some(match y {
                ..=60 => stone,
                61..=62 => state("minecraft:dirt"),
                _ => state("minecraft:grass_block"),
            }),
            1 => match y {
                ..=30 => Some(stone),
                31..=40 => Some(state("minecraft:gravel")),
                41..=62 => Some(state("minecraft:water")),
                _ => None,
            },
            2 => (!(33..=40).contains(&y)).then_some(stone),
            other => panic!("unknown base {other}"),
        })
        .collect()
}

/// `FlatLevelSource` as a site world: one biome, the layers as the base
/// column, and heights walked over the layers by the heightmap's predicate.
struct FlatSiteWorld<'a> {
    min_y: i32,
    max_y: i32,
    layers: Vec<VoxelId>,
    biome: u32,
    states: &'a WorldStates,
    predicates: &'a HeightmapPredicates,
}

impl FlatSiteWorld<'_> {
    fn free_height(&self, heightmap: HeightmapName) -> i32 {
        let top = (self.layers.len() as i32 - 1).min(self.max_y - self.min_y);
        (0..=top)
            .rev()
            .find(|&layer| self.opaque(self.layers[layer as usize], heightmap))
            .map_or(self.min_y, |layer| self.min_y + layer + 1)
    }
}

impl SiteWorld for FlatSiteWorld<'_> {
    fn biome_at(&mut self, _: IVec3) -> Option<u32> {
        Some(self.biome)
    }

    fn column_admits(&mut self, _: i32, _: i32, _: i32, _: i32, biomes: &BiomeMask) -> bool {
        biomes.contains(self.biome as usize)
    }

    fn all_biomes_within(&mut self, _: IVec3, _: i32, biomes: &BiomeMask) -> bool {
        biomes.contains(self.biome as usize)
    }

    fn free_height(&mut self, _: i32, _: i32, heightmap: HeightmapName) -> i32 {
        FlatSiteWorld::free_height(self, heightmap)
    }

    fn base_column(&mut self, _: i32, _: i32) -> BaseColumn<'_> {
        BaseColumn {
            min_y: self.min_y,
            states: &self.layers,
            air: self.states.air,
        }
    }

    fn opaque(&self, state: VoxelId, heightmap: HeightmapName) -> bool {
        self.predicates
            .get(state)
            .contains(heightmap_kind(heightmap))
    }

    fn states(&self) -> &WorldStates {
        self.states
    }

    fn cold_enough_to_snow(&mut self, pos: IVec3, sea_level: i32) -> bool {
        let climate = &corpus_climate()[self.biome as usize];
        temperature(climate, pos.into(), sea_level) < RAIN_TEMPERATURE
    }
}

fn flat_start(frozen: &FrozenStructures, case: &DumpCase) -> Option<Start> {
    let (min_y, height, sea_level) = dimension(&case.dimension);
    let id = frozen.structure_ids[&ResourceLocation::parse(&case.structure).unwrap()];
    let states = &program().world;
    let mut world = FlatSiteWorld {
        min_y,
        max_y: min_y + height - 1,
        layers: layers(case.base, min_y)
            .into_iter()
            .map(|state| state.unwrap_or(states.air))
            .collect(),
        biome: biome_index().get(&case.biome).expect("a corpus biome"),
        states,
        predicates: predicates(),
    };
    let mut ctx = Context {
        frozen,
        structure: &frozen.structures[id.0 as usize],
        chunk: case.chunk,
        seed: case.seed,
        height: HeightContext {
            min_y,
            depth: height,
            sea_level,
        },
        accessor_min_y: min_y,
        accessor_height: height,
        world: &mut world,
    };
    let site = site(&mut ctx).filter(|site| site.biome_ok)?;
    let pieces = layout(&mut ctx, site);
    (!pieces.is_empty()).then(|| Start::new(frozen, id, pieces))
}

/// The oracle's stub level around the start: the base layers, heights answered
/// from the layers alone, and every write logged.
fn region(case: &DumpCase, bounds: BoundingBox) -> BoxRegion {
    let (min_y, height, sea_level) = dimension(&case.dimension);
    let world = WorldStates::clone(&program().world);
    let air = world.air;
    let layers = layers(case.base, min_y);
    let mut region = BoxRegion::new(
        BlockPos::new(bounds.min.x - 16, min_y, bounds.min.z - 16),
        BlockPos::new(bounds.max.x + 16, min_y + height - 1, bounds.max.z + 16),
        air,
    );
    for (layer, state) in layers.iter().enumerate() {
        if let Some(state) = state {
            region.blocks.fill_layer(min_y + layer as i32, *state);
        }
    }
    region.world = world;
    region.extent.sea_level = sea_level;
    region.biome = biome_index().get(&case.biome).expect("a corpus biome");
    let predicates = predicates().clone();
    region.with_height(move |blocks: &BoxVolume, kind: HeightmapName, _, _| {
        let max_y = blocks.max().y;
        let top = (layers.len() as i32 - 1).min(max_y - min_y);
        (0..=top)
            .rev()
            .find(|&layer| {
                layers[layer as usize]
                    .is_some_and(|state| predicates.get(state).contains(heightmap_kind(kind)))
            })
            .map_or(min_y, |layer| min_y + layer + 1)
    })
}

fn palette_index() -> &'static HashMap<VoxelId, u32> {
    static INDEX: LazyLock<HashMap<VoxelId, u32>> = LazyLock::new(|| {
        dump()
            .palette
            .iter()
            .enumerate()
            .map(|(index, text)| (resolve(&parse_state(text)), index as u32))
            .collect()
    });
    &INDEX
}

/// The stub level creates a position's block entity once and reloads it from
/// every later template block there, so one entry per position stands with
/// the last data; a later write of a block that holds none leaves it in place,
/// which the column merge, unlike the stub, drops.
fn by_position(entities: &[GeneratedBlockEntity]) -> Vec<GeneratedBlockEntity> {
    let mut settled: Vec<GeneratedBlockEntity> = Vec::new();
    for entity in entities {
        match settled
            .iter_mut()
            .find(|held| held.position() == entity.position())
        {
            Some(held) => *held = entity.clone(),
            None => settled.push(entity.clone()),
        }
    }
    settled
}

/// The oracle records each spawn as it reached `addFreshEntity`: a vehicle,
/// then each of its passengers.
fn arrivals(entity: &GeneratedEntity, out: &mut Vec<NbtCompound>) {
    let mut compound = to_nbt_compound(entity).expect("an entity serialises");
    compound
        .child_tags
        .retain(|(key, _)| !matches!(key.as_str(), "UUID" | "Passengers"));
    out.push(compound);
    for passenger in &entity.passengers {
        arrivals(passenger, out);
    }
}

/// The oracle's tag is the entity's whole save and ours holds what generation
/// decided, so the comparison is over the keys ours carries; an entity's own
/// random values are masked on both sides.
fn projected(expected: &NbtCompound, ours: &NbtCompound) -> NbtCompound {
    let mut out = NbtCompound::new();
    for (key, _) in &ours.child_tags {
        if let Some(value) = expected.get(key) {
            out.child_tags.push((key.clone(), value.clone()));
        }
    }
    canonical(&out)
}

fn compare_chunk(
    label: &str,
    expected: &DumpChunk,
    writes: &[(BlockPos, VoxelId)],
    block_entities: &[GeneratedBlockEntity],
    spawns: &[GeneratedEntity],
    rng: [i64; 2],
) -> Vec<String> {
    let mut faults = Vec::new();
    let fault = |faults: &mut Vec<String>, what: String| faults.push(format!("{label}: {what}"));
    let written = written(writes);
    let palette = palette_index();
    let mut entries = Vec::with_capacity(written.len());
    for (pos, state) in &written {
        match palette.get(state) {
            Some(index) => entries.push((pos.to_array(), *index)),
            None => fault(
                &mut faults,
                format!(
                    "{} at {:?} is outside the oracle's palette",
                    state_named(*state),
                    pos.to_array()
                ),
            ),
        }
    }
    if written.len() as u32 != expected.count {
        fault(
            &mut faults,
            format!("count: want {}, got {}", expected.count, written.len()),
        );
    }
    if let Some(full) = &expected.full {
        if let Some((at, (want, got))) = full
            .iter()
            .zip(&entries)
            .enumerate()
            .find(|(_, (want, got))| want != got)
        {
            fault(
                &mut faults,
                format!(
                    "entry {at}: want {} at {:?}, got {} at {:?}",
                    dump().palette[want.1 as usize],
                    want.0,
                    dump().palette[got.1 as usize],
                    got.0
                ),
            );
        }
    } else if faults.is_empty() && fnv(&entries) != expected.hash {
        fault(&mut faults, "hash differs with the same count".to_owned());
    }

    let mut ours: Vec<_> = block_entities.iter().collect();
    ours.sort_by_key(|entity| entity.position().to_array());
    if ours.len() != expected.block_entities.len() {
        fault(
            &mut faults,
            format!(
                "block entities: want {} ({}), got {}",
                expected.block_entities.len(),
                expected
                    .block_entities
                    .iter()
                    .map(|(pos, type_id, _)| format!("{type_id} at {pos:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                ours.len()
            ),
        );
    } else {
        for ((pos, type_id, want), mine) in expected.block_entities.iter().zip(&ours) {
            let got = to_nbt_compound(mine).unwrap_or_else(|e| panic!("{label}: {e}"));
            if got.get_string("id") != Some(type_id.as_str()) || mine.position().to_array() != *pos
            {
                fault(
                    &mut faults,
                    format!(
                        "block entity: want {type_id} at {pos:?}, got {:?} at {:?}",
                        got.get_string("id"),
                        mine.position().to_array()
                    ),
                );
                break;
            }
            let (want, got) = (canonical(want), canonical(&got));
            if want != got {
                fault(
                    &mut faults,
                    format!("block entity {type_id} at {pos:?}:\n  want {want:?}\n  got  {got:?}"),
                );
                break;
            }
        }
    }
    let mut ours = Vec::new();
    for spawn in spawns {
        arrivals(spawn, &mut ours);
    }
    if ours.len() != expected.entities.len() {
        fault(
            &mut faults,
            format!(
                "entities: want {} ({}), got {} ({})",
                expected.entities.len(),
                expected
                    .entities
                    .iter()
                    .map(|entity| format!("{} at {:?}", entity.type_id, entity.nbt.get_list("Pos")))
                    .collect::<Vec<_>>()
                    .join(", "),
                ours.len(),
                ours.iter()
                    .map(|entity| format!(
                        "{:?} at {:?}",
                        entity.get_string("id"),
                        entity.get_list("Pos")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    } else {
        for (want, got) in expected.entities.iter().zip(&ours) {
            let id = got.get_string("id").map(str::to_owned);
            let mut got = got.clone();
            got.child_tags.retain(|(key, _)| key != "id");
            let got = canonical(&got);
            let want_nbt = projected(&want.nbt, &got);
            if id.as_deref() != Some(want.type_id.as_str()) || want_nbt != got {
                fault(
                    &mut faults,
                    format!(
                        "entity {}:\n  want {want_nbt:?}\n  got  {got:?}",
                        want.type_id
                    ),
                );
                break;
            }
        }
    }
    if rng != expected.rng {
        fault(
            &mut faults,
            format!(
                "random after placement: want {:?}, got {rng:?}",
                expected.rng
            ),
        );
    }
    faults
}

#[test]
fn structure_geometry_matches_the_oracle_chunk_by_chunk() {
    let dump = dump();
    let frozen = frozen_shared();
    let program = program();
    let mut unported = BTreeSet::new();
    let mut faults = Vec::new();
    let mut placed = 0;
    let mut chunks = 0;
    for case in &dump.cases {
        let id = frozen.structure_ids[&ResourceLocation::parse(&case.structure).unwrap()];
        let structure = &frozen.structures[id.0 as usize];
        let label = format!(
            "{} {} base {} at {:?}",
            case.structure, case.biome, case.base, case.chunk
        );
        let ours = flat_start(frozen, case);
        let Some(expected) = &case.start else {
            assert!(
                ours.is_none(),
                "{label}: a start the reference does not have"
            );
            continue;
        };
        let Some(start) = ours else {
            unported.insert(structure.kind.type_name());
            continue;
        };
        assert_eq!(start.bounds, expected.bounds, "{label}: start box");
        assert_eq!(
            start.pieces.len() as u32,
            expected.pieces,
            "{label}: pieces"
        );
        assert_eq!(structure.step as u8, expected.step, "{label}: step");
        assert_eq!(structure.step_index, expected.index, "{label}: step index");
        let (min_y, height, _) = dimension(&case.dimension);
        let y_sections: Vec<i32> = (min_y >> 4..(min_y + height) >> 4).collect();
        let liquid = match &structure.kind {
            StructureKind::Jigsaw { config, .. } => config.liquid_settings,
            _ => LiquidSettings::default(),
        };
        let mut region = region(case, start.bounds);
        for chunk in &expected.chunks {
            region.writes.clear();
            let mut run = program.run(RunScratch::default());
            let mut rng = XoroshiroRandom::new(chunk.stream_seed as u64);
            place_start(
                frozen,
                program,
                &mut run,
                &mut region,
                &start,
                case.chunk,
                column_clip(chunk.chunk, &y_sections),
                &mut rng,
                liquid,
            );
            let (block_entities, spawns, _) = run.finish();
            faults.extend(compare_chunk(
                &format!("{label} chunk {:?}", chunk.chunk),
                chunk,
                &region.writes,
                &by_position(&block_entities),
                &spawns,
                [rng.next_i64(), rng.next_i64()],
            ));
            chunks += 1;
        }
        placed += 1;
    }
    assert!(
        faults.is_empty(),
        "{} chunks differ; the first 20:\n{}",
        faults.len(),
        faults[..faults.len().min(20)].join("\n")
    );
    assert_eq!(unported, UNPORTED_GEOMETRY_TYPES.into_iter().collect());
    assert_eq!((placed, chunks), (48, 687));
}
