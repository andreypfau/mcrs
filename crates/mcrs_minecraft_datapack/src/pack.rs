use std::any::Any;
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

/// What a pack holds at a path: the bytes of a file, or a ready value in place of one.
pub enum PackContent<'f> {
    Bytes(&'f [u8]),
    Value(Box<dyn Any + Send + Sync>),
}

/// A file or a value a pack contributes under `data/<namespace>/`, after its overlays are
/// applied.
pub struct PackFile<'f> {
    pub pack: PackIndex,
    pub namespace: &'f str,
    /// The path below the namespace directory.
    pub relative: &'f str,
    /// The path in the pack, overlay directory included.
    pub path: &'f str,
    pub content: PackContent<'f>,
}

impl PackStack {
    pub fn packs(&self) -> &[StackedPack] {
        &self.packs
    }

    pub fn get(&self, index: PackIndex) -> Option<&StackedPack> {
        self.packs.get(index.index())
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

    /// Every file and value of every pack, from the bottom of the stack up. Within one pack the
    /// content of a later overlay replaces the content of the same path below it. Each value of a
    /// listed pack moves out of `files`, which keeps a unit value in its place.
    pub fn files<'f>(&self, files: &'f mut PackFiles) -> Vec<PackFile<'f>> {
        let mut by_pack: HashMap<&str, &'f mut BTreeMap<String, Content>> = files
            .packs
            .iter_mut()
            .map(|(pack, contents)| (pack.as_str(), contents))
            .collect();
        let mut out = Vec::new();
        for (pack_index, pack) in self.indexed() {
            let Some(contents) = by_pack.remove(pack.id()) else {
                continue;
            };
            let roots: Vec<String> = std::iter::once("data/".to_owned())
                .chain(pack.overlays.iter().map(|o| format!("{o}/data/")))
                .collect();
            let mut merged = Merged::new();
            for (path, content) in contents.iter_mut() {
                let mut under = roots.iter().enumerate().filter_map(|(rank, root)| {
                    let (namespace, relative) =
                        path.strip_prefix(root.as_str())?.split_once('/')?;
                    Some((rank, (namespace, relative)))
                });
                match content {
                    Content::Bytes(bytes) => {
                        let bytes: &'f [u8] = bytes;
                        for (rank, key) in under {
                            keep(&mut merged, key, (rank, path, PackContent::Bytes(bytes)));
                        }
                    }
                    Content::Value(value) => {
                        if let Some((rank, key)) = under.next_back() {
                            // A unit box does not allocate.
                            let value = std::mem::replace(value, Box::new(()));
                            keep(&mut merged, key, (rank, path, PackContent::Value(value)));
                        }
                    }
                }
            }
            out.extend(
                merged
                    .into_iter()
                    .map(|((namespace, relative), (_, path, content))| PackFile {
                        pack: pack_index,
                        namespace,
                        relative,
                        path,
                        content,
                    }),
            );
        }
        out
    }
}

/// The content each `(namespace, relative)` path of one pack lists, with the rank of the root it
/// lies below: `data/` first, then each overlay.
type Merged<'f> = BTreeMap<(&'f str, &'f str), (usize, &'f str, PackContent<'f>)>;

fn keep<'f>(
    merged: &mut Merged<'f>,
    key: (&'f str, &'f str),
    listed: (usize, &'f str, PackContent<'f>),
) {
    if merged.get(&key).is_none_or(|(rank, ..)| *rank < listed.0) {
        merged.insert(key, listed);
    }
}

enum Content {
    Bytes(Vec<u8>),
    Value(Box<dyn Any + Send + Sync>),
}

/// The files and ready values of each pack, by their path in the pack. A value replaces the file
/// at the same path, whichever of the two is inserted first.
#[derive(Default)]
pub struct PackFiles {
    packs: HashMap<String, BTreeMap<String, Content>>,
}

impl PackFiles {
    pub fn insert(&mut self, pack: &str, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        let contents = self.packs.entry(pack.to_owned()).or_default();
        let content = contents
            .entry(path.into())
            .or_insert(Content::Bytes(Vec::new()));
        if let Content::Bytes(held) = content {
            *held = bytes.into();
        }
    }

    pub fn insert_value(
        &mut self,
        pack: &str,
        path: impl Into<String>,
        value: Box<dyn Any + Send + Sync>,
    ) {
        self.packs
            .entry(pack.to_owned())
            .or_default()
            .insert(path.into(), Content::Value(value));
    }

    pub(crate) fn holds_value(&self, pack: &str, path: &str) -> bool {
        let content = self.packs.get(pack).and_then(|contents| contents.get(path));
        matches!(content, Some(Content::Value(_)))
    }
}
