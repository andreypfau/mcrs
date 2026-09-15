use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use bevy_math::IVec3;
use bytes::Buf;
use mcrs_minecraft_chunk::{Blocks, BoxVolume, Volume, VoxelId};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{Nbt, to_nbt_compound};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::block_pos_seed;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::compile::CompiledPlacedFeature;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, WorldStates};
use mcrs_minecraft_worldgen_feature::proto::{
    Feature, Holder, PlacedFeature, ProcessorRule, StructureProcessor, processor_list,
};
use mcrs_minecraft_worldgen_feature::rule_test::RuleTest;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::template::{
    ChainKind, Placement, SettingsRandom, compile_chain, mirror_state, place_template, rotate_state,
};
use mcrs_minecraft_worldgen_structure::LiquidSettings;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};

use super::structures::frozen_shared;
use super::template_manifest::{freeze, parse_state, resolve};
use super::{
    biome_registry, block_tags, blocks, build_program_with, corpus, corpus_features, fluid_tags,
    one_step,
};
use crate::feature_program::{FeatureProgram, RunScratch};
use crate::structures::place::place_element;
use mcrs_minecraft_worldgen_feature_place::block_entity::BLOCK_ENTITY_TYPES;
use mcrs_minecraft_worldgen_structure::frozen::{ElementId, FrozenElement};

const MAGIC: &[u8; 8] = b"MCTMPLP2";
const BIOME: &str = "minecraft:plains";
const WORLD_SEED: i64 = 0x5EED;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

struct DumpEntity {
    pos: [i32; 3],
    type_id: String,
    nbt: NbtCompound,
}

struct DumpPlacement {
    floor: u8,
    seed: i64,
    template_drawn: String,
    rotation_drawn: u8,
    pos: [i32; 3],
    placed: bool,
    count: u32,
    hash: u64,
    full: Option<Vec<([i32; 3], u32)>>,
    entities: Vec<DumpEntity>,
    rng: [i64; 2],
}

type CaseKey = (String, String, u8, u8);

struct DumpCase {
    kind: u8,
    key: CaseKey,
    rotation: u8,
    liquid: u8,
    mirror: u8,
    pivot: [i32; 3],
    position: [i32; 3],
    reference: [i32; 3],
    clip: Option<BoundingBox>,
    placements: Vec<DumpPlacement>,
}

struct Dump {
    types: Vec<String>,
    blocks: Vec<(String, String, bool)>,
    rotation_palette: Vec<String>,
    rotations: Vec<[u32; 6]>,
    palette: Vec<String>,
    cases: Vec<DumpCase>,
}

fn read_dump() -> Dump {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/template_placement.bin");
    let mut r = open_dump(&path, MAGIC);
    let vec3 = |r: &mut bytes::Bytes| [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
    let types = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
    let blocks = (0..r.get_u32_le())
        .map(|_| (dump_string(&mut r), dump_string(&mut r), r.get_u8() == 1))
        .collect();
    let rotation_palette = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
    let rotations = (0..r.get_u32_le())
        .map(|_| std::array::from_fn(|_| r.get_u32_le()))
        .collect();
    let palette = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
    let cases = (0..r.get_u32_le())
        .map(|_| {
            let kind = r.get_u8();
            let template_key = dump_string(&mut r);
            let processors_key = dump_string(&mut r);
            let projection = r.get_u8();
            let legacy = r.get_u8();
            let rotation = r.get_u8();
            let liquid = r.get_u8();
            let mirror = r.get_u8();
            let pivot = vec3(&mut r);
            let position = vec3(&mut r);
            let reference = vec3(&mut r);
            let clip = (r.get_u8() == 1).then(|| {
                let min = vec3(&mut r);
                let max = vec3(&mut r);
                BoundingBox {
                    min: IVec3::from_array(min).into(),
                    max: IVec3::from_array(max).into(),
                }
            });
            let placements = (0..r.get_u32_le())
                .map(|_| {
                    let floor = r.get_u8();
                    let seed = r.get_i64_le();
                    let template_drawn = dump_string(&mut r);
                    let rotation_drawn = r.get_u8();
                    let pos = vec3(&mut r);
                    let placed = r.get_u8() == 1;
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
                    let entities = (0..r.get_u32_le())
                        .map(|_| {
                            let pos = vec3(&mut r);
                            let type_id = dump_string(&mut r);
                            let len = r.get_u32_le() as usize;
                            let bytes = r.copy_to_bytes(len);
                            let nbt = Nbt::read(&mut NbtReadHelper::new(Cursor::new(&bytes[..])))
                                .unwrap_or_else(|e| panic!("block entity at {pos:?}: {e}"))
                                .root_tag;
                            DumpEntity { pos, type_id, nbt }
                        })
                        .collect();
                    let rng = [r.get_i64_le(), r.get_i64_le()];
                    DumpPlacement {
                        floor,
                        seed,
                        template_drawn,
                        rotation_drawn,
                        pos,
                        placed,
                        count,
                        hash,
                        full,
                        entities,
                        rng,
                    }
                })
                .collect();
            DumpCase {
                kind,
                key: (template_key, processors_key, projection, legacy),
                rotation,
                liquid,
                mirror,
                pivot,
                position,
                reference,
                clip,
                placements,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    Dump {
        types,
        blocks,
        rotation_palette,
        rotations,
        palette,
        cases,
    }
}

fn dump() -> &'static Dump {
    static DUMP: LazyLock<Dump> = LazyLock::new(read_dump);
    &DUMP
}

/// The parity program: the corpus over one biome with every pool element
/// compiled, at the world seed the oracle's stub level answers.
pub(super) fn program() -> &'static FeatureProgram {
    static PROGRAM: LazyLock<FeatureProgram> = LazyLock::new(|| {
        build_program_with(
            &one_step(vec![], BIOME),
            corpus_features(),
            &biome_registry(&[BIOME]),
            WORLD_SEED,
            Some(&**frozen_shared()),
        )
    });
    &PROGRAM
}

pub(super) fn state_named(id: VoxelId) -> String {
    format!("{}#{}", block_name(id), id.0)
}

fn block_name(id: VoxelId) -> &'static str {
    let index = corpus().block_index(BlockStateId(id.0));
    corpus().blocks()[index as usize].identifier.as_str()
}

#[test]
fn the_block_entity_type_order_is_the_registry_s() {
    assert_eq!(BLOCK_ENTITY_TYPES.to_vec(), dump().types);
}

#[test]
fn the_loot_seeded_kinds_and_every_template_block_entity_id_are_pinned() {
    let dump = dump();
    let type_of: BTreeMap<&str, &str> = dump
        .blocks
        .iter()
        .map(|(block, kind, _)| (block.as_str(), kind.as_str()))
        .collect();
    let mut seeded: BTreeSet<&str> = BTreeSet::new();
    for (_, kind, loot_seeded) in &dump.blocks {
        if *loot_seeded {
            seeded.insert(kind);
        }
    }
    for id in GeneratedBlockEntity::IDS {
        assert!(
            dump.types.iter().any(|t| t == id),
            "{id} is not a registered block entity type"
        );
        assert_eq!(
            GeneratedBlockEntity::LOOT_SEEDED_IDS.contains(&id),
            seeded.contains(id),
            "{id}: RandomizableContainer membership"
        );
    }
    for sand in ["minecraft:suspicious_sand", "minecraft:suspicious_gravel"] {
        assert_eq!(type_of[sand], "minecraft:brushable_block");
    }

    let frozen = frozen_shared();
    let mut checked = 0;
    for template in &frozen.templates {
        for block in template.palettes.iter().flat_map(|palette| palette.iter()) {
            let Some(nbt) = &block.nbt else { continue };
            let block_id = block_name(block.state);
            assert_eq!(
                nbt.get_string("id"),
                type_of.get(block_id).copied(),
                "{block_id} at {:?}",
                block.pos
            );
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} block entities checked");
}

#[test]
fn every_block_state_rotates_and_mirrors_as_the_reference_does() {
    let dump = dump();
    let program = program();
    let mut resolved: HashMap<&str, VoxelId> = HashMap::new();
    let mut resolve_text = |text: &'static str| {
        *resolved
            .entry(text)
            .or_insert_with(|| resolve(&parse_state(text)))
    };
    let census: HashMap<VoxelId, [VoxelId; 5]> = dump
        .rotations
        .iter()
        .map(|row| {
            let [state, cw, half, ccw, left_right, front_back] =
                row.map(|index| resolve_text(dump.rotation_palette[index as usize].as_str()));
            (state, [cw, half, ccw, left_right, front_back])
        })
        .collect();
    assert_eq!(
        census.len(),
        dump.rotations.len(),
        "two census rows resolve to one state"
    );

    let turns = [
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::Counterclockwise90,
    ];
    let mirrors = [Mirror::LeftRight, Mirror::FrontBack];
    let mut mismatches = Vec::new();
    for id in 0..corpus().state_count() {
        let state = VoxelId(id as u16);
        let expected = census.get(&state).copied().unwrap_or([state; 5]);
        let got = turns
            .map(|turn| rotate_state(&program.world, state, turn))
            .into_iter()
            .chain(mirrors.map(|mirror| mirror_state(&program.world, state, mirror)));
        let labels = ["cw90", "cw180", "ccw90", "left_right", "front_back"];
        for ((label, want), got) in labels.iter().zip(expected).zip(got) {
            if got != want {
                mismatches.push(format!(
                    "{} {label}: want {}, got {}",
                    state_named(state),
                    state_named(want),
                    state_named(got)
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} states rotate or mirror differently; the first 20:\n{}",
        mismatches.len(),
        mismatches[..mismatches.len().min(20)].join("\n")
    );
}

fn id_of(entity: &GeneratedBlockEntity) -> String {
    to_nbt_compound(entity)
        .unwrap()
        .get_string("id")
        .unwrap()
        .to_owned()
}

pub(super) fn canonical(compound: &NbtCompound) -> NbtCompound {
    let mut child_tags: Vec<(String, NbtTag)> = compound
        .child_tags
        .iter()
        .filter(|(_, value)| match value {
            NbtTag::List(items) => !items.is_empty(),
            NbtTag::Compound(inner) => !inner.child_tags.is_empty(),
            _ => true,
        })
        .map(|(key, value)| (key.clone(), canonical_tag(value)))
        .collect();
    child_tags.sort_by(|a, b| a.0.cmp(&b.0));
    NbtCompound { child_tags }
}

fn canonical_tag(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(inner) => NbtTag::Compound(canonical(inner)),
        NbtTag::List(items) => NbtTag::List(items.iter().map(canonical_tag).collect()),
        other => other.clone(),
    }
}

pub(super) fn fnv(entries: &[([i32; 3], u32)]) -> u64 {
    let mut hash = FNV_OFFSET;
    for (pos, index) in entries {
        for word in [pos[0], pos[1], pos[2], *index as i32] {
            for byte in word.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
    }
    hash
}

/// First write fixes the order, last write fixes the state.
pub(super) fn written(writes: &[(BlockPos, VoxelId)]) -> Vec<(BlockPos, VoxelId)> {
    let mut slot: HashMap<BlockPos, usize> = HashMap::new();
    let mut out: Vec<(BlockPos, VoxelId)> = Vec::new();
    for &(pos, state) in writes {
        match slot.get(&pos) {
            Some(&at) => out[at].1 = state,
            None => {
                slot.insert(pos, out.len());
                out.push((pos, state));
            }
        }
    }
    out
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

fn region(min: BlockPos, max: BlockPos, floor: u8) -> BoxRegion {
    let world = WorldStates::clone(&program().world);
    let air = world.air_states.clone();
    let state = |block: &str| VoxelId::from(corpus().default_state(block));
    let region = BoxRegion::new(min, max, world.air);
    let mut region = match floor {
        0 => region.floor(63, state("minecraft:dirt")),
        1 => region
            .floor(63, state("minecraft:water"))
            .floor(60, state("minecraft:stone")),
        _ => region
            .floor(63, state("minecraft:lava"))
            .floor(60, state("minecraft:stone")),
    };
    region.world = world;
    region.with_height(move |blocks: &BoxVolume, _: HeightmapName, x, z| {
        let (min, max) = (blocks.min(), blocks.max());
        (min.y..=max.y)
            .rev()
            .find(|&y| !air.contains(blocks.get(BlockPos::new(x, y, z)).0 as usize))
            .map_or(min.y, |y| y + 1)
    })
}

struct Outcome {
    placed: bool,
    writes: Vec<(BlockPos, VoxelId)>,
    entities: Vec<GeneratedBlockEntity>,
    rng: [i64; 2],
}

/// Every way one placement can differ from the oracle's, as one line each.
fn compare(label: &str, expected: &DumpPlacement, got: &Outcome) -> Vec<String> {
    let mut faults = Vec::new();
    let fault = |faults: &mut Vec<String>, what: String| faults.push(format!("{label}: {what}"));
    if got.placed != expected.placed {
        fault(
            &mut faults,
            format!("placed: want {}, got {}", expected.placed, got.placed),
        );
    }
    let written = written(&got.writes);
    let palette = palette_index();
    let mut entries = Vec::with_capacity(written.len());
    let mut unknown = Vec::new();
    for (pos, state) in &written {
        match palette.get(state) {
            Some(index) => entries.push((pos.to_array(), *index)),
            None => unknown.push(format!("{} at {:?}", state_named(*state), pos.to_array())),
        }
    }
    if !unknown.is_empty() {
        fault(
            &mut faults,
            format!(
                "{} states outside the oracle's palette, first {}",
                unknown.len(),
                unknown[0]
            ),
        );
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

    let mut ours: Vec<&GeneratedBlockEntity> = got.entities.iter().collect();
    ours.sort_by_key(|entity| entity.position().to_array());
    if ours.len() != expected.entities.len() {
        fault(
            &mut faults,
            format!(
                "block entities: want {} ({}), got {} ({})",
                expected.entities.len(),
                expected
                    .entities
                    .iter()
                    .map(|entity| format!("{} at {:?}", entity.type_id, entity.pos))
                    .collect::<Vec<_>>()
                    .join(", "),
                ours.len(),
                ours.iter()
                    .map(|entity| format!(
                        "{} at {:?}",
                        id_of(entity),
                        entity.position().to_array()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    } else {
        for (theirs, mine) in expected.entities.iter().zip(&ours) {
            let encoded = to_nbt_compound(mine).unwrap_or_else(|e| panic!("{label}: {e}"));
            if encoded.get_string("id") != Some(theirs.type_id.as_str())
                || mine.position().to_array() != theirs.pos
            {
                fault(
                    &mut faults,
                    format!(
                        "block entity: want {} at {:?}, got {} at {:?}",
                        theirs.type_id,
                        theirs.pos,
                        id_of(mine),
                        mine.position().to_array()
                    ),
                );
                break;
            }
            let (want, got) = (canonical(&theirs.nbt), canonical(&encoded));
            if want != got {
                fault(
                    &mut faults,
                    format!(
                        "block entity {} at {:?}:\n  want {want:?}\n  got  {got:?}",
                        theirs.type_id, theirs.pos
                    ),
                );
                break;
            }
        }
    }
    if got.rng != expected.rng {
        fault(
            &mut faults,
            format!(
                "random after placement: want {:?}, got {:?}",
                expected.rng, got.rng
            ),
        );
    }
    faults
}

fn element_key(
    frozen: &mcrs_minecraft_worldgen_structure::frozen::FrozenStructures,
) -> Vec<(CaseKey, ElementId)> {
    let template_named: BTreeMap<u32, &ResourceLocation> = frozen
        .template_ids
        .iter()
        .map(|(id, template)| (template.0, id))
        .collect();
    frozen
        .elements
        .iter()
        .enumerate()
        .filter_map(|(index, element)| {
            let FrozenElement::Single {
                template,
                legacy,
                processors,
                projection,
                ..
            } = element
            else {
                return None;
            };
            let processors = match processors {
                Holder::Reference(id) => format!("ref:{id}"),
                Holder::Inline(list) => {
                    assert!(
                        processor_list(list).is_empty(),
                        "element {index}: a non-empty inline processor list"
                    );
                    "inline".to_owned()
                }
            };
            Some((
                (
                    template_named[&template.0].to_string(),
                    processors,
                    *projection as u8,
                    u8::from(*legacy),
                ),
                ElementId(index as u32),
            ))
        })
        .collect()
}

fn assert_same_keys(kind: &str, ours: &BTreeSet<CaseKey>, theirs: &BTreeSet<CaseKey>) {
    let missing: Vec<_> = theirs.difference(ours).collect();
    let extra: Vec<_> = ours.difference(theirs).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{kind} cases: {} only in the fixture ({:?}), {} only here ({:?})",
        missing.len(),
        &missing[..missing.len().min(5)],
        extra.len(),
        &extra[..extra.len().min(5)]
    );
}

fn report(faults: &[String], placements: usize) {
    assert!(
        faults.is_empty(),
        "{} of {placements} placements differ; the first 20:\n{}",
        faults.len(),
        faults[..faults.len().min(20)].join("\n")
    );
}

#[test]
fn every_pool_element_places_as_the_reference_does() {
    let dump = dump();
    let program = program();
    let frozen = frozen_shared();
    let mut by_key: BTreeMap<CaseKey, ElementId> = BTreeMap::new();
    for (key, element) in element_key(frozen) {
        by_key.entry(key).or_insert(element);
    }
    let fixture_keys: BTreeSet<CaseKey> = dump
        .cases
        .iter()
        .filter(|case| case.kind == 0)
        .map(|case| case.key.clone())
        .collect();
    assert_same_keys("pool", &by_key.keys().cloned().collect(), &fixture_keys);

    let mut faults = Vec::new();
    let mut placements = 0;
    for case in dump.cases.iter().filter(|case| case.kind == 0) {
        let element = by_key[&case.key];
        let rotation = Rotation::ALL[case.rotation as usize];
        let liquid = match case.liquid {
            0 => LiquidSettings::ApplyWaterlogging,
            _ => LiquidSettings::IgnoreWaterlogging,
        };
        for placement in &case.placements {
            placements += 1;
            let label = format!(
                "{} {} projection {} legacy {} rotation {rotation:?} liquid {liquid:?} floor {} seed {}",
                case.key.0, case.key.1, case.key.2, case.key.3, placement.floor, placement.seed
            );
            let mut region = region(
                BlockPos::new(-48, -64, -48),
                BlockPos::new(63, 319, 63),
                placement.floor,
            );
            let mut run = program.run(RunScratch::default());
            let mut rng = XoroshiroRandom::new(placement.seed as u64);
            let placed = place_element(
                program,
                &mut run,
                &mut region,
                element,
                IVec3::from_array(case.position),
                IVec3::from_array(case.reference),
                rotation,
                case.clip,
                &mut rng,
                liquid,
            );
            let (entities, _) = run.finish();
            let outcome = Outcome {
                placed,
                writes: region.writes,
                entities,
                rng: [rng.next_i64(), rng.next_i64()],
            };
            faults.extend(compare(&label, placement, &outcome));
        }
    }
    report(&faults, placements);
}

fn feature_key(feature: &Feature) -> Option<CaseKey> {
    let Feature::Template {
        templates,
        processors,
    } = feature
    else {
        return None;
    };
    let ids = templates
        .iter()
        .map(|entry| entry.data.id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let processors = match processors {
        Some(Holder::Reference(id)) => format!("ref:{id}"),
        Some(Holder::Inline(_)) => "inline".to_owned(),
        None => "none".to_owned(),
    };
    Some((ids, processors, 0, 0))
}

#[test]
fn every_template_feature_places_as_the_reference_does() {
    let dump = dump();
    let mut nodes: BTreeMap<CaseKey, Feature> = BTreeMap::new();
    for feature in corpus_features().features.values() {
        feature.for_each_feature(&mut |node| {
            if let Some(key) = feature_key(node) {
                nodes.entry(key).or_insert_with(|| node.clone());
            }
        });
    }
    let fixture_keys: BTreeSet<CaseKey> = dump
        .cases
        .iter()
        .filter(|case| case.kind == 1)
        .map(|case| case.key.clone())
        .collect();
    assert_same_keys("feature", &nodes.keys().cloned().collect(), &fixture_keys);

    let mut faults = Vec::new();
    let mut placements = 0;
    for case in dump.cases.iter().filter(|case| case.kind == 1) {
        let node = &nodes[&case.key];
        let tables = one_step(
            vec![Arc::new(CompiledPlacedFeature {
                id: None,
                placed: PlacedFeature {
                    feature: Holder::Inline(Box::new(node.clone())),
                    placement: Vec::new(),
                },
            })],
            BIOME,
        );
        let program = build_program_with(
            &tables,
            corpus_features(),
            &biome_registry(&[BIOME]),
            WORLD_SEED,
            None,
        );
        let generator = program
            .generator_at(0, 0)
            .unwrap_or_else(|| panic!("{:?} compiled to no generator", case.key));
        for placement in &case.placements {
            placements += 1;
            let label = format!(
                "{} {} floor {} seed {} (drew {} rotation {} at {:?})",
                case.key.0,
                case.key.1,
                placement.floor,
                placement.seed,
                placement.template_drawn,
                placement.rotation_drawn,
                placement.pos
            );
            let mut region = region(
                BlockPos::new(-16, 40, -16),
                BlockPos::new(31, 99, 31),
                placement.floor,
            );
            let mut run = program.run(RunScratch::default());
            let mut rng = XoroshiroRandom::new(placement.seed as u64);
            let placed = generator.place(
                &mut run,
                &mut region,
                &mut rng,
                IVec3::from_array(case.position).into(),
                &|_| true,
            );
            let (entities, _) = run.finish();
            let outcome = Outcome {
                placed,
                writes: region.writes,
                entities,
                rng: [rng.next_i64(), rng.next_i64()],
            };
            faults.extend(compare(&label, placement, &outcome));
        }
    }
    report(&faults, placements);
}

/// `RuinedPortalPiece.makeSettings`, from the properties the case key spells.
fn portal_processors(key: &str) -> Vec<StructureProcessor> {
    let fields: BTreeMap<&str, &str> = key
        .strip_prefix("portal:")
        .unwrap()
        .split(',')
        .map(|field| field.split_once('=').unwrap_or(("placement", field)))
        .collect();
    let flag = |name: &str| fields[name] == "true";
    let (cold, air_pocket, blackstone) = (flag("cold"), flag("air_pocket"), flag("blackstone"));
    let mossiness: f32 = fields["mossiness"].parse().unwrap();
    let state = |name: &str| BlockState {
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
        output_state: state(target),
        block_entity_modifier: None,
    };
    let lava = if fields["placement"] == "on_ocean_floor" {
        replace("lava", None, "magma_block")
    } else if cold {
        replace("lava", None, "netherrack")
    } else {
        replace("lava", Some(0.2), "magma_block")
    };
    let mut rules = vec![replace("gold_block", Some(0.3), "air"), lava];
    if !cold {
        rules.push(replace("netherrack", Some(0.07), "magma_block"));
    }
    let ignored = if air_pocket {
        vec![state("structure_block")]
    } else {
        vec![state("air"), state("structure_block")]
    };
    let mut list = vec![
        StructureProcessor::BlockIgnore { blocks: ignored },
        StructureProcessor::Rule { rules },
        StructureProcessor::BlockAge {
            mossiness: f64::from(mossiness),
        },
        StructureProcessor::ProtectedBlocks {
            value: mcrs_minecraft_core::HolderSet::Tag(ResourceLocation::minecraft(
                "features_cannot_replace",
            )),
        },
        StructureProcessor::LavaSubmergedBlock,
    ];
    if blackstone {
        list.push(StructureProcessor::BlackstoneReplace);
    }
    list
}

#[test]
fn every_ruined_portal_chain_places_as_the_reference_does() {
    let dump = dump();
    let biomes = biome_registry(&[BIOME]);
    let resolver = crate::feature_program::Resolver::new(
        &blocks().0,
        Some(block_tags()),
        Some(fluid_tags()),
        &biomes,
        WORLD_SEED,
        &[],
    )
    .expect("the corpus resolves");
    let mut templates: BTreeMap<&str, mcrs_minecraft_worldgen_feature::template::FrozenTemplate> =
        BTreeMap::new();
    let mut faults = Vec::new();
    let mut placements = 0;
    let cases: Vec<&DumpCase> = dump.cases.iter().filter(|case| case.kind == 2).collect();
    assert_eq!(cases.len(), 78);
    for case in cases {
        let template = templates
            .entry(case.key.0.as_str())
            .or_insert_with(|| freeze(&case.key.0).1);
        let chain = compile_chain(
            &portal_processors(&case.key.1),
            ChainKind::Feature,
            &resolver,
            WORLD_SEED,
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.key.1));
        let rotation = Rotation::ALL[case.rotation as usize];
        let mirror = Mirror::ALL[case.mirror as usize];
        let position = IVec3::from_array(case.position);
        let palette = LegacyRandom::new(block_pos_seed(position))
            .next_i32_bound(template.palettes.len() as i32) as usize;
        for placement in &case.placements {
            placements += 1;
            let label = format!(
                "{} {} rotation {rotation:?} mirror {mirror:?} floor {} seed {}",
                case.key.0, case.key.1, placement.floor, placement.seed
            );
            let mut region = region(
                BlockPos::new(-48, -64, -48),
                BlockPos::new(63, 319, 63),
                placement.floor,
            );
            let mut rng = XoroshiroRandom::new(placement.seed as u64);
            let mut entities = Vec::new();
            let placed = place_template(
                &Placement {
                    template,
                    jigsaws: &[],
                    palette,
                    position,
                    reference: IVec3::from_array(case.reference),
                    rotation,
                    mirror,
                    pivot: IVec3::from_array(case.pivot),
                    random: SettingsRandom::Positional,
                    clip: case.clip,
                    chain: &chain,
                    waterlog: true,
                },
                &mut region,
                &mut rng,
                &mut entities,
            );
            let outcome = Outcome {
                placed,
                writes: region.writes,
                entities,
                rng: [rng.next_i64(), rng.next_i64()],
            };
            faults.extend(compare(&label, placement, &outcome));
        }
    }
    report(&faults, placements);
}
