use std::collections::BTreeSet;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{Directory, Entry, Files, Release, merge};

pub(crate) fn is_font_definition(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|(_, rest)| rest.starts_with("font/"))
}

pub(crate) fn is_texture(name: &str) -> bool {
    name.split_once('/')
        .is_some_and(|(_, rest)| rest.starts_with("textures/"))
}

#[derive(Deserialize)]
struct FontDefinition {
    providers: Vec<Provider>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Provider {
    Bitmap {
        file: String,
    },
    #[serde(other)]
    Other,
}

/// The textures the bitmap providers of these font definitions draw from, as paths below
/// `assets/`.
pub(crate) fn font_textures(definitions: &Files) -> BTreeSet<String> {
    let mut textures = BTreeSet::new();
    for (name, bytes) in definitions {
        if !name.ends_with(".json") {
            continue;
        }
        match serde_json::from_slice::<FontDefinition>(bytes) {
            Ok(definition) => {
                for provider in definition.providers {
                    if let Provider::Bitmap { file } = provider {
                        let (namespace, path) =
                            file.split_once(':').unwrap_or(("minecraft", &file));
                        textures.insert(format!("{namespace}/textures/{path}"));
                    }
                }
            }
            Err(error) => tracing::warn!("{name} is not a font definition: {error}"),
        }
    }
    textures
}

pub fn font_files(directory: &Directory, chunks: &[(u64, &[u8])]) -> Result<Files, String> {
    let mut files = directory.unpack(chunks, is_font_definition)?;
    let textures = font_textures(&files);
    files.extend(directory.unpack(chunks, |name| textures.contains(name))?);
    Ok(files)
}

/// Where the font files of one jar lie, so they can be fetched before its central directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct FontHint {
    pub jar_sha1: String,
    pub fonts: Vec<HintedFont>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct HintedFont {
    pub name: String,
    pub offset: u64,
    pub span: u64,
    pub method: u16,
    pub crc32: u32,
    pub compressed: u64,
    pub size: u64,
}

impl FontHint {
    pub fn of(jar: &[u8], jar_sha1: &str) -> Result<Self, String> {
        let directory = Directory::of(jar)?;
        let files = font_files(&directory, &[(0, jar)])?;
        let fonts = files
            .iter()
            .map(|(name, _)| {
                let entry = directory
                    .entry(&format!("assets/{name}"))
                    .expect("an unpacked file has an entry");
                let extent = directory.extent(entry);
                HintedFont {
                    name: entry.name.clone(),
                    offset: entry.offset,
                    span: extent.end - extent.start,
                    method: entry.method,
                    crc32: entry.crc32,
                    compressed: entry.compressed,
                    size: entry.size,
                }
            })
            .collect();
        Ok(Self {
            jar_sha1: jar_sha1.to_owned(),
            fonts,
        })
    }

    pub(crate) fn for_release(release: &Release) -> Option<Self> {
        if release.font_hint.is_empty() {
            return None;
        }
        let entries_end = release.jar.size - release.directory.size;
        match serde_json::from_str::<Self>(release.font_hint) {
            Ok(hint) if hint.jar_sha1 != release.jar.sha1 => {
                tracing::warn!("the font hint describes another jar; ignoring it");
                None
            }
            Ok(hint)
                if hint.fonts.is_empty()
                    || hint
                        .fonts
                        .iter()
                        .any(|font| font.offset.saturating_add(font.span) > entries_end) =>
            {
                tracing::warn!("the font hint points outside the jar's entries; ignoring it");
                None
            }
            Ok(hint) => Some(hint),
            Err(error) => {
                tracing::warn!("the font hint is unreadable, ignoring it: {error}");
                None
            }
        }
    }

    pub(crate) fn spans(&self) -> Vec<Range<u64>> {
        let mut spans: Vec<Range<u64>> = self
            .fonts
            .iter()
            .map(|font| font.offset..font.offset + font.span)
            .collect();
        spans.sort_unstable_by_key(|span| span.start);
        merge(spans)
    }

    pub(crate) fn directory(&self) -> Directory {
        Directory::new(self.fonts.iter().map(HintedFont::entry).collect(), u64::MAX)
    }

    pub(crate) fn agrees_with(&self, directory: &Directory) -> bool {
        self.fonts.iter().all(|font| {
            directory.entry(&font.name).is_some_and(|entry| {
                *entry == font.entry()
                    && directory.extent(entry) == (font.offset..font.offset + font.span)
            })
        })
    }
}

impl HintedFont {
    fn entry(&self) -> Entry {
        Entry {
            name: self.name.clone(),
            offset: self.offset,
            method: self.method,
            crc32: self.crc32,
            compressed: self.compressed,
            size: self.size,
        }
    }
}
