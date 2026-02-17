use crate::resource_location::ResourceLocation;
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

/// A single entry in a Minecraft tag file.
#[derive(Debug)]
pub enum TagEntry {
    /// A required element reference (`"namespace:path"`).
    Element(ResourceLocation),
    /// An optional element reference — silently ignored if the element doesn't exist.
    OptionalElement(ResourceLocation),
    /// A required nested tag reference (`"#namespace:path"`).
    Tag(Handle<TagFile>),
    /// An optional nested tag reference — silently ignored if the tag file doesn't exist.
    OptionalTag(Handle<TagFile>),
}

/// A Minecraft tag file asset (e.g. `minecraft/tags/block/mineable/pickaxe.json`).
///
/// `VisitAssetDependencies` is implemented manually because the `Handle<TagFile>`
/// values are inside enum variants and cannot be auto-detected by the derive macro.
#[derive(Debug, TypePath)]
pub struct TagFile {
    pub replace: bool,
    pub values: Vec<TagEntry>,
}

impl Asset for TagFile {}

impl VisitAssetDependencies for TagFile {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        for entry in &self.values {
            match entry {
                TagEntry::Tag(h) | TagEntry::OptionalTag(h) => visit(h.id().into()),
                _ => {}
            }
        }
    }
}

// ─── Serialized JSON forms ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SerializedTagFile {
    values: Vec<SerializedTagEntry>,
    #[serde(default)]
    replace: bool,
}

/// An entry in the JSON tag file.  Two forms are supported:
///
/// Short:  `"minecraft:stone"` or `"#minecraft:base_stone_overworld"`
/// Full:   `{"id": "minecraft:stone", "required": false}`
#[derive(Debug, Clone, PartialEq, Eq)]
struct SerializedTagEntry {
    id: TagOrElementLocation,
    required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TagOrElementLocation {
    loc: ResourceLocation,
    is_tag: bool,
}

impl FromStr for TagOrElementLocation {
    type Err = crate::resource_location::ResourceLocationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix('#') {
            Ok(TagOrElementLocation { loc: ResourceLocation::from_str(rest)?, is_tag: true })
        } else {
            Ok(TagOrElementLocation { loc: ResourceLocation::from_str(s)?, is_tag: false })
        }
    }
}

impl<'de> Deserialize<'de> for TagOrElementLocation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        TagOrElementLocation::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for TagOrElementLocation {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.is_tag {
            s.serialize_str(&format!("#{}", self.loc.as_str()))
        } else {
            s.serialize_str(self.loc.as_str())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TagEntryRepr {
    Short(TagOrElementLocation),
    Full(TagEntryFull),
}

#[derive(Debug, Deserialize)]
struct TagEntryFull {
    id: TagOrElementLocation,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

impl<'de> Deserialize<'de> for SerializedTagEntry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match TagEntryRepr::deserialize(d)? {
            TagEntryRepr::Short(loc) => Ok(SerializedTagEntry { id: loc, required: true }),
            TagEntryRepr::Full(f) => Ok(SerializedTagEntry { id: f.id, required: f.required }),
        }
    }
}

// ─── Loader ──────────────────────────────────────────────────────────────────

/// Settings passed to `TagFileLoader` to control nested-tag path construction.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TagFileSettings {
    /// The registry segment used when resolving nested `#tag` references.
    ///
    /// e.g. `"block"` → nested tag `#minecraft:mineable/pickaxe` is loaded from
    /// `minecraft/tags/block/mineable/pickaxe.json`
    pub registry_segment: String,
}

/// Bevy `AssetLoader` for Minecraft JSON tag files.
#[derive(Default, TypePath)]
pub struct TagFileLoader;

#[derive(Debug, thiserror::Error)]
pub enum TagFileLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for TagFileLoader {
    type Asset = TagFile;
    type Settings = TagFileSettings;
    type Error = TagFileLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &TagFileSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<TagFile, TagFileLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let raw: SerializedTagFile = serde_json::from_slice(&bytes)?;

        let seg = &settings.registry_segment;
        let values = raw
            .values
            .into_iter()
            .map(|entry| {
                if entry.id.is_tag {
                    let loc = &entry.id.loc;
                    let path = format!(
                        "{}/tags/{}/{}.json",
                        loc.namespace(),
                        seg,
                        loc.path()
                    );
                    let s = settings.clone();
                    let handle = load_context
                        .loader()
                        .with_settings::<TagFileSettings>(move |out| *out = s.clone())
                        .load::<TagFile>(path);
                    if entry.required {
                        TagEntry::Tag(handle)
                    } else {
                        TagEntry::OptionalTag(handle)
                    }
                } else if entry.required {
                    TagEntry::Element(entry.id.loc)
                } else {
                    TagEntry::OptionalElement(entry.id.loc)
                }
            })
            .collect();

        Ok(TagFile { replace: raw.replace, values })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
