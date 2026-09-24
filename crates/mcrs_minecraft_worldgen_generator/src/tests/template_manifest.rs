use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::PathBuf;

use bytes::Buf;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_worldgen_feature::template::{
    FrozenTemplate, Joint, PaletteState, Template, TemplateManifest,
};
use mcrs_minecraft_worldgen_testing::{assets_dir, dump_placements, dump_string, open_dump};

use super::corpus;
use crate::structures::{DYNAMIC_SHAPE_BLOCKS, resolve_palette_state};

const MAGIC: &[u8; 8] = b"MCTMPLT0";

struct DumpJigsaw {
    pos: [i32; 3],
    front: String,
    top: String,
    joint: String,
    name: String,
    pool: String,
    target: String,
    placement: i32,
    selection: i32,
    final_state: String,
}

struct DumpPalette {
    full: u32,
    other: u32,
    entity: u32,
    entries: Vec<String>,
    jigsaws: Vec<DumpJigsaw>,
}

struct DumpManifest {
    id: String,
    size: [i32; 3],
    palettes: Vec<DumpPalette>,
}

struct DumpListed {
    id: String,
    palettes: Vec<(Vec<([i32; 3], String)>, Vec<u8>)>,
}

fn read_dump() -> (Vec<String>, Vec<DumpManifest>, Vec<DumpListed>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/templates.bin");
    let mut r = open_dump(&path, MAGIC);
    let dynamic = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
    let manifests = (0..r.get_u32_le())
        .map(|_| DumpManifest {
            id: dump_string(&mut r),
            size: [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()],
            palettes: (0..r.get_u32_le())
                .map(|_| {
                    let (full, other, entity) = (r.get_u32_le(), r.get_u32_le(), r.get_u32_le());
                    let entries = (0..r.get_u32_le()).map(|_| dump_string(&mut r)).collect();
                    let jigsaws = (0..r.get_u32_le())
                        .map(|_| {
                            let pos = [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
                            let _state = dump_string(&mut r);
                            DumpJigsaw {
                                pos,
                                front: dump_string(&mut r),
                                top: dump_string(&mut r),
                                joint: dump_string(&mut r),
                                name: dump_string(&mut r),
                                pool: dump_string(&mut r),
                                target: dump_string(&mut r),
                                placement: r.get_i32_le(),
                                selection: r.get_i32_le(),
                                final_state: {
                                    let _raw = dump_string(&mut r);
                                    dump_string(&mut r)
                                },
                            }
                        })
                        .collect();
                    DumpPalette {
                        full,
                        other,
                        entity,
                        entries,
                        jigsaws,
                    }
                })
                .collect(),
        })
        .collect();
    let listed = (0..r.get_u32_le())
        .map(|_| DumpListed {
            id: dump_string(&mut r),
            palettes: (0..r.get_u32_le())
                .map(|_| {
                    let blocks = dump_placements(&mut r);
                    let bits = r.copy_to_bytes(blocks.len().div_ceil(8)).to_vec();
                    (blocks, bits)
                })
                .collect(),
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    (dynamic, manifests, listed)
}

pub(super) fn parse_state(text: &str) -> PaletteState {
    let (name, properties) = match text.split_once('[') {
        Some((name, rest)) => (
            name,
            Some(
                rest.trim_end_matches(']')
                    .split(',')
                    .filter(|kv| !kv.is_empty())
                    .map(|kv| {
                        let (k, v) = kv.split_once('=').unwrap_or_else(|| panic!("{text}: {kv}"));
                        (k.to_owned(), v.to_owned())
                    })
                    .collect::<BTreeMap<_, _>>(),
            ),
        ),
        None => (text, None),
    };
    PaletteState {
        id: ResourceLocation::parse(name).unwrap(),
        properties,
    }
}

pub(super) fn resolve(state: &PaletteState) -> VoxelId {
    resolve_palette_state(corpus(), state)
        .unwrap_or_else(|| panic!("{state} does not resolve"))
        .id
}

pub(super) fn freeze(id: &str) -> (Template, FrozenTemplate, TemplateManifest) {
    let id = ResourceLocation::parse(id).unwrap();
    let path = assets_dir()
        .join("minecraft/structure")
        .join(format!("{}.nbt", id.path()));
    let file = File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let template: Template =
        from_gzip_bytes(file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let (frozen, manifest) = template
        .freeze(&id, &|state| resolve_palette_state(corpus(), state))
        .unwrap_or_else(|e| panic!("{e}"));
    (template, frozen, manifest)
}

fn is_structure_void(state: &str) -> bool {
    state.split('[').next() == Some("minecraft:structure_void")
}

#[test]
#[ignore = "reference parity check; run with --ignored"]
fn every_template_manifest_matches_the_oracle() {
    let (dynamic, manifests, listed) = read_dump();
    assert_eq!(dynamic, DYNAMIC_SHAPE_BLOCKS);
    assert_eq!(manifests.len(), 1511);
    assert_eq!(listed.len(), 33);

    for dump in &manifests {
        let id = &dump.id;
        let (template, frozen, manifest) = freeze(id);
        let frozen = frozen.palettes;
        assert_eq!(manifest.size.map(i32::from), dump.size, "{id}: size");
        assert_eq!(frozen.len(), dump.palettes.len(), "{id}: palette count");
        let file_palettes: Vec<&[PaletteState]> = match (&template.palette, &template.palettes) {
            (Some(p), None) => vec![p],
            (None, Some(ps)) => ps.iter().map(Vec::as_slice).collect(),
            _ => unreachable!("freeze accepted it"),
        };
        for (p, expected) in dump.palettes.iter().enumerate() {
            let entries: Vec<_> = file_palettes[p]
                .iter()
                .map(|state| resolve_palette_state(corpus(), state).unwrap())
                .collect();
            assert_eq!(
                entries.len(),
                expected.entries.len(),
                "{id}[{p}]: entry count"
            );
            for (i, text) in expected.entries.iter().enumerate() {
                assert_eq!(
                    resolve(&parse_state(text)),
                    entries[i].id,
                    "{id}[{p}]: entry {i}"
                );
            }
            let full: HashMap<VoxelId, bool> =
                entries.iter().map(|e| (e.id, e.full_block)).collect();
            let sections: Vec<u8> = frozen[p]
                .iter()
                .map(|b| match (b.nbt.is_some(), full[&b.state]) {
                    (true, _) => 2,
                    (false, true) => 0,
                    (false, false) => 1,
                })
                .collect();
            assert!(sections.is_sorted(), "{id}[{p}]: sections interleave");
            let count = |section| sections.iter().filter(|&&s| s == section).count() as u32;
            assert_eq!(
                (count(0), count(1), count(2)),
                (expected.full, expected.other, expected.entity),
                "{id}[{p}]: section lengths"
            );

            let jigsaws = &manifest.jigsaws[p];
            assert_eq!(
                jigsaws.len(),
                expected.jigsaws.len(),
                "{id}[{p}]: jigsaw count"
            );
            for (j, (ours, theirs)) in jigsaws.iter().zip(&expected.jigsaws).enumerate() {
                let at = format!("{id}[{p}] jigsaw {j}");
                assert_eq!(ours.pos.map(i32::from), theirs.pos, "{at}: pos");
                assert_eq!(ours.front.name(), theirs.front, "{at}: front");
                assert_eq!(ours.top.name(), theirs.top, "{at}: top");
                let joint = match ours.joint {
                    Joint::Rollable => "rollable",
                    Joint::Aligned => "aligned",
                };
                assert_eq!(joint, theirs.joint, "{at}: joint");
                assert_eq!(ours.name.to_string(), theirs.name, "{at}: name");
                assert_eq!(ours.pool.to_string(), theirs.pool, "{at}: pool");
                assert_eq!(ours.target.to_string(), theirs.target, "{at}: target");
                assert_eq!(
                    ours.placement_priority, theirs.placement,
                    "{at}: placement_priority"
                );
                assert_eq!(
                    ours.selection_priority, theirs.selection,
                    "{at}: selection_priority"
                );
                assert!(
                    !theirs.final_state.is_empty(),
                    "{at}: vanilla failed to parse final_state"
                );
                let final_state = (!is_structure_void(&theirs.final_state))
                    .then(|| resolve(&parse_state(&theirs.final_state)));
                assert_eq!(ours.final_state, final_state, "{at}: final_state");
            }
        }
    }

    let mut states: HashMap<String, VoxelId> = HashMap::new();
    for dump in &listed {
        let id = &dump.id;
        let (_, frozen, _) = freeze(id);
        let frozen = frozen.palettes;
        assert_eq!(frozen.len(), dump.palettes.len(), "{id}: palette count");
        for (p, (blocks, bits)) in dump.palettes.iter().enumerate() {
            let expected: Vec<([i32; 3], VoxelId)> = blocks
                .iter()
                .map(|(pos, text)| {
                    let state = *states
                        .entry(text.clone())
                        .or_insert_with(|| resolve(&parse_state(text)));
                    (*pos, state)
                })
                .collect();
            let ours: Vec<([i32; 3], VoxelId)> = frozen[p]
                .iter()
                .map(|b| (b.pos.map(i32::from), b.state))
                .collect();
            assert_eq!(ours, expected, "{id}[{p}]: block list");
            for (i, block) in frozen[p].iter().enumerate() {
                let bit = bits[i >> 3] & (1 << (i & 7)) != 0;
                assert_eq!(block.nbt.is_some(), bit, "{id}[{p}] block {i}: nbt");
            }
        }
    }
}
