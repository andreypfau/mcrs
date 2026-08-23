use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::{GzDecoder, ZlibDecoder};
use lz4_java_wrc::Lz4BlockInput;

use crate::chunk::{self, Chunk};
use crate::{AnvilError, ErrorKind};

pub const SECTOR_BYTES: usize = 4096;
pub const REGION_SIDE: i32 = 32;

const HEADER_SECTORS: u32 = 2;
const HEADER_BYTES: usize = HEADER_SECTORS as usize * SECTOR_BYTES;
const CHUNK_HEADER_BYTES: usize = 5;
const EXTERNAL_STREAM_FLAG: u8 = 128;

impl std::fmt::Debug for RegionFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionFile")
            .field("path", &self.path)
            .field("region_x", &self.region_x)
            .field("region_z", &self.region_z)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

pub struct RegionFile {
    path: PathBuf,
    region_x: i32,
    region_z: i32,
    bytes: Vec<u8>,
}

impl RegionFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AnvilError> {
        let path = path.as_ref();
        let at = |kind: ErrorKind| AnvilError {
            path: path.to_path_buf(),
            kind,
        };
        let (region_x, region_z) =
            parse_region_name(path).ok_or_else(|| at(ErrorKind::FileName))?;
        let bytes = std::fs::read(path).map_err(|source| at(ErrorKind::Io(source)))?;
        if bytes.len() < HEADER_BYTES {
            return Err(at(ErrorKind::ShortHeader { len: bytes.len() }));
        }
        Ok(Self {
            path: path.to_path_buf(),
            region_x,
            region_z,
            bytes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn region_x(&self) -> i32 {
        self.region_x
    }

    pub fn region_z(&self) -> i32 {
        self.region_z
    }

    /// Absolute column coordinates of every slot whose header entry is non-zero.
    pub fn present(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..REGION_SIDE * REGION_SIDE)
            .filter(move |&slot| self.header_entry(slot as usize) != 0)
            .map(move |slot| {
                (
                    self.region_x * REGION_SIDE + slot % REGION_SIDE,
                    self.region_z * REGION_SIDE + slot / REGION_SIDE,
                )
            })
    }

    pub fn timestamp(&self, x: i32, z: i32) -> i32 {
        let head = HEADER_BYTES / 2 + slot_index(x, z) * 4;
        i32::from_be_bytes(self.bytes[head..head + 4].try_into().unwrap())
    }

    /// The chunk's decompressed NBT, or `None` when the slot is empty.
    pub fn chunk_nbt(&self, x: i32, z: i32) -> Result<Option<Vec<u8>>, AnvilError> {
        self.read_nbt(x, z).map_err(|kind| AnvilError {
            path: self.path.clone(),
            kind,
        })
    }

    pub fn read_chunk(&self, x: i32, z: i32) -> Result<Option<Chunk>, AnvilError> {
        self.read_nbt(x, z)
            .and_then(|nbt| nbt.map(|nbt| chunk::parse(&nbt)).transpose())
            .map_err(|kind| AnvilError {
                path: self.path.clone(),
                kind,
            })
    }

    fn header_entry(&self, slot: usize) -> i32 {
        let head = slot * 4;
        i32::from_be_bytes(self.bytes[head..head + 4].try_into().unwrap())
    }

    fn read_nbt(&self, x: i32, z: i32) -> Result<Option<Vec<u8>>, ErrorKind> {
        let (chunk_region_x, chunk_region_z) = (x >> 5, z >> 5);
        if (chunk_region_x, chunk_region_z) != (self.region_x, self.region_z) {
            return Err(ErrorKind::WrongRegion {
                x,
                z,
                chunk_region_x,
                chunk_region_z,
                region_x: self.region_x,
                region_z: self.region_z,
            });
        }

        let entry = self.header_entry(slot_index(x, z));
        if entry == 0 {
            return Ok(None);
        }
        let sector = (entry as u32) >> 8;
        let sector_count = (entry as u32) & 0xff;
        if sector < HEADER_SECTORS {
            return Err(ErrorKind::SectorInHeader { x, z, sector });
        }
        let sectors = self.bytes.len() / SECTOR_BYTES;
        let end = sector + sector_count;
        if end as usize > sectors {
            return Err(ErrorKind::SectorOutOfBounds {
                x,
                z,
                sector,
                end,
                sectors,
            });
        }

        let span = &self.bytes[sector as usize * SECTOR_BYTES..end as usize * SECTOR_BYTES];
        let length = i32::from_be_bytes(span[..4].try_into().unwrap());
        let version = span[4];
        let available = span.len() - CHUNK_HEADER_BYTES;
        if length < 1 || (length - 1) as usize > available {
            return Err(ErrorKind::PayloadLength {
                x,
                z,
                length,
                available,
            });
        }

        if version & EXTERNAL_STREAM_FLAG != 0 {
            let name = format!("c.{x}.{z}.mcc");
            let external = self.path.with_file_name(&name);
            let payload =
                std::fs::read(&external).map_err(|_| ErrorKind::MissingExternal { x, z, name })?;
            return decompress(version & !EXTERNAL_STREAM_FLAG, &payload, x, z).map(Some);
        }

        let payload = &span[CHUNK_HEADER_BYTES..CHUNK_HEADER_BYTES + (length - 1) as usize];
        decompress(version, payload, x, z).map(Some)
    }
}

fn slot_index(x: i32, z: i32) -> usize {
    ((z & 31) * REGION_SIDE + (x & 31)) as usize
}

fn parse_region_name(path: &Path) -> Option<(i32, i32)> {
    let name = path.file_name()?.to_str()?;
    let ["r", x, z, "mca"] = name.split('.').collect::<Vec<_>>()[..] else {
        return None;
    };
    Some((x.parse().ok()?, z.parse().ok()?))
}

fn decompress(version: u8, payload: &[u8], x: i32, z: i32) -> Result<Vec<u8>, ErrorKind> {
    let mut out = Vec::new();
    let read =
        |mut reader: Box<dyn Read + '_>, out: &mut Vec<u8>| reader.read_to_end(out).map(drop);
    let result = match version {
        1 => read(Box::new(GzDecoder::new(payload)), &mut out),
        2 => read(Box::new(ZlibDecoder::new(payload)), &mut out),
        3 => read(Box::new(payload), &mut out),
        4 => read(Box::new(Lz4BlockInput::new(payload)), &mut out),
        127 => return Err(ErrorKind::CustomCompression { x, z }),
        id => return Err(ErrorKind::UnknownCompression { x, z, id }),
    };
    result.map_err(|source| ErrorKind::Decompress { x, z, source })?;
    Ok(out)
}
