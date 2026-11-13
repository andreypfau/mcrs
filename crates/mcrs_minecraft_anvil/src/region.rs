use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::{GzDecoder, ZlibDecoder};
use lz4_java_wrc::Lz4BlockInput;

use crate::atomic::{temp_path, write_atomic};
use crate::chunk::{self, Chunk};
use crate::palette::PaletteLookup;
use crate::{AnvilError, ErrorKind};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::RegionPos;
use mcrs_voxel::ColumnPos;

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
    /// The two header sectors; sector payloads stay on disk until a chunk asks for one.
    bytes: Vec<u8>,
    len: u64,
}

#[derive(Clone, Copy, Debug)]
struct Record {
    sector: u32,
    count: u32,
}

impl RegionFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AnvilError> {
        let path = path.as_ref();
        let at = |kind: ErrorKind| AnvilError {
            path: path.to_path_buf(),
            kind,
        };
        let pos = parse_region_name(path).ok_or_else(|| at(ErrorKind::FileName))?;
        let mut file = std::fs::File::open(path).map_err(|source| at(ErrorKind::Io(source)))?;
        let len = file
            .metadata()
            .map_err(|source| at(ErrorKind::Io(source)))?
            .len();
        if len < HEADER_BYTES as u64 {
            return Err(at(ErrorKind::ShortHeader {
                len: usize::try_from(len).unwrap_or(usize::MAX),
            }));
        }
        let mut bytes = vec![0; HEADER_BYTES];
        file.read_exact(&mut bytes)
            .map_err(|source| at(ErrorKind::Io(source)))?;
        Ok(Self {
            path: path.to_path_buf(),
            pos,
            bytes,
            len,
        })
    }

    /// An empty region file that a new save starts from.
    pub fn create(pos: RegionPos, path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            pos,
            bytes: vec![0; HEADER_BYTES],
            len: HEADER_BYTES as u64,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pos(&self) -> RegionPos {
        self.pos
    }

    /// The bytes this handle keeps in memory, which is only ever the two header sectors.
    #[cfg(test)]
    pub fn bytes_held(&self) -> usize {
        self.bytes.len()
    }

    /// Whether a header entry points into the header or outside the file.
    pub fn has_damaged_slot(&self) -> bool {
        self.present().any(|pos| self.record(pos).is_err())
    }

    /// The columns whose record lives in their own `.mcc` file.
    pub fn external_columns(&self) -> Vec<ColumnPos> {
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        self.present()
            .filter(|&pos| {
                self.record(pos)
                    .ok()
                    .flatten()
                    .and_then(|record| record.read_version(&mut file))
                    .is_some_and(|version| version & EXTERNAL_STREAM_FLAG != 0)
            })
            .collect()
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
                nbt.map(|nbt| chunk::parse_column(&nbt, blocks, biomes))
                    .transpose()
                    .map(Option::flatten)
            })
            .and_then(|chunk| match chunk {
                Some(chunk) if chunk.pos != pos => Err(ErrorKind::WrongPosition {
                    slot_x: pos.x,
                    slot_z: pos.z,
                    x: chunk.pos.x,
                    z: chunk.pos.z,
                }),
                chunk => Ok(chunk),
            })
            .map_err(|kind| AnvilError {
                path: self.path.clone(),
                kind,
            })
    }

    fn header_entry(&self, slot: usize) -> i32 {
        be_i32(&self.bytes, slot * 4).unwrap_or(0)
    }

    /// Lays out a whole new file at `out`: every slot not in `replaced` keeps its
    /// record byte for byte, and each replaced chunk is stored zlib-compressed.
    /// Sectors stream from the source into a file beside `out`, which is renamed into place.
    /// A slot whose header entry is invalid is left out, so the caller copies the region first.
    /// A source that cannot be read fails the write.
    pub fn write(
        &mut self,
        replaced: &[(ColumnPos, Vec<u8>)],
        out: &Path,
    ) -> Result<(), AnvilError> {
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
        let source = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        let target = match out.canonicalize() {
            Ok(target) => target,
            Err(_) => dir
                .canonicalize()
                .map_err(io)?
                .join(out.file_name().ok_or_else(|| at(ErrorKind::FileName))?),
        };
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
            if let Some(slot) = new.get_mut(slot_index(*pos)) {
                *slot = Some(nbt);
            }
        }

        let temp = temp_path(out).map_err(io)?;
        let result = self.write_layout(&new, out, &temp, &at, &io, same_dir);
        match result {
            Ok(()) => {
                if source == target {
                    self.reload_header().map_err(io)?;
                }
                Ok(())
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(error)
            }
        }
    }

    fn reload_header(&mut self) -> std::io::Result<()> {
        let mut file = std::fs::File::open(&self.path)?;
        self.len = file.metadata()?.len();
        file.read_exact(&mut self.bytes)
    }

    fn write_layout(
        &self,
        new: &[Option<&[u8]>],
        out: &Path,
        temp: &Path,
        at: &impl Fn(ErrorKind) -> AnvilError,
        io: &impl Fn(std::io::Error) -> AnvilError,
        same_dir: bool,
    ) -> Result<(), AnvilError> {
        let dir = out
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs() as i32);
        let mut header = vec![0u8; HEADER_BYTES];
        let mut externals = Vec::new();
        let mut stale = Vec::new();
        let mut sectors = 0u32;
        let mut source = match std::fs::File::open(&self.path) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(io(error)),
        };
        let mut file = std::fs::File::create(temp).map_err(io)?;
        file.write_all(&header).map_err(io)?;
        for (slot, replacement) in new.iter().enumerate() {
            let pos = self
                .pos
                .column_at(slot as i32 % REGION_SIDE, slot as i32 / REGION_SIDE);
            let external = || dir.join(format!("c.{}.{}.mcc", pos.x, pos.z));
            let timestamp;
            let record = match replacement {
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
                None => {
                    let (Some(source), Ok(Some(kept))) = (source.as_mut(), self.record(pos)) else {
                        continue;
                    };
                    let (record, external_payload) =
                        self.kept_record(source, kept, pos, same_dir).map_err(at)?;
                    timestamp = self.timestamp(slot);
                    if let Some(payload) = external_payload {
                        externals.push((external(), payload));
                    }
                    record
                }
            };
            let sector = HEADER_SECTORS + sectors;
            let count = u32::try_from(record.len() / SECTOR_BYTES).map_err(|_| {
                at(ErrorKind::OtherRegionName {
                    x: self.pos.x,
                    z: self.pos.z,
                })
            })?;
            put_be(&mut header, slot * 4, (sector << 8) | count);
            put_be(&mut header, SECTOR_BYTES + slot * 4, timestamp as u32);
            file.write_all(&record).map_err(io)?;
            sectors += count;
        }
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        file.write_all(&header).map_err(io)?;
        file.sync_all().map_err(io)?;
        drop(file);

        for (path, payload) in &externals {
            write_atomic(path, payload).map_err(io)?;
        }
        std::fs::rename(temp, out).map_err(io)?;
        for path in stale {
            match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(io(error));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The record of a slot that stays as it is, and its `.mcc` payload when the new file is in
    /// another directory.
    fn kept_record(
        &self,
        source: &mut std::fs::File,
        record: Record,
        pos: ColumnPos,
        same_dir: bool,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), ErrorKind> {
        let bytes = read_sectors(source, record)?;
        let mut payload = None;
        if !same_dir
            && bytes
                .get(4)
                .is_some_and(|version| version & EXTERNAL_STREAM_FLAG != 0)
        {
            let name = format!("c.{}.{}.mcc", pos.x, pos.z);
            payload = Some(std::fs::read(self.path.with_file_name(&name)).map_err(|_| {
                ErrorKind::MissingExternal {
                    x: pos.x,
                    z: pos.z,
                    name,
                }
            })?);
        }
        Ok((bytes, payload))
    }

    fn timestamp(&self, slot: usize) -> i32 {
        be_i32(&self.bytes, SECTOR_BYTES + slot * 4).unwrap_or(0)
    }

    /// The sectors a slot's header entry points at, checked against the file.
    fn record(&self, pos: ColumnPos) -> Result<Option<Record>, ErrorKind> {
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
        let count = (entry as u32) & 0xff;
        if sector < HEADER_SECTORS {
            return Err(ErrorKind::SectorInHeader { x, z, sector });
        }
        let end = sector + count;
        let sectors = usize::try_from(self.len.div_ceil(SECTOR_BYTES as u64)).unwrap_or(usize::MAX);
        if end as usize > sectors {
            return Err(ErrorKind::SectorOutOfBounds {
                x,
                z,
                sector,
                end,
                sectors,
            });
        }
        Ok(Some(Record { sector, count }))
    }

    fn read_record(&self, record: Record) -> Result<Vec<u8>, ErrorKind> {
        let mut file = std::fs::File::open(&self.path)?;
        Ok(read_sectors(&mut file, record)?)
    }

    /// The decompressed NBT of the record at `pos`, or `None` when the slot is empty.
    pub fn read_nbt(&self, pos: ColumnPos) -> Result<Option<Vec<u8>>, ErrorKind> {
        let ColumnPos { x, z } = pos;
        let Some(record) = self.record(pos)? else {
            return Ok(None);
        };
        let span = self.read_record(record)?;
        let available = span.len().saturating_sub(CHUNK_HEADER_BYTES);
        let (Some(length), Some(&version)) = (be_i32(&span, 0), span.get(4)) else {
            return Err(ErrorKind::PayloadLength {
                x,
                z,
                length: 0,
                available,
            });
        };
        let payload = usize::try_from(length)
            .ok()
            .and_then(|length| length.checked_sub(1))
            .and_then(|stored| span.get(CHUNK_HEADER_BYTES..CHUNK_HEADER_BYTES + stored));
        let Some(payload) = payload else {
            return Err(ErrorKind::PayloadLength {
                x,
                z,
                length,
                available,
            });
        };

        if version & EXTERNAL_STREAM_FLAG != 0 {
            let name = format!("c.{x}.{z}.mcc");
            let external = self.path.with_file_name(&name);
            let payload =
                std::fs::read(&external).map_err(|_| ErrorKind::MissingExternal { x, z, name })?;
            return decompress(version & !EXTERNAL_STREAM_FLAG, &payload, x, z).map(Some);
        }

        decompress(version, payload, x, z).map(Some)
    }
}

impl Record {
    fn read_version(self, file: &mut std::fs::File) -> Option<u8> {
        let offset = u64::from(self.sector) * SECTOR_BYTES as u64 + 4;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut version = [0];
        file.read_exact(&mut version).ok()?;
        Some(version[0])
    }
}

fn read_sectors(file: &mut std::fs::File, record: Record) -> std::io::Result<Vec<u8>> {
    let mut bytes = vec![0; record.count as usize * SECTOR_BYTES];
    file.seek(SeekFrom::Start(
        u64::from(record.sector) * SECTOR_BYTES as u64,
    ))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn be_i32(bytes: &[u8], at: usize) -> Option<i32> {
    bytes
        .get(at..)?
        .first_chunk()
        .copied()
        .map(i32::from_be_bytes)
}

fn put_be(bytes: &mut [u8], at: usize, value: u32) {
    if let Some(slot) = bytes.get_mut(at..).and_then(<[u8]>::first_chunk_mut) {
        *slot = value.to_be_bytes();
    }
}

fn zlib(nbt: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(nbt)?;
    encoder.finish()
}

fn slot_index(pos: ColumnPos) -> usize {
    (RegionPos::local_z(pos) * REGION_SIDE + RegionPos::local_x(pos)) as usize
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_slot_does_not_stop_the_rewrite_of_the_others() {
        let dir = std::env::temp_dir().join(format!("mcrs-anvil-region-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.0.0.mca");
        let (kept, broken, replaced) = (
            ColumnPos::new(0, 0),
            ColumnPos::new(1, 0),
            ColumnPos::new(2, 0),
        );
        let mut bytes = vec![0u8; HEADER_BYTES];
        let mut record = Vec::new();
        record.extend_from_slice(&1i32.to_be_bytes());
        record.push(3);
        record.resize(SECTOR_BYTES, 0);
        bytes.extend_from_slice(&record);
        put_be(&mut bytes, slot_index(kept) * 4, (HEADER_SECTORS << 8) | 1);
        put_be(&mut bytes, slot_index(broken) * 4, (900 << 8) | 1);
        std::fs::write(&path, bytes).unwrap();

        RegionFile::open(&path)
            .unwrap()
            .write(&[(replaced, vec![0])], &path)
            .unwrap();

        let region = RegionFile::open(&path).unwrap();
        let present: Vec<_> = region.present().collect();
        assert_eq!(present, vec![kept, replaced]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_source_that_cannot_be_read_fails_the_write_and_leaves_no_new_file() {
        let dir = std::env::temp_dir().join(format!("mcrs-anvil-unread-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("out")).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let source = dir.join("src/r.0.0.mca");
        let kept = ColumnPos::new(0, 0);
        let mut bytes = vec![0u8; HEADER_BYTES];
        bytes.resize(HEADER_BYTES + SECTOR_BYTES, 0);
        put_be(&mut bytes, slot_index(kept) * 4, (HEADER_SECTORS << 8) | 1);
        std::fs::write(&source, bytes).unwrap();
        let mut region = RegionFile::open(&source).unwrap();
        std::fs::remove_file(&source).unwrap();
        std::fs::create_dir(&source).unwrap();

        let out = dir.join("out/r.0.0.mca");
        let result = region.write(&[(ColumnPos::new(2, 0), vec![0])], &out);
        assert!(result.is_err());
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_handle_reads_the_new_layout_after_an_in_place_write() {
        let dir = std::env::temp_dir().join(format!("mcrs-anvil-inplace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.0.0.mca");
        let (first, second) = (ColumnPos::new(0, 0), ColumnPos::new(1, 0));
        let mut bytes = vec![0u8; HEADER_BYTES];
        for (n, pos) in [first, second].into_iter().enumerate() {
            let mut record = 2i32.to_be_bytes().to_vec();
            record.extend_from_slice(&[3, n as u8 + 1]);
            record.resize(SECTOR_BYTES, 0);
            bytes.extend_from_slice(&record);
            put_be(
                &mut bytes,
                slot_index(pos) * 4,
                ((HEADER_SECTORS + n as u32) << 8) | 1,
            );
        }
        std::fs::write(&path, bytes).unwrap();

        let mut region = RegionFile::open(&path).unwrap();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let noise: Vec<u8> = (0..20_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect();
        region.write(&[(first, noise)], &path).unwrap();

        assert_eq!(region.read_nbt(second).unwrap(), Some(vec![2]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
