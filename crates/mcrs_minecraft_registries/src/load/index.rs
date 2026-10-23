use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{
    PackDiagnostic, PackFile, PackFiles, PackIndex, PackStack, Visibility,
};

use super::declaration::{EntrySource, Source};
use crate::diagnostics::Diagnostic;

pub(crate) struct Located<'f> {
    pub(crate) pack: PackIndex,
    pub(crate) path: &'f str,
    pub(crate) bytes: &'f [u8],
}

#[derive(Default)]
pub(crate) struct SlotFiles<'f> {
    pub(crate) entries: BTreeMap<ResourceLocation, EntrySource<'f>>,
    pub(crate) tags: BTreeMap<ResourceLocation, Vec<Located<'f>>>,
}

#[derive(Default)]
pub(crate) struct Directories {
    by_path: HashMap<String, (usize, bool)>,
}

impl Directories {
    pub(crate) fn insert(&mut self, key: &ResourceLocation, slot: usize, has_entries: bool) {
        self.by_path
            .insert(key.path().to_owned(), (slot, has_entries));
    }

    fn find<'p>(&self, path: &'p str, tag: bool) -> Option<(usize, &'p str)> {
        path.match_indices('/').rev().find_map(|(at, _)| {
            let (directory, rest) = path.split_at_checked(at)?;
            match self.by_path.get(directory) {
                Some(&(slot, has_entries)) if tag || has_entries => {
                    Some((slot, rest.strip_prefix('/')?))
                }
                _ => None,
            }
        })
    }
}

pub(crate) struct Candidate<'f> {
    pack: PackIndex,
    namespace: &'f str,
    relative: &'f str,
    slot: usize,
    tag: bool,
    name: ResourceLocation,
    path: &'f str,
    bytes: &'f [u8],
}

impl<'f> Candidate<'f> {
    pub(crate) fn tested(&self) -> (PackIndex, &'f str, &'f str) {
        (self.pack, self.namespace, self.relative)
    }
}

pub(crate) fn assign<'f>(
    candidates: Vec<Candidate<'f>>,
    visibility: &Visibility<'_>,
    slots: usize,
) -> Vec<SlotFiles<'f>> {
    let mut out: Vec<SlotFiles<'f>> = (0..slots).map(|_| SlotFiles::default()).collect();
    for candidate in candidates {
        if visibility.hidden(candidate.pack, candidate.namespace, candidate.relative) {
            continue;
        }
        let Some(slot) = out.get_mut(candidate.slot) else {
            continue;
        };
        if candidate.tag {
            slot.tags.entry(candidate.name).or_default().push(Located {
                pack: candidate.pack,
                path: candidate.path,
                bytes: candidate.bytes,
            });
        } else {
            slot.entries.insert(
                candidate.name,
                EntrySource {
                    pack: candidate.pack,
                    source: Source::File {
                        path: candidate.path,
                        bytes: candidate.bytes,
                    },
                },
            );
        }
    }
    out
}

pub(crate) fn candidates<'f>(
    stack: &PackStack,
    files: &'f PackFiles,
    directories: &Directories,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Candidate<'f>> {
    let mut out = Vec::new();
    for file in stack.files(files) {
        let PackFile {
            pack,
            namespace,
            relative,
            path,
            bytes,
        } = file;
        let (tag, inside) = match relative.strip_prefix("tags/") {
            Some(inside) => (true, inside),
            None => (false, relative),
        };
        let Some((slot, file)) = directories.find(inside, tag) else {
            continue;
        };
        let name = match file.strip_suffix(".json") {
            Some(stem) => ResourceLocation::new(namespace, stem).ok(),
            None if ResourceLocation::new(namespace, relative).is_ok() => continue,
            None => None,
        };
        let Some(name) = name else {
            diagnostics.push(Diagnostic::Pack(PackDiagnostic::InvalidFileName {
                pack: stack.get(pack).map_or("", |pack| pack.id()).to_owned(),
                zip: stack.get(pack).is_some_and(|pack| pack.is_zip()),
                namespace: namespace.to_owned(),
                path: relative.to_owned(),
            }));
            continue;
        };
        out.push(Candidate {
            pack,
            namespace,
            relative,
            slot,
            tag,
            name,
            path,
            bytes,
        });
    }
    out
}
