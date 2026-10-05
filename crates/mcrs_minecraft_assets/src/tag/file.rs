use crate::asset::read_all;
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_registry::tags::TagFile as TagFileJson;
use serde::{Deserialize, Serialize};

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
        let bytes = read_all(reader).await?;
        let raw: TagFileJson = serde_json::from_slice(&bytes)?;

        let seg = &settings.registry_segment;
        let values = raw
            .values
            .into_iter()
            .map(|entry| {
                if entry.tag {
                    let loc = &entry.id;
                    let path = format!("{}/tags/{}/{}.json", loc.namespace(), seg, loc.path());
                    let s = settings.clone();
                    let handle = load_context
                        .load_builder()
                        .with_settings::<TagFileSettings>(move |out| *out = s.clone())
                        .load::<TagFile>(path);
                    if entry.required {
                        TagEntry::Tag(handle)
                    } else {
                        TagEntry::OptionalTag(handle)
                    }
                } else if entry.required {
                    TagEntry::Element(entry.id)
                } else {
                    TagEntry::OptionalElement(entry.id)
                }
            })
            .collect();

        Ok(TagFile {
            replace: raw.replace,
            values,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
