use std::io::Write;
use std::path::{Path, PathBuf};

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use mcrs_minecraft_core::ColumnPos;

use crate::chunk::LIGHT_BYTES;
use crate::region::SECTOR_BYTES;
use mcrs_minecraft_chunk::PalettedContainer::Homogeneous;
use mcrs_minecraft_chunk::section::{Biomes, Blocks, NoiseBiomes};
use mcrs_minecraft_chunk::{SectionKind, VoxelId};
use std::cell::Cell;

use crate::{AnvilError, ChunkStatus, ErrorKind, PaletteLookup, Properties, RegionFile};
use mcrs_minecraft_core::VERSION;

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
    chunk_nbt_versioned(x, z, sections, VERSION.world_version)
}

fn chunk_nbt_versioned(x: i32, z: i32, sections: Vec<NbtTag>, data_version: i32) -> NbtCompound {
    let mut root = chunk_nbt_without_status(x, z, sections, data_version);
    root.put_string("status", "minecraft:full".to_string());
    root
}

fn chunk_nbt_without_status(
    x: i32,
    z: i32,
    sections: Vec<NbtTag>,
    data_version: i32,
) -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", data_version);
    root.put_int("xPos", x);
    root.put_int("zPos", z);
    root.put_int("yPos", -4);
    root.put_bool("isLightOn", true);
    root.put_long("InhabitedTime", 42);
    root.put_long("LastUpdate", 1757);
    root.put_list("sections", sections);
    root
}

fn retrogen_record(target_status: &str, rerun: &[&str]) -> NbtCompound {
    let mut record = NbtCompound::new();
    record.put_string("target_status", target_status.to_string());
    record.put_list(
        "statuses_to_rerun",
        rerun
            .iter()
            .map(|status| NbtTag::String(status.to_string()))
            .collect(),
    );
    record
}

fn chunk_with_retrogen(status: &str, record: NbtCompound) -> NbtCompound {
    let mut root = chunk_nbt_without_status(0, 0, Vec::new(), VERSION.world_version);
    root.put_string("status", status.to_string());
    root.put_component("retrogen", record);
    root
}

fn quart_biomes(entries: &[u16]) -> NbtCompound {
    container(
        vec![
            NbtTag::String("minecraft:plains".to_string()),
            NbtTag::String("minecraft:desert".to_string()),
        ],
        Some(pack(entries, 1)),
    )
}

fn section_with_noise_biomes(y: i8, noise_biomes: NbtCompound) -> NbtCompound {
    let mut s = section(
        y,
        container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
    );
    s.put_component("noise_biomes", noise_biomes);
    s
}

fn chunk_at(status: &str, sections: Vec<NbtTag>) -> NbtCompound {
    let mut root = chunk_nbt_without_status(0, 0, sections, VERSION.world_version);
    root.put_string("status", status.to_string());
    root
}

fn single_slot(version: u8, root: &NbtCompound) -> Vec<(i32, i32, u8, Vec<u8>)> {
    vec![(0, 0, version, compress(version, &nbt_bytes(root)))]
}

/// Stands in for the block and biome registries with ids a test can name.
#[derive(Default)]
struct TestRegistry {
    asked: Cell<usize>,
}

const AIR: VoxelId = VoxelId(0);
const STONE: VoxelId = VoxelId(100);
const OAK_LOG: VoxelId = VoxelId(200);
const OAK_LOG_Y: VoxelId = VoxelId(201);
const BEDROCK: VoxelId = VoxelId(300);
const DEEPSLATE: VoxelId = VoxelId(301);
const PLAINS: u8 = 1;
const DESERT: u8 = 2;

fn numbered(index: u16) -> VoxelId {
    VoxelId(1000 + index)
}

impl PaletteLookup<VoxelId> for TestRegistry {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        self.asked.set(self.asked.get() + 1);
        match name {
            "minecraft:air" => Some(AIR),
            "minecraft:stone" => Some(STONE),
            "minecraft:oak_log" if properties.get("axis") == Some("y") => Some(OAK_LOG_Y),
            "minecraft:oak_log" => Some(OAK_LOG),
            "minecraft:bedrock" => Some(BEDROCK),
            "minecraft:deepslate" => Some(DEEPSLATE),
            _ => name
                .strip_prefix("minecraft:block_")?
                .parse()
                .ok()
                .map(numbered),
        }
    }
}

impl PaletteLookup<u8> for TestRegistry {
    fn resolve(&self, name: &str, _properties: Properties<'_>) -> Option<u8> {
        match name {
            "minecraft:plains" => Some(PLAINS),
            "minecraft:desert" => Some(DESERT),
            _ => None,
        }
    }
}

fn read_one(
    fixture: &Fixture,
    version: u8,
    root: &NbtCompound,
) -> Result<crate::Chunk, AnvilError> {
    read_with(fixture, version, root, &TestRegistry::default())
}

fn read_with(
    fixture: &Fixture,
    version: u8,
    root: &NbtCompound,
    registry: &TestRegistry,
) -> Result<crate::Chunk, AnvilError> {
    let path = fixture.region(0, 0, &single_slot(version, root));
    Ok(RegionFile::open(&path)?
        .read_chunk(ColumnPos::new(0, 0), registry, registry)?
        .unwrap())
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
            chunk.sections[0].block_states,
            Some(Homogeneous(STONE)),
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
        .read_chunk(
            ColumnPos::new(0, 0),
            &TestRegistry::default(),
            &TestRegistry::default(),
        )
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::CustomCompression { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("127"), "{err}");
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
    assert_eq!(
        region.present().collect::<Vec<_>>(),
        vec![ColumnPos::new(33, 2)]
    );
    let chunk = region
        .read_chunk(
            ColumnPos::new(33, 2),
            &TestRegistry::default(),
            &TestRegistry::default(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(chunk.pos.x, 33);
    assert_eq!(chunk.sections[0].block_states, Some(Homogeneous(DEEPSLATE)));
}

#[test]
fn a_missing_mcc_file_is_a_loud_error() {
    let fixture = Fixture::new("external_missing");
    let path = fixture.region(0, 0, &[(0, 0, ZLIB | EXTERNAL, Vec::new())]);
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(
            ColumnPos::new(0, 0),
            &TestRegistry::default(),
            &TestRegistry::default(),
        )
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
    assert!(
        region
            .read_chunk(
                ColumnPos::new(1, 0),
                &TestRegistry::default(),
                &TestRegistry::default()
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_chunk_outside_the_region_is_a_loud_error() {
    let fixture = Fixture::new("wrong_region");
    let path = fixture.region(0, 0, &single_slot(ZLIB, &chunk_nbt(0, 0, Vec::new())));
    let err = RegionFile::open(&path)
        .unwrap()
        .read_chunk(
            ColumnPos::new(32, 0),
            &TestRegistry::default(),
            &TestRegistry::default(),
        )
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::WrongRegion { .. }), "{err}");
}

/// Three entries need two bits, but `Strategy.createForBlockStates` maps bit
/// counts one through four onto the four-bit configuration. Packing at two bits
/// would decode most cells to the wrong palette entry.
#[test]
fn every_cell_holds_its_resolved_id() {
    let fixture = Fixture::new("remap");
    let mut entries = vec![0u16; Blocks::ENTRY_COUNT];
    entries[Blocks::index(1, 0, 0)] = 1;
    entries[Blocks::index(0, 0, 1)] = 2;
    entries[Blocks::index(15, 15, 15)] = 2;
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

    let ids = [AIR, STONE, OAK_LOG_Y];
    let mut index = 0;
    blocks.for_each(|cell| {
        assert_eq!(cell, ids[entries[index] as usize], "cell {index}");
        index += 1;
    });
    assert_eq!(index, Blocks::ENTRY_COUNT);
}

/// A single-entry container stores no cells, so it answers from the palette.
#[test]
fn a_uniform_container_holds_its_value_in_every_cell() {
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
    let mut cells = 0;
    blocks.for_each(|cell| {
        assert_eq!(cell, STONE);
        cells += 1;
    });
    assert_eq!(cells, Blocks::ENTRY_COUNT);
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
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(
        matches!(&err.kind, ErrorKind::UnknownPaletteEntry { name } if name == "modded:widget"),
        "{err}"
    );
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

/// A palette above 256 entries leaves the linear/hashmap configurations behind
/// for the global one, whose storage width is the raw `ceillog2` of the palette
/// size rather than a table entry.
#[test]
fn a_global_palette_section_is_stored_at_its_own_width() {
    let fixture = Fixture::new("global_palette");
    let palette: Vec<NbtTag> = (0..300)
        .map(|i| NbtTag::Compound(block(&format!("minecraft:block_{i}"))))
        .collect();
    let mut entries = vec![0u16; Blocks::ENTRY_COUNT];
    entries[Blocks::index(0, 0, 0)] = 299;
    entries[Blocks::index(3, 4, 5)] = 256;
    entries[Blocks::index(15, 15, 15)] = 137;

    let root = chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(section(
            0,
            container(palette, Some(pack(&entries, 9))),
        ))],
    );
    let registry = TestRegistry::default();
    let chunk = read_with(&fixture, ZLIB, &root, &registry).unwrap();
    assert_eq!(registry.asked.get(), 300, "every palette entry is resolved");
    let blocks = chunk.sections[0].block_states.as_ref().unwrap();
    assert_eq!(blocks.get(0, 0, 0), numbered(299));
    assert_eq!(blocks.get(3, 4, 5), numbered(256));
    assert_eq!(blocks.get(15, 15, 15), numbered(137));
    assert_eq!(blocks.get(1, 0, 0), numbered(0));
}

#[test]
fn a_biome_container_of_the_old_entry_count_is_a_load_error() {
    let fixture = Fixture::new("biome_old_entry_count");
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
            Some(vec![0i64; 1]),
        ),
    );

    let err = read_one(&fixture, ZLIB, &chunk_nbt(0, 0, vec![NbtTag::Compound(s)])).unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::DataLength {
                found: 1,
                expected: 64,
                bits: 1,
                ..
            }
        ),
        "{err}"
    );
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
    let mut entries = vec![0u16; Blocks::ENTRY_COUNT];
    entries[Blocks::index(0, 0, 0)] = 255;
    entries[Blocks::index(9, 0, 0)] = 200;
    entries[Blocks::index(15, 15, 15)] = 1;

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
    assert_eq!(blocks.get(0, 0, 0), numbered(255));
    assert_eq!(blocks.get(9, 0, 0), numbered(200));
    assert_eq!(blocks.get(15, 15, 15), numbered(1));
    assert_eq!(blocks.get(1, 0, 0), numbered(0));
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
    let mut entries = vec![0u16; Blocks::ENTRY_COUNT];
    entries[Blocks::index(2, 0, 0)] = 5;
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

/// A section whose `BlockLight` holds `value`, written ahead of a good
/// `SkyLight`, the block states and a second section, so a value read at the
/// wrong width shows in what follows it.
fn chunk_with_block_light(value: NbtTag) -> NbtCompound {
    let mut sky = vec![0u8; LIGHT_BYTES];
    sky[0] = 0x0f;
    let mut first = NbtCompound::new();
    first.put_byte("Y", 0);
    first.put("BlockLight", value);
    first.put("SkyLight", NbtTag::ByteArray(sky.into_boxed_slice()));
    first.put_component(
        "block_states",
        container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
    );
    let second = section(
        1,
        container(vec![NbtTag::Compound(block("minecraft:air"))], None),
    );
    chunk_nbt(
        0,
        0,
        vec![NbtTag::Compound(first), NbtTag::Compound(second)],
    )
}

fn assert_block_light_is_absent(fixture: &Fixture, stored: Vec<(&str, NbtTag)>) {
    for (shape, value) in stored {
        let chunk = read_one(fixture, ZLIB, &chunk_with_block_light(value))
            .unwrap_or_else(|err| panic!("{shape}: {err}"));
        assert_eq!(chunk.sections.len(), 2, "{shape}");
        let first = &chunk.sections[0];
        assert_eq!(first.block_light, None, "{shape}");
        let sky = first
            .sky_light
            .as_ref()
            .unwrap_or_else(|| panic!("{shape}"));
        assert_eq!(sky.get(0, 0, 0), 15, "{shape}");
        assert_eq!(first.block_states, Some(Homogeneous(STONE)), "{shape}");
        assert_eq!(chunk.sections[1].y, 1, "{shape}");
    }
}

#[test]
fn light_that_is_no_collection_reads_as_absent() {
    let fixture = Fixture::new("light_no_collection");
    assert_block_light_is_absent(
        &fixture,
        vec![
            ("a string", NbtTag::String("bright".to_string())),
            ("an int", NbtTag::Int(7)),
            ("a compound", NbtTag::Compound(NbtCompound::new())),
        ],
    );
}

fn air_and_stone() -> Vec<NbtTag> {
    vec![
        NbtTag::Compound(block("minecraft:air")),
        NbtTag::Compound(block("minecraft:stone")),
    ]
}

fn container_holding(palette: Vec<NbtTag>, data: NbtTag) -> NbtCompound {
    let mut c = container(palette, None);
    c.put("data", data);
    c
}

/// Two entries pack sixteen cells into a word, so a word of one puts stone in
/// the first cell of its row and air in the rest.
#[test]
fn block_data_in_another_array_or_a_list_reads_one_word_per_element() {
    let fixture = Fixture::new("block_data_by_element");
    let stored = [
        ("a long array", NbtTag::LongArray(vec![1; 256])),
        ("an int array", NbtTag::IntArray(vec![1; 256])),
        (
            "a byte array",
            NbtTag::ByteArray(vec![1u8; 256].into_boxed_slice()),
        ),
        ("a list of ints", NbtTag::List(vec![NbtTag::Int(1); 256])),
        (
            "a list of doubles",
            NbtTag::List(vec![NbtTag::Double(1.9); 256]),
        ),
    ];
    for (shape, value) in stored {
        let states = container_holding(air_and_stone(), value);
        let root = chunk_nbt(0, 0, vec![NbtTag::Compound(section(0, states))]);
        let chunk = read_one(&fixture, ZLIB, &root).unwrap_or_else(|err| panic!("{shape}: {err}"));
        let blocks = chunk.sections[0].block_states.as_ref().unwrap();
        for index in 0..Blocks::ENTRY_COUNT {
            let (x, y, z) = (index & 15, index >> 8, index >> 4 & 15);
            let expected = if x == 0 { STONE } else { AIR };
            assert_eq!(blocks.get(x, y, z), expected, "{shape} at {x},{y},{z}");
        }
    }
}

/// One bit per cell packs 64 biome cells into a word, so the sign an element
/// widens with and the way a float narrows both show in the cells: -2 clears a
/// word's first cell and sets every other.
#[test]
fn biome_data_widens_each_element_as_a_signed_number() {
    let fixture = Fixture::new("biome_data_by_element");
    let stored = [
        ("a long array", NbtTag::LongArray(vec![-2; 64])),
        ("an int array", NbtTag::IntArray(vec![-2; 64])),
        ("a byte array", NbtTag::ByteArray(Box::new([0xfe; 64]))),
        (
            "a list of shorts",
            NbtTag::List(vec![NbtTag::Short(-2); 64]),
        ),
        (
            "a list of doubles",
            NbtTag::List(vec![NbtTag::Double(-2.7); 64]),
        ),
    ];
    for (shape, value) in stored {
        let mut s = section(
            0,
            container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
        );
        let palette = vec![
            NbtTag::String("minecraft:plains".to_string()),
            NbtTag::String("minecraft:desert".to_string()),
        ];
        s.put_component("biomes", container_holding(palette, value));
        let chunk = read_one(&fixture, ZLIB, &chunk_nbt(0, 0, vec![NbtTag::Compound(s)]))
            .unwrap_or_else(|err| panic!("{shape}: {err}"));
        let biomes = chunk.sections[0].biomes.as_ref().unwrap();
        for index in 0..Biomes::ENTRY_COUNT {
            let (x, y, z) = (index & 15, index >> 8, index >> 4 & 15);
            let expected = if index % 64 == 0 { PLAINS } else { DESERT };
            assert_eq!(biomes.get(x, y, z), expected, "{shape} at {x},{y},{z}");
        }
    }
}

#[test]
fn packed_data_that_is_no_list_of_numbers_reads_as_absent() {
    let fixture = Fixture::new("packed_data_malformed");
    let malformed = [
        NbtTag::String("not a long stream".to_string()),
        NbtTag::Int(7),
        NbtTag::Compound(NbtCompound::new()),
        NbtTag::List(vec![NbtTag::String("x".to_string())]),
        NbtTag::List(vec![NbtTag::Long(1), NbtTag::String("x".to_string())]),
    ];
    for value in malformed {
        let needed = container_holding(air_and_stone(), value.clone());
        let root = chunk_nbt(0, 0, vec![NbtTag::Compound(section(0, needed))]);
        let err = read_one(&fixture, ZLIB, &root).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::MissingData { bits: 4, .. }),
            "{value:?}: {err}"
        );

        let stone = vec![NbtTag::Compound(block("minecraft:stone"))];
        let unneeded = container_holding(stone, value.clone());
        let root = chunk_nbt(0, 0, vec![NbtTag::Compound(section(1, unneeded))]);
        let chunk =
            read_one(&fixture, ZLIB, &root).unwrap_or_else(|err| panic!("{value:?}: {err}"));
        assert_eq!(chunk.sections[0].y, 1, "{value:?}");
        assert_eq!(
            chunk.sections[0].block_states,
            Some(Homogeneous(STONE)),
            "{value:?}"
        );
    }
}

#[test]
fn a_chunk_one_data_version_off_either_way_is_refused_and_the_current_one_loads() {
    let fixture = Fixture::new("data_version_adjacent");
    let current = VERSION.world_version;
    read_one(
        &fixture,
        ZLIB,
        &chunk_nbt_versioned(0, 0, Vec::new(), current),
    )
    .unwrap();
    for found in [current - 1, current + 1] {
        let err = read_one(
            &fixture,
            ZLIB,
            &chunk_nbt_versioned(0, 0, Vec::new(), found),
        )
        .unwrap_err();
        assert!(
            matches!(
                err.kind,
                ErrorKind::DataVersion { found: f, expected } if f == found && expected == current
            ),
            "{err}"
        );
        assert!(
            err.to_string()
                .ends_with(&format!("DataVersion {found}, expected {current}")),
            "{err}"
        );
    }
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
        matches!(
            err.kind,
            ErrorKind::MissingDataVersion { expected } if expected == VERSION.world_version
        ),
        "{err}"
    );
    assert!(
        err.to_string().ends_with(&format!(
            "no DataVersion, expected {}",
            VERSION.world_version
        )),
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
                expected
            } if expected == VERSION.world_version
        ),
        "{err}"
    );
}

#[test]
fn a_chunk_with_an_unregistered_status_is_a_load_error() {
    let fixture = Fixture::new("unregistered_status");
    let mut root = chunk_nbt(0, 0, Vec::new());
    root.put_string("status", "minecraft:not_a_status".to_string());
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Nbt(_)), "{err}");
}

#[test]
fn a_current_chunk_with_the_old_status_key_is_refused_for_the_missing_field() {
    let fixture = Fixture::new("old_status_key");
    let mut root = chunk_nbt_without_status(0, 0, Vec::new(), VERSION.world_version);
    root.put_string("Status", "minecraft:full".to_string());
    let err = read_one(&fixture, ZLIB, &root).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Nbt(_)), "{err}");
    assert!(err.to_string().contains("status"), "{err}");
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
    root.put_component(
        "retrogen",
        retrogen_record("minecraft:full", &["minecraft:features"]),
    );
    let mut heightmaps = NbtCompound::new();
    heightmaps.put("MOTION_BLOCKING", NbtTag::LongArray(vec![7i64; 37]));
    root.put_component("Heightmaps", heightmaps);
    let mut block_entity = NbtCompound::new();
    block_entity.put_string("id", "minecraft:chest".to_string());
    root.put_list("block_entities", vec![NbtTag::Compound(block_entity)]);

    let chunk = read_one(&fixture, ZLIB, &root).unwrap();
    assert_eq!(chunk.min_section_y, -4);
    assert_eq!(chunk.status, ChunkStatus::Full);
    assert!(chunk.is_light_on);
    assert_eq!(chunk.inhabited_time, 42);
    assert_eq!(chunk.heightmaps["MOTION_BLOCKING"].len(), 37);
    assert_eq!(chunk.block_entities.len(), 1);
    assert_eq!(
        chunk.block_entities[0].get_string("id"),
        Some("minecraft:chest")
    );
    assert_eq!(
        chunk.retrogen,
        Some(crate::RetroGen {
            target_status: ChunkStatus::Full,
            statuses_to_rerun: vec![ChunkStatus::Features],
            has_below_zero_retrogen: false,
            missing_bedrock: Vec::new(),
        })
    );
}

fn retrogen_error(name: &str, record: NbtCompound) -> String {
    let fixture = Fixture::new(name);
    let err = read_one(
        &fixture,
        ZLIB,
        &chunk_with_retrogen("minecraft:terrain", record),
    )
    .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Nbt(_)), "{err}");
    err.to_string()
}

#[test]
fn an_unregistered_status_in_retrogen_is_a_load_error() {
    retrogen_error(
        "retrogen_unregistered_target",
        retrogen_record("minecraft:not_a_status", &[]),
    );
    retrogen_error(
        "retrogen_unregistered_rerun",
        retrogen_record("minecraft:full", &["minecraft:not_a_status"]),
    );
}

#[test]
fn a_malformed_missing_bedrock_reads_as_absent() {
    let fixture = Fixture::new("malformed_bedrock");
    let malformed = [
        NbtTag::String("not a bit set".to_string()),
        NbtTag::Int(7),
        NbtTag::Compound(NbtCompound::new()),
        NbtTag::List(vec![NbtTag::String("x".to_string())]),
        NbtTag::List(vec![NbtTag::Long(1), NbtTag::String("x".to_string())]),
    ];
    for value in malformed {
        let mut record = retrogen_record("minecraft:full", &["minecraft:biomes"]);
        record.put_bool("has_below_zero_retrogen", true);
        record.put("missing_bedrock", value.clone());
        let chunk = read_one(
            &fixture,
            ZLIB,
            &chunk_with_retrogen("minecraft:terrain", record),
        )
        .unwrap_or_else(|err| panic!("{value:?}: {err}"));
        let retrogen = chunk.retrogen.unwrap();
        assert!(retrogen.missing_bedrock.is_empty(), "{value:?}");
        assert!(retrogen.has_below_zero_retrogen, "{value:?}");
        assert_eq!(retrogen.statuses_to_rerun, vec![ChunkStatus::Biomes]);
    }
}

#[test]
fn missing_bedrock_reads_an_array_or_a_list_of_numbers_element_by_element() {
    let fixture = Fixture::new("bedrock_by_element");
    let stored = [
        (NbtTag::IntArray(vec![1, 2]), vec![1, 2]),
        (NbtTag::IntArray(vec![1, 2, 3]), vec![1, 2, 3]),
        (NbtTag::IntArray(vec![-1, 4]), vec![-1, 4]),
        (NbtTag::ByteArray(Box::new([1; 8])), vec![1; 8]),
        (NbtTag::ByteArray(Box::new([0xff, 2])), vec![-1, 2]),
        (NbtTag::LongArray(vec![i64::MIN, 9]), vec![i64::MIN, 9]),
        (
            NbtTag::List(vec![NbtTag::Int(1), NbtTag::Int(2)]),
            vec![1, 2],
        ),
        (
            NbtTag::List(vec![NbtTag::Long(6), NbtTag::Short(-7)]),
            vec![6, -7],
        ),
        (NbtTag::List(vec![NbtTag::Double(5.0)]), vec![5]),
        (
            NbtTag::List(vec![NbtTag::Double(-2.7), NbtTag::Float(3.9)]),
            vec![-2, 3],
        ),
    ];
    for (value, words) in stored {
        let mut record = retrogen_record("minecraft:full", &["minecraft:biomes"]);
        record.put("missing_bedrock", value.clone());
        let chunk = read_one(
            &fixture,
            ZLIB,
            &chunk_with_retrogen("minecraft:terrain", record),
        )
        .unwrap_or_else(|err| panic!("{value:?}: {err}"));
        assert_eq!(chunk.retrogen.unwrap().missing_bedrock, words, "{value:?}");
    }
}

#[test]
fn a_bad_file_name_is_rejected_before_any_bytes_are_read() {
    let err = RegionFile::open(Path::new("/nowhere/region.mca")).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::FileName), "{err}");
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
        .read_chunk(
            ColumnPos::new(0, 0),
            &TestRegistry::default(),
            &TestRegistry::default(),
        )
        .unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::SectorOutOfBounds { sector: 900, .. }),
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
    assert_eq!(Blocks::index(0, 0, 0), 0);
    assert_eq!(Blocks::index(1, 0, 0), 1);
    assert_eq!(Blocks::index(0, 0, 1), 16);
    assert_eq!(Blocks::index(0, 1, 0), 256);
    assert_eq!(Blocks::index(15, 15, 15), 4095);
    assert_eq!(Blocks::ENTRY_COUNT, 4096);

    assert_eq!(Biomes::index(1, 0, 0), 1);
    assert_eq!(Biomes::index(0, 0, 1), 16);
    assert_eq!(Biomes::index(0, 1, 0), 256);
    assert_eq!(Biomes::index(15, 15, 15), 4095);
    assert_eq!(Biomes::ENTRY_COUNT, 4096);

    assert_eq!(NoiseBiomes::index(1, 0, 0), 1);
    assert_eq!(NoiseBiomes::index(0, 0, 1), 4);
    assert_eq!(NoiseBiomes::index(0, 1, 0), 16);
    assert_eq!(NoiseBiomes::index(3, 3, 3), 63);
    assert_eq!(NoiseBiomes::ENTRY_COUNT, 64);
}

#[test]
fn a_section_reads_its_noise_biomes_at_the_quart_size() {
    let fixture = Fixture::new("noise_biomes_read");
    let mut entries = vec![0u16; NoiseBiomes::ENTRY_COUNT];
    entries[NoiseBiomes::index(0, 0, 0)] = 1;
    entries[NoiseBiomes::index(3, 3, 3)] = 1;
    entries[NoiseBiomes::index(1, 2, 3)] = 1;
    let section = section_with_noise_biomes(0, quart_biomes(&entries));
    let chunk = read_one(
        &fixture,
        ZLIB,
        &chunk_at("minecraft:terrain", vec![NbtTag::Compound(section)]),
    )
    .unwrap();
    let noise = chunk.sections[0].noise_biomes.as_ref().unwrap();
    assert_eq!(noise.get(0, 0, 0), DESERT);
    assert_eq!(noise.get(3, 3, 3), DESERT);
    assert_eq!(noise.get(1, 2, 3), DESERT);
    assert_eq!(noise.get(2, 0, 0), PLAINS);
    assert_eq!(noise.get(1, 2, 2), PLAINS);
    assert!(chunk.sections[0].biomes.is_none());
}

#[test]
fn a_full_chunk_keeps_no_noise_biomes() {
    let fixture = Fixture::new("noise_biomes_full");
    let section = section_with_noise_biomes(0, quart_biomes(&[0; NoiseBiomes::ENTRY_COUNT]));
    let chunk = read_one(
        &fixture,
        ZLIB,
        &chunk_at("minecraft:full", vec![NbtTag::Compound(section)]),
    )
    .unwrap();
    assert!(chunk.sections[0].noise_biomes.is_none());
}

#[test]
fn a_malformed_noise_biomes_container_is_a_load_error() {
    let fixture = Fixture::new("noise_biomes_malformed");
    let short = container(
        vec![
            NbtTag::String("minecraft:plains".to_string()),
            NbtTag::String("minecraft:desert".to_string()),
        ],
        Some(vec![0, 0]),
    );
    for status in ["minecraft:terrain", "minecraft:full"] {
        let section = section_with_noise_biomes(0, short.clone());
        let err = read_one(
            &fixture,
            ZLIB,
            &chunk_at(status, vec![NbtTag::Compound(section)]),
        )
        .unwrap_err();
        assert!(
            matches!(
                err.kind,
                ErrorKind::DataLength {
                    field: "noise_biomes",
                    found: 2,
                    expected: 1,
                    bits: 1,
                    ..
                }
            ),
            "{status}: {err}"
        );
    }
}

pub(crate) mod write {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::Path;

    use mcrs_minecraft_chunk::PalettedContainer;
    use mcrs_minecraft_core::BlockPos;
    use mcrs_minecraft_nbt::deserializer::NbtReadHelper;

    use super::*;
    use crate::fixture::{self, region_chunks};
    use crate::{Chunk, PaletteId, PaletteNames, write_chunk};

    pub(crate) struct Named {
        pub(crate) chunk: Chunk,
        pub(crate) blocks: PaletteNames<VoxelId>,
        pub(crate) biomes: PaletteNames<u8>,
    }

    fn read_named(region: &RegionFile, pos: ColumnPos) -> Named {
        let (blocks, biomes) = (PaletteNames::new(), PaletteNames::new());
        let chunk = region.read_chunk(pos, &blocks, &biomes).unwrap().unwrap();
        Named {
            chunk,
            blocks,
            biomes,
        }
    }

    fn fixture_region(fixture: &Fixture, chunks: usize) -> RegionFile {
        let slots: Vec<_> = region_chunks()
            .into_iter()
            .take(chunks)
            .enumerate()
            .map(|(i, nbt)| {
                let version = [GZIP, ZLIB, NONE, LZ4][i % 4];
                (
                    i as i32 % 32,
                    i as i32 / 32,
                    version,
                    compress(version, &nbt),
                )
            })
            .collect();
        RegionFile::open(fixture.region(0, 0, &slots)).unwrap()
    }

    pub(crate) fn root(nbt: &[u8]) -> NbtCompound {
        mcrs_minecraft_nbt::Nbt::read(&mut NbtReadHelper::new(Cursor::new(nbt)))
            .unwrap()
            .root_tag
    }

    /// Where each id of `from` sits in `into`, interning by the entry's text.
    fn translation<V: PaletteId>(from: &PaletteNames<V>, into: &PaletteNames<V>) -> Vec<V> {
        (0..from.len())
            .map(|index| {
                let (name, properties) = from.entry(V::from_index(index).unwrap()).unwrap();
                let properties = properties.iter().map(|(k, v)| (k.as_str(), v.as_str()));
                into.intern(&name, properties).unwrap()
            })
            .collect()
    }

    fn cells<V: Copy + Eq + std::hash::Hash + Default, const DIM: usize>(
        container: &PalettedContainer<V, DIM>,
    ) -> Vec<V> {
        match container {
            PalettedContainer::Homogeneous(value) => vec![*value; DIM * DIM * DIM],
            PalettedContainer::Heterogeneous(data) => {
                data.cube.as_flattened().as_flattened().to_vec()
            }
        }
    }

    fn biome_cells_of(
        container: &Option<PalettedContainer<u8, { NoiseBiomes::SIZE }>>,
        table: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        container.as_ref().map(|c| {
            cells(c)
                .into_iter()
                .map(|v| table.map_or(v, |t| t[v.index()]))
                .collect()
        })
    }

    pub(crate) fn assert_same_chunk(read: &Named, original: &Named, what: &str) {
        let blocks = translation(&read.blocks, &original.blocks);
        let biomes = translation(&read.biomes, &original.biomes);
        let (a, b) = (&read.chunk, &original.chunk);
        assert_eq!(
            (a.pos, a.min_section_y, &a.status, a.is_light_on),
            (b.pos, b.min_section_y, &b.status, b.is_light_on),
            "{what}"
        );
        assert_eq!(
            (
                a.inhabited_time,
                a.last_update,
                &a.heightmaps,
                &a.block_entities,
                &a.retrogen
            ),
            (
                b.inhabited_time,
                b.last_update,
                &b.heightmaps,
                &b.block_entities,
                &b.retrogen
            ),
            "{what}"
        );
        assert_eq!(a.sections.len(), b.sections.len(), "{what}");
        for (sa, sb) in a.sections.iter().zip(&b.sections) {
            assert_eq!(sa.y, sb.y, "{what}");
            let block_cells = |s: &crate::Section, table: Option<&[VoxelId]>| {
                s.block_states.as_ref().map(|c| {
                    cells(c)
                        .into_iter()
                        .map(|v| table.map_or(v, |t| t[v.index()]))
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(
                block_cells(sa, Some(&blocks)),
                block_cells(sb, None),
                "{what}, section {}",
                sa.y
            );
            let biome_cells = |s: &crate::Section, table: Option<&[u8]>| {
                s.biomes.as_ref().map(|c| {
                    cells(c)
                        .into_iter()
                        .map(|v| table.map_or(v, |t| t[v.index()]))
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(
                biome_cells(sa, Some(&biomes)),
                biome_cells(sb, None),
                "{what}, section {}",
                sa.y
            );
            assert_eq!(
                biome_cells_of(&sa.noise_biomes, Some(&biomes)),
                biome_cells_of(&sb.noise_biomes, None),
                "{what}, section {}, noise biomes",
                sa.y
            );
            assert_eq!(sa.block_light, sb.block_light, "{what}, section {}", sa.y);
            assert_eq!(sa.sky_light, sb.sky_light, "{what}, section {}", sa.y);
        }
    }

    /// Slot index to the record's sectors and its timestamp.
    fn records(bytes: &[u8]) -> HashMap<usize, (&[u8], i32)> {
        (0..1024)
            .filter_map(|slot| {
                let entry = u32::from_be_bytes(bytes[slot * 4..slot * 4 + 4].try_into().unwrap());
                let (sector, count) = ((entry >> 8) as usize, (entry & 0xff) as usize);
                let at = SECTOR_BYTES + slot * 4;
                let timestamp = i32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
                (entry != 0).then(|| {
                    let span = &bytes[sector * SECTOR_BYTES..(sector + count) * SECTOR_BYTES];
                    (slot, (span, timestamp))
                })
            })
            .collect()
    }

    fn rewrite(region: &RegionFile, replaced: &[(ColumnPos, Vec<u8>)], dir: &Path) -> RegionFile {
        let out = dir.join(region.path().file_name().unwrap());
        region.write(replaced, &out).unwrap();
        RegionFile::open(out).unwrap()
    }

    #[test]
    fn a_region_read_and_written_back_reads_equal() {
        reads_equal_after_a_round_trip("write", 8);
    }

    mod exhaustive {
        #[test]
        fn a_region_read_and_written_back_reads_equal() {
            super::reads_equal_after_a_round_trip("write_all", super::fixture::CHUNKS);
        }
    }

    fn reads_equal_after_a_round_trip(name: &str, chunks: usize) {
        let src = Fixture::new(&format!("{name}_src"));
        let dst = Fixture::new(&format!("{name}_dst"));
        let region = fixture_region(&src, chunks);
        let originals: Vec<(ColumnPos, Named)> = region
            .present()
            .map(|pos| (pos, read_named(&region, pos)))
            .collect();
        assert_eq!(originals.len(), chunks);
        let replaced: Vec<(ColumnPos, Vec<u8>)> = originals
            .iter()
            .map(|(pos, n)| (*pos, write_chunk(&n.chunk, &n.blocks, &n.biomes).unwrap()))
            .collect();

        let mut arrays = 0;
        for (pos, nbt) in &replaced {
            let root = root(nbt);
            for section in root.get_list("sections").unwrap() {
                let NbtTag::Compound(section) = section else {
                    panic!("chunk {pos:?}: a section is not a compound");
                };
                for field in ["block_states", "biomes"] {
                    let container = section.get_compound(field).unwrap();
                    if let Some(data) = container.get("data") {
                        assert!(
                            matches!(data, NbtTag::LongArray(_)),
                            "{field} data: {data:?}"
                        );
                        arrays += 1;
                    }
                }
                for field in ["BlockLight", "SkyLight"] {
                    if let Some(light) = section.get(field) {
                        assert!(matches!(light, NbtTag::ByteArray(_)), "{field}: {light:?}");
                        arrays += 1;
                    }
                }
            }
        }
        assert!(arrays > 0, "no packed data or light was written");

        let written = rewrite(&region, &replaced, &dst.dir);
        for (pos, original) in &originals {
            let entries_before = (original.blocks.len(), original.biomes.len());
            let read = read_named(&written, *pos);
            assert_same_chunk(&read, original, &format!("chunk {pos:?}"));
            assert_eq!(
                (original.blocks.len(), original.biomes.len()),
                entries_before,
                "chunk {pos:?}: a written palette entry differs in text from every original one"
            );
        }
    }

    fn written_retrogen(record: NbtCompound, dir: &Fixture, name: &str) -> NbtCompound {
        let src = Fixture::new(name);
        let region = RegionFile::open(src.region(
            0,
            0,
            &single_slot(ZLIB, &chunk_with_retrogen("minecraft:terrain", record)),
        ))
        .unwrap();
        let pos = ColumnPos::new(0, 0);
        let original = read_named(&region, pos);
        let nbt = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();
        let read = read_named(&rewrite(&region, &[(pos, nbt.clone())], &dir.dir), pos);
        assert_same_chunk(&read, &original, name);
        root(&nbt).get_compound("retrogen").unwrap().clone()
    }

    #[test]
    fn an_empty_missing_bedrock_is_not_written() {
        let dst = Fixture::new("retrogen_empty_dst");
        let mut record = retrogen_record("minecraft:full", &["minecraft:biomes"]);
        record.put("missing_bedrock", NbtTag::LongArray(Vec::new()));
        let written = written_retrogen(record, &dst, "retrogen_empty_src");
        assert!(written.get("missing_bedrock").is_none());

        let mut record = retrogen_record("minecraft:full", &["minecraft:biomes"]);
        record.put("missing_bedrock", NbtTag::LongArray(vec![3, 0, 0]));
        let written = written_retrogen(record, &dst, "retrogen_zero_words_src");
        assert_eq!(
            written.get("missing_bedrock"),
            Some(&NbtTag::LongArray(vec![3]))
        );
    }

    #[test]
    fn noise_biomes_are_written_back_only_when_held() {
        let (src, dst) = (Fixture::new("noise_src"), Fixture::new("noise_dst"));
        let mut entries = vec![0u16; NoiseBiomes::ENTRY_COUNT];
        entries[NoiseBiomes::index(2, 1, 3)] = 1;
        let held = section_with_noise_biomes(0, quart_biomes(&entries));
        let bare = section(
            1,
            container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
        );
        let root_in = chunk_at(
            "minecraft:terrain",
            vec![NbtTag::Compound(held), NbtTag::Compound(bare)],
        );
        let region = RegionFile::open(src.region(0, 0, &single_slot(ZLIB, &root_in))).unwrap();
        let pos = ColumnPos::new(0, 0);
        let original = read_named(&region, pos);
        assert!(original.chunk.sections[0].noise_biomes.is_some());
        assert!(original.chunk.sections[1].noise_biomes.is_none());

        let nbt = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();
        let written = root(&nbt);
        let sections = written.get_list("sections").unwrap();
        let has_key = |index: usize| {
            let NbtTag::Compound(section) = &sections[index] else {
                panic!("a section is not a compound");
            };
            section.get("noise_biomes").is_some()
        };
        assert!(has_key(0));
        assert!(!has_key(1));

        let read = read_named(&rewrite(&region, &[(pos, nbt)], &dst.dir), pos);
        assert_same_chunk(&read, &original, "noise biomes");
    }

    #[test]
    fn a_block_set_by_name_reads_back() {
        let (src, dst) = (Fixture::new("set_block_src"), Fixture::new("set_block_dst"));
        let region = fixture_region(&src, 1);
        let pos = ColumnPos::new(0, 0);
        let mut original = read_named(&region, pos);
        let untouched = read_named(&region, pos);
        let torch = original
            .blocks
            .intern("minecraft:torch", std::iter::empty())
            .unwrap();
        let at = BlockPos::new(5, 70, 9);
        original.chunk.set_block(at, torch).unwrap();
        let nbt = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();

        let read = read_named(&rewrite(&region, &[(pos, nbt)], &dst.dir), pos);
        let section = read.chunk.sections.iter().find(|s| s.y == 70 >> 4).unwrap();
        let id = section.block_states.as_ref().unwrap().get(5, 70 & 15, 9);
        assert_eq!(
            read.blocks.entry(id),
            Some(("minecraft:torch".to_string(), vec![]))
        );

        let table = translation(&read.blocks, &untouched.blocks);
        let torch_in_untouched = untouched
            .blocks
            .intern("minecraft:torch", std::iter::empty())
            .unwrap();
        for (sa, sb) in read.chunk.sections.iter().zip(&untouched.chunk.sections) {
            let a: Vec<VoxelId> = cells(sa.block_states.as_ref().unwrap())
                .into_iter()
                .map(|v| table[v.index()])
                .collect();
            let mut b = cells(sb.block_states.as_ref().unwrap());
            if sb.y == 70 >> 4 {
                b[Blocks::index(5, 70 & 15, 9)] = torch_in_untouched;
            }
            assert_eq!(a, b, "section {}", sa.y);
        }
    }

    #[test]
    fn a_written_chunk_carries_the_current_data_version_and_asks_for_light() {
        let (src, dst) = (Fixture::new("version_src"), Fixture::new("version_dst"));
        let root_in = chunk_nbt_versioned(
            0,
            0,
            vec![NbtTag::Compound(section(
                0,
                container(vec![NbtTag::Compound(block("minecraft:stone"))], None),
            ))],
            VERSION.world_version,
        );
        let region = RegionFile::open(src.region(0, 0, &single_slot(ZLIB, &root_in))).unwrap();
        let pos = ColumnPos::new(0, 0);
        let mut edited = read_named(&region, pos);
        assert!(edited.chunk.is_light_on);
        let glowstone = edited
            .blocks
            .intern("minecraft:glowstone", std::iter::empty())
            .unwrap();
        edited
            .chunk
            .set_block(BlockPos::new(1, 2, 3), glowstone)
            .unwrap();
        let nbt = write_chunk(&edited.chunk, &edited.blocks, &edited.biomes).unwrap();

        assert_eq!(
            root(&nbt).get_int("DataVersion"),
            Some(VERSION.world_version)
        );
        let read = read_named(&rewrite(&region, &[(pos, nbt)], &dst.dir), pos);
        assert!(!read.chunk.is_light_on);
    }

    #[test]
    fn an_edited_chunk_drops_its_saved_light_and_heightmaps() {
        let (src, dst) = (Fixture::new("stale_src"), Fixture::new("stale_dst"));
        let region = fixture_region(&src, 8);
        let lit = region
            .present()
            .find(|&pos| {
                let n = read_named(&region, pos);
                !n.chunk.heightmaps.is_empty()
                    && n.chunk.sections.iter().any(|s| s.block_light.is_some())
            })
            .expect("a fixture chunk carries light and heightmaps");
        let mut edited = read_named(&region, lit);
        let torch = edited
            .blocks
            .intern("minecraft:torch", std::iter::empty())
            .unwrap();
        let at = BlockPos::new(lit.x * 16 + 3, 64, lit.z * 16 + 4);
        edited.chunk.set_block(at, torch).unwrap();
        let nbt = write_chunk(&edited.chunk, &edited.blocks, &edited.biomes).unwrap();
        assert!(root(&nbt).get("Heightmaps").is_none());

        let read = read_named(&rewrite(&region, &[(lit, nbt)], &dst.dir), lit);
        assert!(read.chunk.heightmaps.is_empty());
        for section in &read.chunk.sections {
            assert!(section.block_light.is_none(), "section {}", section.y);
            assert!(section.sky_light.is_none(), "section {}", section.y);
        }
    }

    #[test]
    fn a_chunk_needing_256_sectors_goes_to_an_external_file() {
        let (src, dst) = (Fixture::new("external_src"), Fixture::new("external_dst"));
        let root_in = chunk_nbt(
            33,
            2,
            vec![NbtTag::Compound(section(
                1,
                container(vec![NbtTag::Compound(block("minecraft:deepslate"))], None),
            ))],
        );
        let region = RegionFile::open(src.region(
            1,
            0,
            &[(33, 2, ZLIB, compress(ZLIB, &nbt_bytes(&root_in)))],
        ))
        .unwrap();
        let pos = ColumnPos::new(33, 2);
        let mut original = read_named(&region, pos);

        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let noise: Box<[u8]> = (0..(1 << 20) + (1 << 17))
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let mut chest = NbtCompound::new();
        chest.put_string("id", "minecraft:chest".to_string());
        chest.put_int("x", 33 * 16);
        chest.put_int("y", 20);
        chest.put_int("z", 2 * 16);
        chest.put("noise", NbtTag::ByteArray(noise));
        original.chunk.block_entities.push(chest);
        let nbt = write_chunk(&original.chunk, &original.blocks, &original.biomes).unwrap();

        let written = rewrite(&region, &[(pos, nbt)], &dst.dir);
        let bytes = std::fs::read(written.path()).unwrap();
        let (record, _) = records(&bytes)[&(2 * 32 + 1)];
        assert_eq!(record.len(), SECTOR_BYTES, "the stub takes one sector");
        assert_eq!(&record[..5], &[0, 0, 0, 1, ZLIB | EXTERNAL]);
        assert!(dst.dir.join("c.33.2.mcc").is_file());

        let read = read_named(&written, pos);
        assert_same_chunk(&read, &original, "external chunk");
    }

    #[test]
    fn writing_onto_the_file_it_was_read_from_is_refused() {
        let fixture = Fixture::new("refuse");
        let region = fixture_region(&fixture, 2);
        let before = std::fs::read(region.path()).unwrap();
        let pos = ColumnPos::new(0, 0);
        let n = read_named(&region, pos);
        let replaced = [(pos, write_chunk(&n.chunk, &n.blocks, &n.biomes).unwrap())];

        let same = region.path().to_path_buf();
        let spelled_otherwise = fixture.dir.join(".").join("r.0.0.mca");
        for out in [same, spelled_otherwise] {
            let err = region.write(&replaced, &out).unwrap_err();
            assert!(
                matches!(err.kind, ErrorKind::WriteOntoSource { .. }),
                "{}: {err}",
                out.display()
            );
        }
        assert_eq!(std::fs::read(region.path()).unwrap(), before);
    }

    #[test]
    fn a_state_without_properties_is_written_as_a_bare_name() {
        let fixture = Fixture::new("bare_names");
        let root_in = chunk_nbt(
            0,
            0,
            vec![NbtTag::Compound(section(
                0,
                container(
                    vec![
                        NbtTag::String("minecraft:stone".to_string()),
                        NbtTag::Compound(block_with("minecraft:oak_log", "axis", "y")),
                    ],
                    Some(pack(&[0, 1].repeat(2048), 4)),
                ),
            ))],
        );
        let region = RegionFile::open(fixture.region(0, 0, &single_slot(ZLIB, &root_in))).unwrap();
        let n = read_named(&region, ColumnPos::new(0, 0));
        let root = root(&write_chunk(&n.chunk, &n.blocks, &n.biomes).unwrap());
        let NbtTag::Compound(section) = &root.get_list("sections").unwrap()[0] else {
            panic!("a section is not a compound");
        };
        let palette = section
            .get_compound("block_states")
            .unwrap()
            .get_list("palette")
            .unwrap();
        assert_eq!(palette[0], NbtTag::String("minecraft:stone".to_string()));
        assert_eq!(
            palette[1],
            NbtTag::Compound(block_with("minecraft:oak_log", "axis", "y"))
        );
    }

    #[test]
    fn untouched_chunks_are_copied_byte_for_byte() {
        let (src, dst) = (Fixture::new("verbatim_src"), Fixture::new("verbatim_dst"));
        let region = fixture_region(&src, 40);
        let pos = ColumnPos::new(5, 0);
        let n = read_named(&region, pos);
        let replaced = [(pos, write_chunk(&n.chunk, &n.blocks, &n.biomes).unwrap())];

        let written = rewrite(&region, &replaced, &dst.dir);
        let (before, after) = (
            std::fs::read(region.path()).unwrap(),
            std::fs::read(written.path()).unwrap(),
        );
        let (before, after) = (records(&before), records(&after));
        assert_eq!(before.len(), 40);
        assert_eq!(after.len(), 40);
        for (slot, record) in &before {
            if *slot == 5 {
                continue;
            }
            assert_eq!(after.get(slot), Some(record), "slot {slot}");
        }
    }
}
