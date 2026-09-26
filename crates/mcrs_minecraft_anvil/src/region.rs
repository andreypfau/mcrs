use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::{GzDecoder, ZlibDecoder};
use lz4_java_wrc::Lz4BlockInput;

use crate::chunk::{self, Chunk};
use crate::palette::PaletteLookup;
use crate::{AnvilError, ErrorKind};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{ColumnPos, RegionPos};

pub const SECTOR_BYTES: usize = 4096;
pub const REGION_SIDE: i32 = 32;

const HEADER_SECTORS: u32 = 2;
const HEADER_BYTES: usize = HEADER_SECTORS as usize * SECTOR_BYTES;
const CHUNK_HEADER_BYTES: usize = 5;
const EXTERNAL_STREAM_FLAG: u8 = 128;
const ZLIB: u8 = 2;
/// A record this many sectors or longer is stored in its own `c.<x>.<z>.mcc` file.
const EXTERNAL_SECTORS: usize = 256;

impl std::fmt::Debug for RegionFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionFile")
            .field("path", &self.path)
            .field("pos", &self.pos)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

pub struct RegionFile {
    path: PathBuf,
    pos: RegionPos,
    bytes: Vec<u8>,
}

impl RegionFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AnvilError> {
        let path = path.as_ref();
        let at = |kind: ErrorKind| AnvilError {
            path: path.to_path_buf(),
            kind,
        };
        let pos = parse_region_name(path).ok_or_else(|| at(ErrorKind::FileName))?;
        let bytes = std::fs::read(path).map_err(|source| at(ErrorKind::Io(source)))?;
        if bytes.len() < HEADER_BYTES {
            return Err(at(ErrorKind::ShortHeader { len: bytes.len() }));
        }
        Ok(Self {
            path: path.to_path_buf(),
            pos,
            bytes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pos(&self) -> RegionPos {
        self.pos
    }

    /// Absolute column coordinates of every slot whose header entry is non-zero.
    pub fn present(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        (0..REGION_SIDE * REGION_SIDE)
            .filter(move |&slot| self.header_entry(slot as usize) != 0)
            .map(move |slot| self.pos.column_at(slot % REGION_SIDE, slot / REGION_SIDE))
    }

    pub fn read_chunk(
        &self,
        pos: ColumnPos,
        blocks: &impl PaletteLookup<VoxelId>,
        biomes: &impl PaletteLookup<u8>,
    ) -> Result<Option<Chunk>, AnvilError> {
        self.read_nbt(pos)
            .and_then(|nbt| {
                nbt.map(|nbt| chunk::parse(&nbt, blocks, biomes))
                    .transpose()
            })
            .map_err(|kind| AnvilError {
                path: self.path.clone(),
                kind,
            })
    }

    fn header_entry(&self, slot: usize) -> i32 {
        let head = slot * 4;
        i32::from_be_bytes(self.bytes[head..head + 4].try_into().unwrap())
    }

    /// Lays out a whole new file at `out`: every slot not in `replaced` keeps its
    /// record byte for byte, and each replaced chunk is stored zlib-compressed.
    /// The file is written beside `out` and renamed into place.
    pub fn write(&self, replaced: &[(ColumnPos, Vec<u8>)], out: &Path) -> Result<(), AnvilError> {
        let at = |kind: ErrorKind| AnvilError {
            path: out.to_path_buf(),
            kind,
        };
        let io = |source: std::io::Error| at(ErrorKind::Io(source));
        match parse_region_name(out) {
            None => return Err(at(ErrorKind::FileName)),
            Some(pos) if pos != self.pos => {
                return Err(at(ErrorKind::OtherRegionName {
                    x: self.pos.x,
                    z: self.pos.z,
                }));
            }
            Some(_) => {}
        }
        let dir = out
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let source = self.path.canonicalize().map_err(io)?;
        let target = match out.canonicalize() {
            Ok(target) => target,
            Err(_) => dir
                .canonicalize()
                .map_err(io)?
                .join(out.file_name().unwrap()),
        };
        if source == target {
            return Err(at(ErrorKind::WriteOntoSource {
                source_path: self.path.clone(),
            }));
        }
        let same_dir = source.parent() == target.parent();

        let mut new: Vec<Option<&[u8]>> = vec![None; (REGION_SIDE * REGION_SIDE) as usize];
        for (pos, nbt) in replaced {
            let region = RegionPos::from(*pos);
            if region != self.pos {
                return Err(at(ErrorKind::WrongRegion {
                    x: pos.x,
                    z: pos.z,
                    chunk_region_x: region.x,
                    chunk_region_z: region.z,
                    region_x: self.pos.x,
                    region_z: self.pos.z,
                }));
            }
            new[slot_index(*pos)] = Some(nbt);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i32);
        let mut header = vec![0u8; HEADER_BYTES];
        let mut body = Vec::new();
        let mut externals = Vec::new();
        let mut stale = Vec::new();
        for slot in 0..(REGION_SIDE * REGION_SIDE) as usize {
            let pos = self
                .pos
                .column_at(slot as i32 % REGION_SIDE, slot as i32 / REGION_SIDE);
            let external = || dir.join(format!("c.{}.{}.mcc", pos.x, pos.z));
            let timestamp;
            let record = match new[slot] {
                Some(nbt) => {
                    timestamp = now;
                    let payload = zlib(nbt).map_err(io)?;
                    let mut record = Vec::with_capacity(CHUNK_HEADER_BYTES + payload.len());
                    if (CHUNK_HEADER_BYTES + payload.len()).div_ceil(SECTOR_BYTES)
                        >= EXTERNAL_SECTORS
                    {
                        record.extend_from_slice(&1i32.to_be_bytes());
                        record.push(ZLIB | EXTERNAL_STREAM_FLAG);
                        externals.push((external(), payload));
                    } else {
                        record.extend_from_slice(&(payload.len() as i32 + 1).to_be_bytes());
                        record.push(ZLIB);
                        record.extend_from_slice(&payload);
                        stale.push(external());
                    }
                    record.resize(record.len().div_ceil(SECTOR_BYTES) * SECTOR_BYTES, 0);
                    record
                }
                None => match self.record(pos).map_err(at)? {
                    None => continue,
                    Some(span) => {
                        timestamp = self.timestamp(slot);
                        if !same_dir && span.get(4).is_some_and(|v| v & EXTERNAL_STREAM_FLAG != 0) {
                            let name = format!("c.{}.{}.mcc", pos.x, pos.z);
                            let from = self.path.with_file_name(&name);
                            let payload = std::fs::read(&from).map_err(|_| {
                                at(ErrorKind::MissingExternal {
                                    x: pos.x,
                                    z: pos.z,
                                    name,
                                })
                            })?;
                            externals.push((external(), payload));
                        }
                        span.to_vec()
                    }
                },
            };
            let sector = HEADER_SECTORS as usize + body.len() / SECTOR_BYTES;
            let count = record.len() / SECTOR_BYTES;
            header[slot * 4..slot * 4 + 4]
                .copy_from_slice(&((sector as u32) << 8 | count as u32).to_be_bytes());
            header[SECTOR_BYTES + slot * 4..SECTOR_BYTES + slot * 4 + 4]
                .copy_from_slice(&timestamp.to_be_bytes());
            body.extend_from_slice(&record);
        }

        for (path, payload) in &externals {
            replace_file(path, &[payload]).map_err(io)?;
        }
        replace_file(out, &[&header, &body]).map_err(io)?;
        for path in stale {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io(e)),
                _ => {}
            }
        }
        Ok(())
    }

    fn timestamp(&self, slot: usize) -> i32 {
        let at = SECTOR_BYTES + slot * 4;
        i32::from_be_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }

    /// The sectors a slot's header entry points at, checked against the file.
    fn record(&self, pos: ColumnPos) -> Result<Option<&[u8]>, ErrorKind> {
        let ColumnPos { x, z } = pos;
        let chunk_region = RegionPos::from(pos);
        if chunk_region != self.pos {
            return Err(ErrorKind::WrongRegion {
                x,
                z,
                chunk_region_x: chunk_region.x,
                chunk_region_z: chunk_region.z,
                region_x: self.pos.x,
                region_z: self.pos.z,
            });
        }

        let entry = self.header_entry(slot_index(pos));
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

        Ok(Some(
            &self.bytes[sector as usize * SECTOR_BYTES..end as usize * SECTOR_BYTES],
        ))
    }

    fn read_nbt(&self, pos: ColumnPos) -> Result<Option<Vec<u8>>, ErrorKind> {
        let ColumnPos { x, z } = pos;
        let Some(span) = self.record(pos)? else {
            return Ok(None);
        };
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

fn zlib(nbt: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(nbt)?;
    encoder.finish()
}

fn replace_file(path: &Path, parts: &[&[u8]]) -> std::io::Result<()> {
    let name = path.file_name().unwrap().to_string_lossy();
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let written = std::fs::File::create(&temp).and_then(|mut file| {
        for part in parts {
            file.write_all(part)?;
        }
        file.sync_all()
    });
    match written.and_then(|()| std::fs::rename(&temp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

fn slot_index(pos: ColumnPos) -> usize {
    (pos.region_local_z() * REGION_SIDE + pos.region_local_x()) as usize
}

fn parse_region_name(path: &Path) -> Option<RegionPos> {
    let name = path.file_name()?.to_str()?;
    let ["r", x, z, "mca"] = name.split('.').collect::<Vec<_>>()[..] else {
        return None;
    };
    Some(RegionPos::new(x.parse().ok()?, z.parse().ok()?))
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
