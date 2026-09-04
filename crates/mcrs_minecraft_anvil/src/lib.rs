mod chunk;
#[cfg(any(test, feature = "bench-helpers"))]
pub mod fixture;
mod palette;
mod region;

#[cfg(test)]
mod tests;

pub use chunk::{
    Biomes, BlockStates, Chunk, LIGHT_BYTES, Light, PalettedContainer, Section,
    parse as parse_chunk,
};
pub use palette::{BlockStateLookup, Palette, Properties};
pub use region::{REGION_SIDE, RegionFile, SECTOR_BYTES};

use std::path::PathBuf;

/// 26.3 Pre-Release 1; the oldest accepted is snapshot 10, the first with this layout.
pub const DATA_VERSION: i32 = 5017;
pub const OLDEST_DATA_VERSION: i32 = 5015;

pub fn accepts_data_version(found: i32) -> bool {
    (OLDEST_DATA_VERSION..=DATA_VERSION).contains(&found)
}

#[derive(Debug, thiserror::Error)]
#[error("{path}: {kind}")]
pub struct AnvilError {
    pub path: PathBuf,
    pub kind: ErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("file name is not `r.<x>.<z>.mca`")]
    FileName,
    #[error("{len} bytes, shorter than the 8192-byte header")]
    ShortHeader { len: usize },
    #[error(
        "chunk {x},{z} belongs to region {chunk_region_x},{chunk_region_z}, not {region_x},{region_z}"
    )]
    WrongRegion {
        x: i32,
        z: i32,
        chunk_region_x: i32,
        chunk_region_z: i32,
        region_x: i32,
        region_z: i32,
    },
    #[error("chunk {x},{z}: sector {sector} overlaps the header")]
    SectorInHeader { x: i32, z: i32, sector: u32 },
    #[error("chunk {x},{z}: sectors {sector}..{end} run past the {sectors}-sector file")]
    SectorOutOfBounds {
        x: i32,
        z: i32,
        sector: u32,
        end: u32,
        sectors: usize,
    },
    #[error(
        "chunk {x},{z}: payload length {length}, but only {available} bytes are in its sectors"
    )]
    PayloadLength {
        x: i32,
        z: i32,
        length: i32,
        available: usize,
    },
    #[error("chunk {x},{z}: compression id {id} is not a RegionFileVersion")]
    UnknownCompression { x: i32, z: i32, id: u8 },
    #[error("chunk {x},{z}: compression id 127 is the custom scheme, which has no reader")]
    CustomCompression { x: i32, z: i32 },
    #[error("chunk {x},{z}: {source}")]
    Decompress {
        x: i32,
        z: i32,
        source: std::io::Error,
    },
    #[error("chunk {x},{z}: external chunk file `{name}` is missing")]
    MissingExternal { x: i32, z: i32, name: String },
    #[error("{0}")]
    Nbt(#[from] mcrs_minecraft_nbt::Error),
    #[error("DataVersion {found}, expected {OLDEST_DATA_VERSION} to {expected}")]
    DataVersion { found: i32, expected: i32 },
    #[error("no DataVersion, so older than the tag itself; expected {OLDEST_DATA_VERSION} to {expected}")]
    MissingDataVersion { expected: i32 },
    #[error("`{name}` is not a block state this registry knows")]
    UnknownPaletteEntry { name: String },
    #[error("section {y}: `{field}` palette is empty")]
    EmptyPalette { y: i8, field: &'static str },
    #[error(
        "section {y}: `{field}` palette has {len} entries, more than the {max} an index can address"
    )]
    PaletteTooLarge {
        y: i8,
        field: &'static str,
        len: usize,
        max: usize,
    },
    #[error("section {y}: `{field}` needs {bits} bits per entry but has no `data`")]
    MissingData {
        y: i8,
        field: &'static str,
        bits: u32,
    },
    #[error("section {y}: `{field}` has a single-entry palette but also carries `data`")]
    UnexpectedData { y: i8, field: &'static str },
    #[error(
        "section {y}: `{field}` `data` is {found} longs, expected {expected} at {bits} bits per entry"
    )]
    DataLength {
        y: i8,
        field: &'static str,
        found: usize,
        expected: usize,
        bits: u32,
    },
    #[error("section {y}: `{field}` entry {index} is out of range for a palette of {len}")]
    PaletteIndex {
        y: i8,
        field: &'static str,
        index: u16,
        len: usize,
    },
    #[error("section {y}: `{field}` is {found} bytes, expected 2048")]
    LightLength {
        y: i8,
        field: &'static str,
        found: usize,
    },
}
