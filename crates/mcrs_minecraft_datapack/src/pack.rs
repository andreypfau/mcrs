use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_core::{ResourceLocation, rl};

use crate::filter::Filter;

/// The id of the pack the game jar's data pack is loaded as.
pub const VANILLA_PACK: &str = "vanilla";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackKind {
    Vanilla,
    Feature,
    Directory,
    Zip,
}

#[derive(Debug, Clone)]
pub struct AvailablePack {
    pub(crate) id: String,
    pub(crate) kind: PackKind,
    pub(crate) metadata: Option<Vec<u8>>,
}

impl AvailablePack {
    pub fn vanilla() -> Self {
        Self {
            id: VANILLA_PACK.to_owned(),
            kind: PackKind::Vanilla,
            metadata: None,
        }
    }

    pub fn feature(id: impl Into<String>, metadata: impl Into<Vec<u8>>) -> Self {
        Self {
            id: id.into(),
            kind: PackKind::Feature,
            metadata: Some(metadata.into()),
        }
    }

    pub fn directory(file_name: &str, metadata: impl Into<Vec<u8>>) -> Self {
        Self {
            id: format!("file/{file_name}"),
            kind: PackKind::Directory,
            metadata: Some(metadata.into()),
        }
    }

    pub fn zip(file_name: &str, metadata: Option<Vec<u8>>) -> Self {
        Self {
            id: format!("file/{file_name}"),
            kind: PackKind::Zip,
            metadata,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackSelection {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

impl Default for PackSelection {
    fn default() -> Self {
        Self {
            enabled: vec![VANILLA_PACK.to_owned()],
            disabled: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Features {
    pub known: Vec<ResourceLocation>,
    pub vanilla: Vec<ResourceLocation>,
    pub world: Option<Vec<ResourceLocation>>,
}

impl Features {
    /// The vanilla feature alone, known and enabled by the vanilla pack, in a world that does not
    /// restrict its features.
    pub fn vanilla() -> Self {
        let vanilla: ResourceLocation = rl!("minecraft:vanilla").into();
        Self {
            known: vec![vanilla.clone()],
            vanilla: vec![vanilla],
            world: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

/// The position of a pack in its stack; a higher one overrides a lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackIndex(u16);

impl PackIndex {
    pub const fn new(number: u16) -> Self {
        Self(number)
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Debug)]
pub struct StackedPack {
    pub(crate) id: String,
    pub(crate) kind: PackKind,
    pub(crate) known_pack: Option<KnownPack>,
    pub(crate) features: Vec<ResourceLocation>,
    pub(crate) overlays: Vec<String>,
    pub(crate) filter: Option<Filter>,
}

impl StackedPack {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_zip(&self) -> bool {
        self.kind == PackKind::Zip
    }

    pub fn known_pack(&self) -> Option<&KnownPack> {
        self.known_pack.as_ref()
    }

    pub fn features(&self) -> &[ResourceLocation] {
        &self.features
    }

    pub fn overlays(&self) -> &[String] {
        &self.overlays
    }
}

#[derive(Debug, Default)]
pub struct PackStack {
    pub(crate) packs: Vec<StackedPack>,
    pub(crate) enabled_features: Vec<ResourceLocation>,
}

/// A file a pack contributes under `data/<namespace>/`, after its overlays are applied.
pub struct PackFile<'f> {
    pub pack: PackIndex,
    pub namespace: &'f str,
    /// The path below the namespace directory.
    pub relative: &'f str,
    /// The path in the pack, overlay directory included.
    pub path: &'f str,
    pub bytes: &'f [u8],
}

impl PackStack {
    pub fn packs(&self) -> &[StackedPack] {
        &self.packs
    }

    pub fn get(&self, index: PackIndex) -> Option<&StackedPack> {
        self.packs.get(index.index())
    }

    pub fn position(&self, id: &str) -> Option<PackIndex> {
        self.indexed()
            .find(|(_, pack)| pack.id == id)
            .map(|(index, _)| index)
    }

    pub fn indexed(&self) -> impl Iterator<Item = (PackIndex, &StackedPack)> {
        (0..=u16::MAX).map(PackIndex::new).zip(self.packs.iter())
    }

    pub fn known_packs(&self) -> impl Iterator<Item = &KnownPack> {
        self.packs.iter().filter_map(StackedPack::known_pack)
    }

    pub fn enabled_features(&self) -> &[ResourceLocation] {
        &self.enabled_features
    }

    /// Every file of every pack, from the bottom of the stack up. Within one pack the file of a
    /// later overlay replaces the file of the same path below it.
    pub fn files<'f>(&self, files: &'f PackFiles) -> Vec<PackFile<'f>> {
        let mut out = Vec::new();
        for (pack_index, pack) in self.indexed() {
            let Some(contents) = files.packs.get(pack.id()) else {
                continue;
            };
            let mut merged: BTreeMap<(&'f str, &'f str), (&'f str, &'f [u8])> = BTreeMap::new();
            let roots = std::iter::once("data/".to_owned())
                .chain(pack.overlays.iter().map(|o| format!("{o}/data/")));
            for root in roots {
                for (path, bytes) in contents.range(root.clone()..) {
                    let Some(rest) = path.strip_prefix(root.as_str()) else {
                        break;
                    };
                    if let Some((namespace, relative)) = rest.split_once('/') {
                        merged.insert((namespace, relative), (path.as_str(), bytes.as_slice()));
                    }
                }
            }
            out.extend(
                merged
                    .into_iter()
                    .map(|((namespace, relative), (path, bytes))| PackFile {
                        pack: pack_index,
                        namespace,
                        relative,
                        path,
                        bytes,
                    }),
            );
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackFiles {
    packs: HashMap<String, BTreeMap<String, Vec<u8>>>,
}

impl PackFiles {
    pub fn insert(&mut self, pack: &str, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.packs
            .entry(pack.to_owned())
            .or_default()
            .insert(path.into(), bytes.into());
    }
}
