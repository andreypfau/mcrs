use std::fmt::Write;
use std::io::Read;
use std::ops::Range;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use sha1::{Digest, Sha1};

#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(not(target_family = "wasm"))]
pub use native::*;

#[derive(Clone, Copy, Debug)]
pub struct Artifact<'a> {
    pub url: &'a str,
    pub sha1: &'a str,
    pub size: u64,
}

pub const CLIENT_JAR: Artifact<'static> = Artifact {
    url: "https://piston-data.mojang.com/v1/objects/e877b6a07acd633fb3bb475002175cec036e7b87/client.jar",
    sha1: "e877b6a07acd633fb3bb475002175cec036e7b87",
    size: 41483720,
};

#[derive(Default, Debug)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
    status: Mutex<String>,
}

impl Progress {
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }

    pub fn set_done(&self, bytes: u64) {
        self.done.store(bytes, Ordering::Relaxed);
    }

    pub fn set_total(&self, bytes: u64) {
        self.total.store(bytes, Ordering::Relaxed);
    }

    pub fn set_status(&self, status: impl Into<String>) {
        *self.status.lock().unwrap() = status.into();
    }
}

pub fn verify(artifact: &Artifact, bytes: &[u8]) -> bool {
    bytes.len() as u64 == artifact.size && sha1_hex(bytes) == artifact.sha1
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .fold(String::with_capacity(40), |mut hex, byte| {
            write!(hex, "{byte:02x}").unwrap();
            hex
        })
}

pub type Files = Vec<(String, Vec<u8>)>;

const CENTRAL_HEADER: u32 = 0x0201_4b50;
const LOCAL_HEADER: u32 = 0x0403_4b50;
const END_OF_DIRECTORY: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    offset: u64,
    method: u16,
    crc32: u32,
    compressed: u64,
    size: u64,
}

/// A zip's central directory: what every entry is and where its bytes lie.
#[derive(Clone, Debug)]
pub struct Directory {
    entries: Vec<Entry>,
    start: u64,
}

impl Directory {
    /// Reads the records of a central directory that starts at `start` in the zip and runs
    /// through `bytes` up to its end record.
    pub fn parse(bytes: &[u8], start: u64) -> Result<Self, String> {
        let mut entries = Vec::new();
        let mut rest = bytes;
        while rest.len() >= 46 && u32_at(rest, 0) == CENTRAL_HEADER {
            let name_len = u16_at(rest, 28) as usize;
            let end = 46 + name_len + u16_at(rest, 30) as usize + u16_at(rest, 32) as usize;
            let name = rest
                .get(46..46 + name_len)
                .and_then(|name| std::str::from_utf8(name).ok())
                .ok_or("a central directory record has no readable name")?;
            entries.push(Entry {
                name: name.to_owned(),
                offset: u32_at(rest, 42) as u64,
                method: u16_at(rest, 10),
                crc32: u32_at(rest, 16),
                compressed: u32_at(rest, 20) as u64,
                size: u32_at(rest, 24) as u64,
            });
            rest = rest
                .get(end..)
                .ok_or("the central directory is truncated")?;
        }
        if entries.is_empty() {
            return Err("the central directory has no records".to_owned());
        }
        Ok(Self { entries, start })
    }

    /// The directory of a whole zip, found through its end record.
    pub fn of(zip: &[u8]) -> Result<Self, String> {
        let end = zip
            .windows(4)
            .rposition(|word| word == END_OF_DIRECTORY)
            .filter(|&end| zip.len() >= end + 22)
            .ok_or("no end of central directory record")?;
        let start = u32_at(zip, end + 16) as u64;
        let bytes = zip
            .get(start as usize..)
            .ok_or("the central directory lies past the end")?;
        Self::parse(bytes, start)
    }

    /// The byte ranges holding the `assets/` entries whose path below `assets/` is picked, each
    /// from its local header up to the next entry, merged where they touch.
    pub fn spans(&self, mut pick: impl FnMut(&str) -> bool) -> Vec<Range<u64>> {
        let mut starts: Vec<(u64, bool)> = self
            .entries
            .iter()
            .map(|entry| (entry.offset, asset_name(&entry.name).is_some_and(&mut pick)))
            .collect();
        starts.sort_unstable();
        let mut spans: Vec<Range<u64>> = Vec::new();
        for (index, &(start, picked)) in starts.iter().enumerate() {
            if !picked {
                continue;
            }
            let end = starts.get(index + 1).map_or(self.start, |&(next, _)| next);
            match spans.last_mut() {
                Some(last) if last.end == start => last.end = end,
                _ => spans.push(start..end),
            }
        }
        spans
    }

    /// The picked `assets/` files, keyed by their path below `assets/` and checked against
    /// their CRC-32. `chunks` are `(offset, bytes)` pieces of the zip that each hold whole
    /// entries; nothing outside them is read.
    pub fn unpack(
        &self,
        chunks: &[(u64, &[u8])],
        mut pick: impl FnMut(&str) -> bool,
    ) -> Result<Files, String> {
        let mut files = Vec::new();
        for entry in &self.entries {
            let Some(name) = asset_name(&entry.name) else {
                continue;
            };
            if !pick(name) {
                continue;
            }
            let (start, chunk) = chunks
                .iter()
                .find(|(start, chunk)| {
                    (*start..*start + chunk.len() as u64).contains(&entry.offset)
                })
                .ok_or_else(|| format!("{} is not in the bytes held", entry.name))?;
            let local = &chunk[(entry.offset - start) as usize..];
            files.push((name.to_owned(), inflate(entry, local)?));
        }
        Ok(files)
    }
}

fn asset_name(name: &str) -> Option<&str> {
    name.strip_prefix("assets/")
        .filter(|name| !name.is_empty() && !name.ends_with('/'))
}

fn inflate(entry: &Entry, local: &[u8]) -> Result<Vec<u8>, String> {
    if local.len() < 30 || u32_at(local, 0) != LOCAL_HEADER {
        return Err(format!("{} has no local header", entry.name));
    }
    let data = 30 + u16_at(local, 26) as usize + u16_at(local, 28) as usize;
    let data = local
        .get(data..data + entry.compressed as usize)
        .ok_or_else(|| format!("{} is truncated", entry.name))?;
    let mut bytes = Vec::with_capacity(entry.size as usize);
    match entry.method {
        0 => bytes.extend_from_slice(data),
        8 => {
            flate2::read::DeflateDecoder::new(data)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("{}: {error}", entry.name))?;
        }
        method => return Err(format!("{} uses compression method {method}", entry.name)),
    }
    let mut crc = flate2::Crc::new();
    crc.update(&bytes);
    if crc.sum() != entry.crc32 || bytes.len() as u64 != entry.size {
        return Err(format!("{} fails its CRC-32", entry.name));
    }
    Ok(bytes)
}

/// The picked `assets/` files of a whole jar whose SHA-1 already matched.
pub fn entries(jar: &[u8], pick: impl FnMut(&str) -> bool) -> Files {
    const CORRUPT: &str = "a jar that matched its pinned SHA-1 is a readable zip";
    Directory::of(jar)
        .and_then(|directory| directory.unpack(&[(0, jar)], pick))
        .expect(CORRUPT)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Cursor, Write};

    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    use super::*;

    /// A small jar laid out like the client's: classes, then assets, then more classes.
    pub fn sample_jar() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let noise = |seed: u32, len: usize| -> Vec<u8> {
            (0..len as u32)
                .map(|i| (i.wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8)
                .collect()
        };
        let mut file = |name: &str, options, bytes: &[u8]| {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        };
        file("net/minecraft/Main.class", deflated, &noise(1, 200_000));
        file(
            "assets/minecraft/models/block/stone.json",
            deflated,
            &noise(2, 30_000),
        );
        file(
            "assets/minecraft/textures/block/stone.png",
            deflated,
            &noise(3, 40_000),
        );
        file(
            "assets/minecraft/font/default.json",
            stored,
            br#"{"providers":[{"type":"reference","id":"minecraft:include/default"}]}"#,
        );
        file(
            "assets/minecraft/font/include/default.json",
            stored,
            br#"{"providers":[{"type":"bitmap","file":"minecraft:font/ascii.png","ascent":7,"chars":["a"]},{"type":"space","advances":{" ":4}}]}"#,
        );
        file("data/minecraft/textures/fake.png", deflated, b"data");
        file("net/minecraft/Other.class", deflated, &noise(4, 150_000));
        file(
            "assets/minecraft/textures/font/ascii.png",
            deflated,
            &noise(5, 5_000),
        );
        file(
            "assets/minecraft/textures/font/unused.png",
            deflated,
            &noise(6, 5_000),
        );
        file("assets/minecraft/lang/en_us.json", stored, br#"{"a":"b"}"#);
        drop(file);
        zip.add_directory("assets/minecraft/textures/", deflated)
            .unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn entries_keeps_only_picked_files_under_assets() {
        let kept = entries(&sample_jar(), |name| {
            name.starts_with("minecraft/textures/")
        });

        let names: Vec<&str> = kept.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "minecraft/textures/block/stone.png",
                "minecraft/textures/font/ascii.png",
                "minecraft/textures/font/unused.png"
            ]
        );
        assert_eq!(kept[0].1.len(), 40_000);
    }

    #[test]
    fn asset_spans_alone_are_enough_to_unpack_the_assets() {
        let jar = sample_jar();
        let directory = Directory::of(&jar).unwrap();
        let spans = directory.spans(|_| true);
        assert_eq!(spans.len(), 2);

        let chunks: Vec<(u64, &[u8])> = spans
            .iter()
            .map(|span| (span.start, &jar[span.start as usize..span.end as usize]))
            .collect();

        assert_eq!(
            directory.unpack(&chunks, |_| true).unwrap(),
            entries(&jar, |_| true)
        );
    }

    #[test]
    fn a_corrupted_entry_fails_its_crc() {
        let mut jar = sample_jar();
        let directory = Directory::of(&jar).unwrap();
        let lang = directory
            .entries
            .iter()
            .find(|entry| entry.name.ends_with("en_us.json"))
            .unwrap();
        let local = lang.offset as usize;
        let data = local + 30 + lang.name.len() + u16_at(&jar, local + 28) as usize;
        jar[data] ^= 0xff;

        let error = directory.unpack(&[(0, &jar)], |_| true).unwrap_err();

        assert!(error.contains("CRC-32"), "{error}");
    }

    #[test]
    fn verify_checks_length_and_digest() {
        let bytes = b"client";
        let sha1 = sha1_hex(bytes);
        let artifact = Artifact {
            url: "",
            sha1: &sha1,
            size: 6,
        };
        assert!(verify(&artifact, bytes));
        assert!(!verify(&artifact, b"clienT"));
        assert!(!verify(
            &Artifact {
                size: 7,
                ..artifact
            },
            bytes
        ));
    }
}
