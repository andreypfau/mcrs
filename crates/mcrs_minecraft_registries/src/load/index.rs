use std::collections::{BTreeMap, HashMap, HashSet};

use mcrs_minecraft_core::ResourceLocation;

use super::declaration::{EntrySource, Source};
use crate::diagnostics::Diagnostic;
use crate::pack::{PackFiles, PackIndex, PackKind, PackStack};

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
    pub(crate) fn directory(key: &ResourceLocation) -> String {
        match key.namespace() {
            "minecraft" => key.path().to_owned(),
            namespace => format!("{namespace}/{}", key.path()),
        }
    }

    pub(crate) fn insert(&mut self, key: &ResourceLocation, slot: usize, has_entries: bool) {
        self.by_path
            .insert(Self::directory(key), (slot, has_entries));
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

pub(crate) struct Visibility {
    hidden: HashSet<(PackIndex, String)>,
}

fn visibility_key(pack: PackIndex, namespace: &str, relative: &str) -> (PackIndex, String) {
    (pack, format!("{namespace}/{relative}"))
}

impl Visibility {
    pub(crate) fn new(
        stack: &PackStack,
        tested: &[(PackIndex, &str, &str)],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let mut hidden = HashSet::new();
        for (above, pack) in stack.packs().iter().enumerate() {
            let Some(filter) = pack.filter() else {
                continue;
            };
            let below = tested.iter().filter(|(at, _, _)| at.index() < above);
            let hides: Result<Vec<_>, String> = below
                .filter_map(
                    |&(at, namespace, relative)| match filter.hides(namespace, relative) {
                        Ok(true) => Some(Ok(visibility_key(at, namespace, relative))),
                        Ok(false) => None,
                        Err(reason) => Some(Err(reason)),
                    },
                )
                .collect();
            match hides {
                Ok(keys) => hidden.extend(keys),
                Err(reason) => diagnostics.push(Diagnostic::FilterIgnored {
                    pack: pack.id().to_owned(),
                    reason,
                }),
            }
        }
        Self { hidden }
    }

    pub(crate) fn hidden(&self, pack: PackIndex, namespace: &str, relative: &str) -> bool {
        self.hidden
            .contains(&visibility_key(pack, namespace, relative))
    }
}

impl Candidate<'_> {
    pub(crate) fn tested(&self) -> (PackIndex, &str, &str) {
        (self.pack, self.namespace, self.relative)
    }
}

fn roots(overlays: &[String]) -> impl Iterator<Item = String> + '_ {
    std::iter::once("data/".to_owned()).chain(overlays.iter().map(|o| format!("{o}/data/")))
}

pub(crate) fn assign<'f>(
    candidates: Vec<Candidate<'f>>,
    visibility: &Visibility,
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
    for (pack_index, pack) in stack.indexed() {
        let Some(contents) = files.of(pack.id()) else {
            continue;
        };
        let mut merged: BTreeMap<(&'f str, &'f str), (&'f str, &'f [u8])> = BTreeMap::new();
        for root in roots(pack.overlays()) {
            for (path, bytes) in contents.range(root.clone()..) {
                let Some(rest) = path.strip_prefix(root.as_str()) else {
                    break;
                };
                if let Some((namespace, relative)) = rest.split_once('/') {
                    merged.insert((namespace, relative), (path.as_str(), bytes.as_slice()));
                }
            }
        }
        for ((namespace, relative), (path, bytes)) in merged {
            let (tag, inside) = match relative.strip_prefix("tags/") {
                Some(inside) => (true, inside),
                None => (false, relative),
            };
            let Some((slot, file)) = directories.find(inside, tag) else {
                continue;
            };
            let invalid = || Diagnostic::InvalidFileName {
                pack: pack.id().to_owned(),
                zip: pack.kind() == PackKind::Zip,
                namespace: namespace.to_owned(),
                path: relative.to_owned(),
            };
            if ResourceLocation::new(namespace, relative).is_err() {
                diagnostics.push(invalid());
                continue;
            }
            let Some(stem) = file.strip_suffix(".json") else {
                continue;
            };
            let Ok(name) = ResourceLocation::new(namespace, stem) else {
                diagnostics.push(invalid());
                continue;
            };
            out.push(Candidate {
                pack: pack_index,
                namespace,
                relative,
                slot,
                tag,
                name,
                path,
                bytes,
            });
        }
    }
    out
}
