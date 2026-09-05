use std::io::Write;
use std::path::{Path, PathBuf};

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::chunk::LIGHT_BYTES;
use crate::region::SECTOR_BYTES;
use crate::{
    AnvilError, Biomes, BlockStateLookup, BlockStates, DATA_VERSION, ErrorKind, Properties,
    RegionFile,
};

const GZIP: u8 = 1;
const ZLIB: u8 = 2;
const NONE: u8 = 3;
const LZ4: u8 = 4;
const EXTERNAL: u8 = 128;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mcrs_anvil_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn region(&self, x: i32, z: i32, slots: &[(i32, i32, u8, Vec<u8>)]) -> PathBuf {
        let path = self.dir.join(format!("r.{x}.{z}.mca"));
        std::fs::write(&path, build_region(slots)).unwrap();
        path
    }

    fn external(&self, x: i32, z: i32, bytes: &[u8]) {
        std::fs::write(self.dir.join(format!("c.{x}.{z}.mcc")), bytes).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Header, then one sector-aligned payload per slot, exactly as `RegionFile`
/// writes them: a big-endian length covering the version byte, the version byte,
/// then the payload.
fn build_region(slots: &[(i32, i32, u8, Vec<u8>)]) -> Vec<u8> {
    let mut header = vec![0u8; 2 * SECTOR_BYTES];
    let mut body = Vec::new();
    for (x, z, version, payload) in slots {
        let sector = 2 + body.len() / SECTOR_BYTES;
        let mut record = Vec::with_capacity(5 + payload.len());
        record.extend_from_slice(&(payload.len() as i32 + 1).to_be_bytes());
        record.push(*version);
        record.extend_from_slice(payload);
        let sector_count = record.len().div_ceil(SECTOR_BYTES);
        record.resize(sector_count * SECTOR_BYTES, 0);
        body.extend_from_slice(&record);

        let slot = ((z & 31) * 32 + (x & 31)) as usize;
        let entry = ((sector as i32) << 8 | sector_count as i32).to_be_bytes();
        header[slot * 4..slot * 4 + 4].copy_from_slice(&entry);
        header[SECTOR_BYTES + slot * 4..SECTOR_BYTES + slot * 4 + 4]
            .copy_from_slice(&1_700_000_000i32.to_be_bytes());
    }
    header.extend_from_slice(&body);
    header
}

fn compress(version: u8, nbt: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    match version {
        GZIP => {
            let mut w = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
            w.write_all(nbt).unwrap();
            w.finish().unwrap();
        }
        ZLIB => {
            let mut w = flate2::write::ZlibEncoder::new(&mut out, flate2::Compression::default());
            w.write_all(nbt).unwrap();
            w.finish().unwrap();
        }
        NONE => out.extend_from_slice(nbt),
        LZ4 => {
            let mut w = lz4_java_wrc::Lz4BlockOutput::new(&mut out);
            w.write_all(nbt).unwrap();
            w.flush().unwrap();
            drop(w);
        }
        other => panic!("no encoder for compression {other}"),
    }
    out
}

fn nbt_bytes(compound: &NbtCompound) -> Vec<u8> {
    mcrs_minecraft_nbt::Nbt::new(String::new(), compound.clone())
        .write()
        .to_vec()
}

fn block(name: &str) -> NbtCompound {
    let mut entry = NbtCompound::new();
    entry.put_string("id", name.to_string());
    entry
}

fn block_with(name: &str, key: &str, value: &str) -> NbtCompound {
    let mut props = NbtCompound::new();
    props.put_string(key, value.to_string());
    let mut entry = block(name);
    entry.put_component("properties", props);
    entry
}

fn container(palette: Vec<NbtTag>, data: Option<Vec<i64>>) -> NbtCompound {
    let mut c = NbtCompound::new();
    c.put_list("palette", palette);
    if let Some(data) = data {
        c.put("data", NbtTag::LongArray(data));
    }
    c
}

/// Packs `entries` the way `SimpleBitStorage` does: `64 / bits` entries per long,
/// lowest entry in the lowest bits, no entry straddling a long boundary.
fn pack(entries: &[u16], bits: u32) -> Vec<i64> {
    let per_long = 64 / bits as usize;
    let mut longs = vec![0i64; entries.len().div_ceil(per_long)];
    for (index, &entry) in entries.iter().enumerate() {
        let shift = (index % per_long) as u32 * bits;
        longs[index / per_long] |= ((entry as u64) << shift) as i64;
    }
    longs
}

fn section(y: i8, block_states: NbtCompound) -> NbtCompound {
    let mut s = NbtCompound::new();
    s.put_byte("Y", y);
    s.put_component("block_states", block_states);
    s
}

fn chunk_nbt(x: i32, z: i32, sections: Vec<NbtTag>) -> NbtCompound {
    chunk_nbt_versioned(x, z, sections, DATA_VERSION)
}

fn chunk_nbt_versioned(x: i32, z: i32, sections: Vec<NbtTag>, data_version: i32) -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", data_version);
    root.put_int("xPos", x);
    root.put_int("zPos", z);
    root.put_int("yPos", -4);
    root.put_string("Status", "minecraft:full".to_string());
    root.put_bool("isLightOn", true);
    root.put_long("InhabitedTime", 42);
    root.put_long("LastUpdate", 1757);
    root.put_list("sections", sections);
    root
}

fn single_slot(version: u8, root: &NbtCompound) -> Vec<(i32, i32, u8, Vec<u8>)> {
    vec![(0, 0, version, compress(version, &nbt_bytes(root)))]
}

fn read_one(
    fixture: &Fixture,
    version: u8,
    root: &NbtCompound,
) -> Result<crate::Chunk, AnvilError> {
    let path = fixture.region(0, 0, &single_slot(version, root));
    Ok(RegionFile::open(&path)?.read_chunk(0, 0)?.unwrap())
}

#[test]
fn every_compression_id_round_trips() {
    let fixture = Fixture::new("compression");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
        ))],
    );
    for version in [GZIP, ZLIB, NONE, LZ4] {
        let chunk = read_one(&fixture, version, &root).unwrap();
        assert_eq!(chunk.sections.len(), 1, "compression {version}");
        assert_eq!(
            chunk.sections[0]
                .block_states
                .as_ref()
                .unwrap()
                .palette
                .name(0),
            "minecraft:stone",
            "compression {version}"
        );
    }
}

#[test]
fn custom_compression_is_a_loud_error() {
    let fixture = Fixture::new("custom");
    let path = fixture.region(0, 0, &[(0, 0, 127, b"whatever".to_vec())]);
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::CustomCompression { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("127"), "{err}");
}

#[test]
fn unknown_compression_names_the_id() {
    let fixture = Fixture::new("unknown_compression");
    let path = fixture.region(0, 0, &[(0, 0, 9, b"whatever".to_vec())]);
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::UnknownCompression { id: 9, .. }),
        "{err}"
    );
}

#[test]
fn external_chunks_come_from_the_mcc_file() {
    let fixture = Fixture::new("external");
    let root = chunk_nbt(
        33,
        2,
        vec![NbtTag::Compound(section(
            1,
            container(vec![NbtTag::Compound(block("minecraft:deepslate"))], None),
        ))],
    );
    fixture.external(33, 2, &compress(ZLIB, &nbt_bytes(&root)));
    let path = fixture.region(1, 0, &[(33, 2, ZLIB | EXTERNAL, Vec::new())]);

    let region = RegionFile::open(&path).unwrap();
    assert_eq!(region.present().collect::<Vec<_>>(), vec![(33, 2)]);
    let chunk = region.read_chunk(33, 2).unwrap().unwrap();
    assert_eq!(chunk.x, 33);
    assert_eq!(
        chunk.sections[0]
            .block_states
            .as_ref()
            .unwrap()
            .palette
            .name(0),
        "minecraft:deepslate"
    );
}

#[test]
fn a_missing_mcc_file_is_a_loud_error() {
    let fixture = Fixture::new("external_missing");
    let path = fixture.region(0, 0, &[(0, 0, ZLIB | EXTERNAL, Vec::new())]);
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::MissingExternal { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("c.0.0.mcc"), "{err}");
}

#[test]
fn an_empty_slot_reads_as_absent() {
    let fixture = Fixture::new("absent");
    let path = fixture.region(0, 0, &single_slot(ZLIB, &chunk_nbt(0, 0, Vec::new())));
    let region = RegionFile::open(&path).unwrap();
    assert_eq!(region.present().count(), 1);
    assert!(region.read_chunk(1, 0).unwrap().is_none());
}

#[test]
fn a_chunk_outside_the_region_is_a_loud_error() {
    let fixture = Fixture::new("wrong_region");
    let path = fixture.region(0, 0, &single_slot(ZLIB, &chunk_nbt(0, 0, Vec::new())));
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(32, 0)
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::WrongRegion { .. }), "{err}");
}

#[test]
fn a_single_value_section_carries_no_data() {
    let fixture = Fixture::new("single_value");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            -4,
            container(vec![NbtTag::Compound(block("minecraft:bedrock"))], None),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    assert_eq!(blocks.palette.len(), 1);
    for (x, y, z) in [(0, 0, 0), (15, 15, 15), (7, 3, 11)] {
        assert_eq!(blocks.name(x, y, z), "minecraft:bedrock");
    }
}

/// A single-entry palette stores no cells, so the container has to answer for
/// them from somewhere other than its data.
struct FakeRegistry;

impl BlockStateLookup for FakeRegistry {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u32> {
        let base = match name {
            "minecraft:air" => 0,
            "minecraft:stone" => 100,
            "minecraft:oak_log" => 200,
            _ => return None,
        };
        Some(
            base + if properties.get("axis") == Some("y") {
                1
            } else {
                0
            },
        )
    }
}

/// The registry sees the name and its properties while both are still borrowed
/// out of the palette's text, so nothing is resolved twice or copied.
#[test]
fn the_palette_resolves_through_a_registry() {
    let fixture = Fixture::new("resolve");
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(1, 0, 0)] = 1;
    entries[BlockStates::index(0, 0, 1)] = 2;
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                    NbtTag::Compound(block_with("minecraft:oak_log", "axis", "y")),
                ],
                Some(pack(&entries, 4)),
            ),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();

    let ids = blocks.resolve_palette(&FakeRegistry).unwrap();
    assert_eq!(ids, vec![0, 100, 201]);
    assert_eq!(ids[blocks.palette_index(0, 0, 0)], 0);
    assert_eq!(ids[blocks.palette_index(1, 0, 0)], 100);
    assert_eq!(ids[blocks.palette_index(0, 0, 1)], 201);
}

#[test]
fn remapping_writes_one_resolved_id_per_cell() {
    let fixture = Fixture::new("remap");
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(1, 0, 0)] = 1;
    entries[BlockStates::index(0, 0, 1)] = 2;
    entries[BlockStates::index(15, 15, 15)] = 2;
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                    NbtTag::Compound(block_with("minecraft:oak_log", "axis", "y")),
                ],
                Some(pack(&entries, 4)),
            ),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();

    let ids = blocks.resolve_palette(&FakeRegistry).unwrap();
    let mut cells = vec![u32::MAX; BlockStates::ENTRY_COUNT];
    blocks.remap_into(&ids, &mut cells);

    assert_eq!(cells[BlockStates::index(0, 0, 0)], 0);
    assert_eq!(cells[BlockStates::index(1, 0, 0)], 100);
    assert_eq!(cells[BlockStates::index(0, 0, 1)], 201);
    assert_eq!(cells[BlockStates::index(15, 15, 15)], 201);
    assert_eq!(cells.iter().filter(|&&c| c == 201).count(), 2);
    assert_eq!(cells.iter().filter(|&&c| c == 100).count(), 1);

    let mut indices = vec![u16::MAX; BlockStates::ENTRY_COUNT];
    blocks.unpack_into(&mut indices);
    assert_eq!(indices, entries);
}

/// A single-entry container stores no cells, so it answers from the palette.
#[test]
fn a_uniform_container_remaps_every_cell() {
    let fixture = Fixture::new("remap_uniform");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    let ids = blocks.resolve_palette(&FakeRegistry).unwrap();
    let mut cells = vec![u32::MAX; BlockStates::ENTRY_COUNT];
    blocks.remap_into(&ids, &mut cells);
    assert!(cells.iter().all(|&c| c == 100));
}

/// The palette entry is read by hand rather than derived, so its rejection of
/// an unknown key needs its own test.
#[test]
fn an_unknown_key_in_a_palette_entry_is_a_loud_error() {
    let fixture = Fixture::new("palette_unknown_key");
    let mut entry = block("minecraft:stone");
    entry.put_int("Weight", 3);
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(vec![NbtTag::Compound(entry)], None),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Nbt(_)), "{err}");
    assert!(err.to_string().contains("Weight"), "{err}");
}

#[test]
fn an_unresolvable_palette_entry_is_a_loud_error() {
    let fixture = Fixture::new("resolve_unknown");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(vec![NbtTag::Compound(block("modded:widget"))], None),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let err = chunk.sections[0]
        .block_states
        .as_ref()
        .unwrap()
        .resolve_palette(&FakeRegistry)
        .unwrap_err();
    assert!(
        matches!(&err, ErrorKind::UnknownPaletteEntry { name } if name == "modded:widget"),
        "{err}"
    );
}

#[test]
fn a_single_value_section_still_reports_every_cell() {
    let fixture = Fixture::new("single_value_entries");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(vec![NbtTag::Compound(block("minecraft:bedrock"))], None),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    let mut cells = vec![0xffffu16; BlockStates::ENTRY_COUNT];
    blocks.unpack_into(&mut cells);
    assert!(cells.iter().all(|&e| e == 0));
}

#[test]
fn a_single_value_section_with_data_is_a_loud_error() {
    let fixture = Fixture::new("single_value_data");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![NbtTag::Compound(block("minecraft:stone"))],
                Some(vec![0i64; 256]),
            ),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::UnexpectedData { .. }),
        "{err}"
    );
}

/// Three entries need two bits, but `Strategy.createForBlockStates` maps bit
/// counts one through four onto the four-bit configuration. Packing at two bits
/// would decode most cells to the wrong palette entry.
#[test]
fn a_three_entry_block_palette_is_stored_at_four_bits() {
    let fixture = Fixture::new("three_entry");
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(0, 0, 0)] = 0;
    entries[BlockStates::index(1, 0, 0)] = 1;
    entries[BlockStates::index(0, 0, 1)] = 2;
    entries[BlockStates::index(15, 15, 15)] = 2;
    entries[BlockStates::index(5, 9, 13)] = 1;

    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                    NbtTag::Compound(block_with("minecraft:oak_log", "axis", "y")),
                ],
                Some(pack(&entries, 4)),
            ),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    assert_eq!(blocks.name(0, 0, 0), "minecraft:air");
    assert_eq!(blocks.name(1, 0, 0), "minecraft:stone");
    assert_eq!(blocks.name(0, 0, 1), "minecraft:oak_log");
    assert_eq!(blocks.name(15, 15, 15), "minecraft:oak_log");
    assert_eq!(blocks.name(5, 9, 13), "minecraft:stone");
    assert_eq!(blocks.name(2, 0, 0), "minecraft:air");
    assert_eq!(blocks.properties(0, 0, 1).get("axis"), Some("y"));
}

/// A palette above 256 entries leaves the linear/hashmap configurations behind
/// for the global one, whose storage width is the raw `ceillog2` of the palette
/// size rather than a table entry.
#[test]
fn a_global_palette_section_is_stored_at_its_own_width() {
    let fixture = Fixture::new("global_palette");
    let palette: Vec<NbtTag> = (0..300)
        .map(|i| NbtTag::Compound(block(&format!("minecraft:block_{i}"))))
        .collect();
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(0, 0, 0)] = 299;
    entries[BlockStates::index(3, 4, 5)] = 256;
    entries[BlockStates::index(15, 15, 15)] = 137;

    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(palette, Some(pack(&entries, 9))),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    assert_eq!(blocks.palette.len(), 300);
    assert_eq!(blocks.name(0, 0, 0), "minecraft:block_299");
    assert_eq!(blocks.name(3, 4, 5), "minecraft:block_256");
    assert_eq!(blocks.name(15, 15, 15), "minecraft:block_137");
    assert_eq!(blocks.name(1, 0, 0), "minecraft:block_0");
}

/// Biomes have no four-bit floor: two entries are stored at one bit.
#[test]
fn a_two_entry_biome_palette_is_stored_at_one_bit() {
    let fixture = Fixture::new("biome_one_bit");
    let mut entries = vec![0u16; Biomes::ENTRY_COUNT];
    entries[Biomes::index(0, 0, 0)] = 1;
    entries[Biomes::index(3, 3, 3)] = 1;
    entries[Biomes::index(1, 2, 3)] = 0;

    let mut s = section(
        0,
        container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
    );
    s.put_component(
        "biomes",
        container(
            vec![
                NbtTag::String("minecraft:plains".to_string()),
                NbtTag::String("minecraft:desert".to_string()),
            ],
            Some(pack(&entries, 1)),
        ),
    );

    let chunk = read_one(&fixture, ZLIB, &chunk_nbt(0, 0, vec![NbtTag::Compound(s)])).unwrap();
    let biomes = chunk.sections[0].biomes.as_ref().unwrap();
    assert_eq!(biomes.name(0, 0, 0), "minecraft:desert");
    assert_eq!(biomes.name(3, 3, 3), "minecraft:desert");
    assert_eq!(biomes.name(1, 2, 3), "minecraft:plains");
    assert_eq!(biomes.name(2, 0, 0), "minecraft:plains");
}

#[test]
fn a_missing_data_array_is_an_error_not_an_empty_section() {
    let fixture = Fixture::new("missing_data");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                ],
                None,
            ),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::MissingData { bits: 4, .. }),
        "{err}"
    );
}

/// 256 is the last size `ceillog2` answers 8 for; reading it as 9 would shift
/// every entry after the first long.
#[test]
fn a_256_entry_palette_is_stored_at_eight_bits() {
    let fixture = Fixture::new("eight_bits");
    let palette: Vec<NbtTag> = (0..256)
        .map(|i| NbtTag::Compound(block(&format!("minecraft:block_{i}"))))
        .collect();
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(0, 0, 0)] = 255;
    entries[BlockStates::index(9, 0, 0)] = 200;
    entries[BlockStates::index(15, 15, 15)] = 1;

    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(palette, Some(pack(&entries, 8))),
        ))],
    );
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    assert_eq!(blocks.name(0, 0, 0), "minecraft:block_255");
    assert_eq!(blocks.name(9, 0, 0), "minecraft:block_200");
    assert_eq!(blocks.name(15, 15, 15), "minecraft:block_1");
    assert_eq!(blocks.name(1, 0, 0), "minecraft:block_0");
}

/// Above 65536 entries an index no longer fits the decoded entry, so a crafted
/// palette would silently truncate every index rather than fail.
#[test]
fn a_palette_too_large_to_index_is_a_loud_error() {
    let fixture = Fixture::new("palette_too_large");
    let palette: Vec<NbtTag> = (0..=u16::MAX as u32 + 1)
        .map(|i| NbtTag::Compound(block(&format!("b{i}"))))
        .collect();
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(0, container(palette, None)))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::PaletteTooLarge { len: 65537, .. }),
        "{err}"
    );
}

#[test]
fn an_empty_palette_is_a_loud_error() {
    let fixture = Fixture::new("empty_palette");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(0, container(Vec::new(), None)))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::EmptyPalette { .. }), "{err}");
}

#[test]
fn a_data_array_longer_than_the_derived_width_is_an_error() {
    let fixture = Fixture::new("data_too_long");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                ],
                Some(vec![0i64; 257]),
            ),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::DataLength {
                found: 257,
                expected: 256,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_data_array_of_the_wrong_length_is_an_error() {
    let fixture = Fixture::new("data_length");
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                ],
                Some(vec![0i64; 255]),
            ),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::DataLength {
                found: 255,
                expected: 256,
                bits: 4,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_palette_index_past_the_palette_is_an_error() {
    let fixture = Fixture::new("palette_index");
    let mut entries = vec![0u16; BlockStates::ENTRY_COUNT];
    entries[BlockStates::index(2, 0, 0)] = 5;
    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(
                vec![
                    NbtTag::Compound(block("minecraft:air")),
                    NbtTag::Compound(block("minecraft:stone")),
                ],
                Some(pack(&entries, 4)),
            ),
        ))],
    );
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::PaletteIndex {
                index: 5,
                len: 2,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_section_outside_any_height_range_keeps_its_light() {
    let fixture = Fixture::new("out_of_range");
    let mut light = vec![0u8; LIGHT_BYTES];
    light[0] = 0x0f;
    let mut s = section(
        120,
        container(vec![NbtTag::Compound(block("minecraft:air"))], None),
    );
    s.put("SkyLight", NbtTag::ByteArray(light.into_boxed_slice()));

    let chunk = read_one(&fixture, ZLIB, &chunk_nbt(0, 0, vec![NbtTag::Compound(s)])).unwrap();
    assert_eq!(chunk.sections[0].y, 120);
    let sky = chunk.sections[0].sky_light.as_ref().unwrap();
    assert_eq!(sky.get(0, 0, 0), 15);
    assert_eq!(sky.get(1, 0, 0), 0);
}

#[test]
fn light_arrays_must_be_2048_bytes() {
    let fixture = Fixture::new("light_length");
    let mut s = section(
        0,
        container(vec![NbtTag::Compound(block("minecraft:air"))], None),
    );
    s.put(
        "BlockLight",
        NbtTag::ByteArray(vec![0u8; 100].into_boxed_slice()),
    );
    let err = read_one(&fixture, ZLIB, &chunk_nbt(0, 0, vec![NbtTag::Compound(s)])).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::LightLength { found: 100, .. }),
        "{err}"
    );
}

#[test]
fn a_stale_data_version_is_a_loud_error() {
    let fixture = Fixture::new("data_version");
    let root = chunk_nbt_versioned(0, 0, Vec::new(), 4903);
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::DataVersion {
                found: 4903,
                expected: 5017
            }
        ),
        "{err}"
    );
    assert!(
        err.to_string()
            .ends_with("DataVersion 4903, expected 5015 to 5017"),
        "{err}"
    );
}

/// Vanilla reads the fields it names off the compound and ignores the rest, and
/// a real save carries keys other tools wrote: a world opened once under
/// Starlight puts `starlight.skylight_state` on every section. The `DataVersion`
/// gate is what catches format drift; refusing a stranger's key on top of it
/// only makes modded worlds unreadable.
#[test]
fn a_key_this_decoder_does_not_know_is_ignored() {
    let fixture = Fixture::new("unknown_key");
    let mut section = section(
        0,
        container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
    );
    section.put_int("starlight.skylight_state", 3);
    let mut root = chunk_nbt(0, 0, vec![NbtTag::Compound(section)]);
    root.put_int("SomeFutureField", 1);
    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    assert_eq!(chunk.sections.len(), 1);
}

/// `DataVersion` arrived in 15w32a, so a chunk older than that has no version
/// to disagree with and must say so rather than leaking a missing-field message.
#[test]
fn a_chunk_older_than_the_version_tag_says_so() {
    let fixture = Fixture::new("no_version");
    let mut root = NbtCompound::new();
    root.put_component("Level", NbtCompound::new());
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::MissingDataVersion { expected: 5017 }),
        "{err}"
    );
}

/// An older chunk trips over whatever field this layout gained or lost long
/// before anything looks at its version, so the version has to be re-read to
/// produce the error that actually explains the failure.
#[test]
fn an_older_layout_reports_its_version_not_its_first_odd_field() {
    let fixture = Fixture::new("old_layout");
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", 1343);
    root.put_component("Level", NbtCompound::new());
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::DataVersion {
                found: 1343,
                expected: 5017
            }
        ),
        "{err}"
    );
}

#[test]
fn the_vanilla_shaped_extra_fields_are_accepted() {
    let fixture = Fixture::new("vanilla_extras");
    let mut root = chunk_nbt(0, 0, Vec::new());
    root.put_list("block_ticks", Vec::new());
    root.put_list("fluid_ticks", Vec::new());
    root.put_list("PostProcessing", Vec::new());
    root.put_component("structures", NbtCompound::new());
    root.put_list("entities", Vec::new());
    root.put_component("UpgradeData", NbtCompound::new());
    root.put_component("blending_data", NbtCompound::new());
    root.put_component("below_zero_retrogen", NbtCompound::new());
    let mut heightmaps = NbtCompound::new();
    heightmaps.put("MOTION_BLOCKING", NbtTag::LongArray(vec![7i64; 37]));
    root.put_component("Heightmaps", heightmaps);
    let mut block_entity = NbtCompound::new();
    block_entity.put_string("id", "minecraft:chest".to_string());
    root.put_list("block_entities", vec![NbtTag::Compound(block_entity)]);

    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    assert_eq!(chunk.min_section_y, -4);
    assert_eq!(chunk.status, "minecraft:full");
    assert!(chunk.is_light_on);
    assert_eq!(chunk.inhabited_time, 42);
    assert_eq!(chunk.heightmaps["MOTION_BLOCKING"].len(), 37);
    assert_eq!(chunk.block_entities.len(), 1);
    assert_eq!(
        chunk.block_entities[0].get_string("id"),
        Some("minecraft:chest")
    );
}

#[test]
fn a_bad_file_name_is_rejected_before_any_bytes_are_read() {
    let err = RegionFile::open(Path::new("/nowhere/region.mca")).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::FileName), "{err}");
}

#[test]
fn a_sector_pointing_into_the_header_is_an_error() {
    let fixture = Fixture::new("sector_in_header");
    let path = fixture.region(0, 0, &single_slot(NONE, &chunk_nbt(0, 0, Vec::new())));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[..4].copy_from_slice(&(1i32 << 8 | 1).to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::SectorInHeader { sector: 1, .. }),
        "{err}"
    );
}

#[test]
fn a_sector_past_the_end_of_the_file_is_an_error() {
    let fixture = Fixture::new("sector_out_of_bounds");
    let path = fixture.region(0, 0, &single_slot(NONE, &chunk_nbt(0, 0, Vec::new())));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[..4].copy_from_slice(&(900i32 << 8 | 1).to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::SectorOutOfBounds { sector: 900, .. }),
        "{err}"
    );
}

#[test]
fn a_payload_longer_than_its_sectors_is_an_error() {
    let fixture = Fixture::new("payload_length");
    let path = fixture.region(0, 0, &single_slot(NONE, &chunk_nbt(0, 0, Vec::new())));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[2 * SECTOR_BYTES..2 * SECTOR_BYTES + 4].copy_from_slice(&99_999i32.to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(0, 0)
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::PayloadLength { length: 99_999, .. }),
        "{err}"
    );
}

#[test]
fn a_truncated_header_is_rejected_at_open() {
    let fixture = Fixture::new("short_header");
    let path = fixture.dir.join("r.0.0.mca");
    std::fs::write(&path, vec![0u8; 100]).unwrap();
    let err = RegionFile::open(&path).unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::ShortHeader { len: 100 }),
        "{err}"
    );
}

#[test]
fn the_index_formulas_match_the_reference() {
    assert_eq!(BlockStates::index(0, 0, 0), 0);
    assert_eq!(BlockStates::index(1, 0, 0), 1);
    assert_eq!(BlockStates::index(0, 0, 1), 16);
    assert_eq!(BlockStates::index(0, 1, 0), 256);
    assert_eq!(BlockStates::index(15, 15, 15), 4095);
    assert_eq!(BlockStates::ENTRY_COUNT, 4096);

    assert_eq!(Biomes::index(1, 0, 0), 1);
    assert_eq!(Biomes::index(0, 0, 1), 4);
    assert_eq!(Biomes::index(0, 1, 0), 16);
    assert_eq!(Biomes::index(3, 3, 3), 63);
    assert_eq!(Biomes::ENTRY_COUNT, 64);
}

#[test]
fn timestamps_come_from_the_second_header_sector() {
    let fixture = Fixture::new("timestamp");
    let path = fixture.region(0, 0, &single_slot(ZLIB, &chunk_nbt(0, 0, Vec::new())));
    assert_eq!(
        RegionFile::open(&path).unwrap().timestamp(0, 0),
        1_700_000_000
    );
}
