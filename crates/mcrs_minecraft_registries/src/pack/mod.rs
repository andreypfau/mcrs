mod filter;
mod metadata;
mod stack;

use std::collections::{BTreeMap, HashMap};

use mcrs_minecraft_core::ResourceLocation;

pub(crate) use filter::Filter;
pub use stack::pack_stack;

pub(crate) const VANILLA: &str = "vanilla";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackKind {
    Vanilla,
    Feature,
    Directory,
    Zip,
}

#[derive(Debug, Clone)]
pub struct AvailablePack {
    id: String,
    kind: PackKind,
    metadata: Option<Vec<u8>>,
}

impl AvailablePack {
    pub fn vanilla() -> Self {
        Self {
            id: VANILLA.to_owned(),
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
            enabled: vec![VANILLA.to_owned()],
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackIndex(u16);

impl PackIndex {
    pub(crate) fn new(number: u16) -> Self {
        Self(number)
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Debug)]
pub struct StackedPack {
    id: String,
    kind: PackKind,
    known_pack: Option<KnownPack>,
    features: Vec<ResourceLocation>,
    overlays: Vec<String>,
    filter: Option<Filter>,
}

impl StackedPack {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn kind(&self) -> PackKind {
        self.kind
    }

    pub fn known_pack(&self) -> Option<&KnownPack> {
        self.known_pack.as_ref()
    }

    pub fn features(&self) -> &[ResourceLocation] {
        &self.features
    }

    pub(crate) fn overlays(&self) -> &[String] {
        &self.overlays
    }

    pub(crate) fn filter(&self) -> Option<&Filter> {
        self.filter.as_ref()
    }
}

#[derive(Debug)]
pub struct PackStack {
    packs: Vec<StackedPack>,
    enabled_features: Vec<ResourceLocation>,
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

    pub(crate) fn indexed(&self) -> impl Iterator<Item = (PackIndex, &StackedPack)> {
        (0..=u16::MAX).map(PackIndex::new).zip(self.packs.iter())
    }

    pub fn known_packs(&self) -> impl Iterator<Item = &KnownPack> {
        self.packs.iter().filter_map(StackedPack::known_pack)
    }

    pub fn enabled_features(&self) -> &[ResourceLocation] {
        &self.enabled_features
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

    pub(crate) fn of(&self, pack: &str) -> Option<&BTreeMap<String, Vec<u8>>> {
        self.packs.get(pack)
    }
}
