use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_assets::asset::read_all;
use mcrs_minecraft_assets::tag::tag_ref::TagRef;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_environment::attribute::EnvironmentAttributeMap;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_registry::key::Block;

// ── Proto (deserialization-only) ──

/// Raw dimension type as deserialized from JSON.
///
/// `infiniburn` is a raw string like `"#minecraft:infiniburn_overworld"`.
/// Resolved into [`DimensionType`] by the loader.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProtoDimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    #[serde(default)]
    pub has_ender_dragon_fight: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: String,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntProvider,
    #[serde(default)]
    pub skybox: Skybox,
    #[serde(default)]
    pub cardinal_light: CardinalLight,
    #[serde(default)]
    pub has_fixed_time: Option<bool>,
    #[serde(default)]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default)]
    pub timelines: Option<String>,
    #[serde(default)]
    pub default_clock: Option<String>,
}

/// Error when converting a [`ProtoDimensionType`] to [`DimensionType`].
#[derive(Debug, thiserror::Error)]
pub enum DimensionTypeResolveError {
    #[error("tag field `{0}` does not start with '#'")]
    MissingHashPrefix(String),
    #[error("invalid resource location in infiniburn: {0}")]
    InvalidResourceLocation(#[from] mcrs_minecraft_core::resource_location::ResourceLocationError),
}

impl ProtoDimensionType {
    /// Parse the raw `infiniburn` string and load the corresponding block tag
    /// file as a sub-asset.
    pub fn resolve(
        self,
        load_context: &mut LoadContext<'_>,
    ) -> Result<DimensionType, DimensionTypeResolveError> {
        let tag_str = self
            .infiniburn
            .strip_prefix('#')
            .ok_or_else(|| DimensionTypeResolveError::MissingHashPrefix(self.infiniburn.clone()))?;

        let infiniburn = TagRef::<Block>::load(tag_str, load_context)?;

        let timelines = self
            .timelines
            .as_deref()
            .map(|raw| {
                let tag_str = raw
                    .strip_prefix('#')
                    .ok_or_else(|| DimensionTypeResolveError::MissingHashPrefix(raw.to_owned()))?;
                TagRef::<Timeline>::load(tag_str, load_context)
                    .map_err(DimensionTypeResolveError::from)
            })
            .transpose()?;

        Ok(DimensionType {
            has_skylight: self.has_skylight,
            has_ceiling: self.has_ceiling,
            has_ender_dragon_fight: self.has_ender_dragon_fight,
            coordinate_scale: self.coordinate_scale,
            min_y: self.min_y,
            height: self.height,
            logical_height: self.logical_height,
            infiniburn,
            ambient_light: self.ambient_light,
            monster_spawn_block_light_limit: self.monster_spawn_block_light_limit,
            monster_spawn_light_level: self.monster_spawn_light_level,
            skybox: self.skybox,
            cardinal_light: self.cardinal_light,
            has_fixed_time: self.has_fixed_time,
            attributes: self.attributes,
            timelines,
            default_clock: self.default_clock,
        })
    }
}

// ── Runtime DimensionType ──

/// Runtime dimension type with a typed `infiniburn` block tag reference.
///
/// The `infiniburn` field is a [`TagRef<Block>`] — a typed tag key paired with
/// its loaded tag file handle. The tag file is loaded as a sub-asset by
/// `DimensionTypeLoader`, so Bevy's dependency graph ensures it (and any
/// nested tags) are fully loaded before `is_loaded_with_dependencies` returns
/// `true`.
#[derive(Debug, Clone, TypePath)]
pub struct DimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: TagRef<Block>,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntProvider,
    pub skybox: Skybox,
    pub cardinal_light: CardinalLight,
    pub has_fixed_time: Option<bool>,
    pub attributes: EnvironmentAttributeMap,
    pub timelines: Option<TagRef<Timeline>>,
    pub default_clock: Option<String>,
}

impl DimensionType {
    pub fn load(
        ctx: &mut LoadContext<'_>,
        loc: &ResourceLocation<Arc<str>>,
    ) -> Handle<DimensionType> {
        ctx.load(format!(
            "{}/dimension_type/{}.json",
            loc.namespace(),
            loc.path()
        ))
    }
}

impl Asset for DimensionType {}

impl VisitAssetDependencies for DimensionType {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.infiniburn.handle().id().untyped());
        if let Some(timelines) = &self.timelines {
            visit(timelines.handle().id().untyped());
        }
    }
}

/// DimensionType data subset for NETWORK_CODEC.
///
/// The `infiniburn` field is serialized as a string like
/// `"#minecraft:infiniburn_overworld"` (the tag key prefixed with `#`).
#[derive(Debug, Clone, Serialize)]
pub struct NetworkDimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: String,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntProvider,
    pub skybox: Skybox,
    pub cardinal_light: CardinalLight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_fixed_time: Option<bool>,
    #[serde(skip_serializing_if = "EnvironmentAttributeMap::is_empty")]
    pub attributes: EnvironmentAttributeMap,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timelines: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clock: Option<String>,
}

impl From<&DimensionType> for NetworkDimensionType {
    fn from(dt: &DimensionType) -> Self {
        NetworkDimensionType {
            has_skylight: dt.has_skylight,
            has_ceiling: dt.has_ceiling,
            has_ender_dragon_fight: dt.has_ender_dragon_fight,
            coordinate_scale: dt.coordinate_scale,
            min_y: dt.min_y,
            height: dt.height,
            logical_height: dt.logical_height,
            infiniburn: format!("#{}", dt.infiniburn.key().as_str()),
            ambient_light: dt.ambient_light,
            monster_spawn_block_light_limit: dt.monster_spawn_block_light_limit,
            monster_spawn_light_level: dt.monster_spawn_light_level.clone(),
            skybox: dt.skybox,
            cardinal_light: dt.cardinal_light.clone(),
            has_fixed_time: dt.has_fixed_time,
            attributes: dt.attributes.clone(),
            timelines: dt
                .timelines
                .as_ref()
                .map(|tag| format!("#{}", tag.key().as_str())),
            default_clock: dt.default_clock.clone(),
        }
    }
}

// ── Loader ──

/// Bevy `AssetLoader` for dimension type JSON files.
#[derive(Default, TypePath)]
pub struct DimensionTypeLoader;

#[derive(Debug, thiserror::Error)]
pub enum DimensionTypeLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("resolve error: {0}")]
    Resolve(#[from] DimensionTypeResolveError),
}

impl AssetLoader for DimensionTypeLoader {
    type Asset = DimensionType;
    type Settings = ();
    type Error = DimensionTypeLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<DimensionType, DimensionTypeLoaderError> {
        let bytes = read_all(reader).await?;
        let proto: ProtoDimensionType = serde_json::from_slice(&bytes)?;
        Ok(proto.resolve(load_context)?)
    }

    fn extensions(&self) -> &[&str] {
        &[] // no extension claim — always use typed load::<DimensionType>()
    }
}

// ── Supporting enums ──

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skybox {
    #[default]
    #[serde(rename = "overworld")]
    Overworld,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "end")]
    End,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CardinalLight {
    #[default]
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "nether")]
    Nether,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::PathBuf;

    fn dimension_type_dirs() -> Vec<PathBuf> {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets");
        let mut dirs = vec![assets.join("minecraft/dimension_type")];
        for pack in std::fs::read_dir(assets.join(mcrs_minecraft_assets::packs::PACKS_ROOT))
            .into_iter()
            .flatten()
        {
            let dir = pack.unwrap().path().join("minecraft/dimension_type");
            if dir.is_dir() {
                dirs.push(dir);
            }
        }
        dirs
    }

    fn numbers_by_value(
        path: String,
        tag: &mcrs_minecraft_nbt::tag::NbtTag,
        out: &mut Vec<String>,
    ) {
        use mcrs_minecraft_nbt::tag::NbtTag;
        match tag {
            NbtTag::Compound(compound) => {
                for (name, child) in &compound.child_tags {
                    numbers_by_value(format!("{path}.{name}"), child, out);
                }
            }
            NbtTag::List(items) => {
                for (index, item) in items.iter().enumerate() {
                    numbers_by_value(format!("{path}[{index}]"), item, out);
                }
            }
            NbtTag::Short(v) => out.push(format!("{path}={}", *v as f32)),
            NbtTag::Int(v) => out.push(format!("{path}={}", *v as f32)),
            NbtTag::Long(v) => out.push(format!("{path}={}", *v as f32)),
            NbtTag::Float(v) => out.push(format!("{path}={v}")),
            NbtTag::Double(v) => out.push(format!("{path}={}", *v as f32)),
            other => out.push(format!("{path}={other:?}")),
        }
    }

    #[test]
    fn every_dimension_type_parses_through_the_registry() {
        let mut count = 0;
        for entry in dimension_type_dirs().into_iter().flat_map(|dir| {
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        }) {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let raw: Value = serde_json::from_slice(&bytes).unwrap();
            let proto: ProtoDimensionType = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

            assert!(
                !proto.attributes.is_empty(),
                "{} has attributes",
                path.display()
            );
            // Through text: a value tree would widen the `f32` fields to `f64`
            // and print 192.33 as 192.3300018310547.
            let written: Value =
                serde_json::from_str(&serde_json::to_string(&proto.attributes).unwrap()).unwrap();
            assert_eq!(
                written,
                raw["attributes"],
                "{} attributes must round-trip unchanged",
                path.display()
            );
            // What the client actually receives must not drift from the raw
            // JSON the field used to be serialized from, widths and the order
            // of a compound's keys apart: the typed values write the int and
            // float tags the game does.
            let mut typed = Vec::new();
            let mut untyped = Vec::new();
            numbers_by_value(
                String::new(),
                &mcrs_minecraft_nbt::to_nbt_tag(&proto.attributes).unwrap(),
                &mut typed,
            );
            numbers_by_value(
                String::new(),
                &mcrs_minecraft_nbt::to_nbt_tag(&raw["attributes"]).unwrap(),
                &mut untyped,
            );
            typed.sort();
            untyped.sort();
            assert_eq!(
                typed,
                untyped,
                "{} attributes must encode to the same NBT",
                path.display()
            );
            count += 1;
        }
        assert_eq!(count, 5);
    }
}
