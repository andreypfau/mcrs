use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::block::Block;
use crate::value::IntValueProvider;
use crate::ResourceLocation;
use mcrs_core::tag::tag_ref::TagRef;

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
    pub monster_spawn_light_level: IntValueProvider,
    #[serde(default)]
    pub skybox: Skybox,
    #[serde(default)]
    pub cardinal_light: CardinalLight,
    #[serde(default)]
    pub has_fixed_time: Option<bool>,
    #[serde(default)]
    pub attributes: Option<Value>,
    #[serde(default)]
    pub timelines: Option<Value>,
    #[serde(default)]
    pub default_clock: Option<String>,
}

/// Error when converting a [`ProtoDimensionType`] to [`DimensionType`].
#[derive(Debug, thiserror::Error)]
pub enum DimensionTypeResolveError {
    #[error("infiniburn field `{0}` does not start with '#'")]
    MissingHashPrefix(String),
    #[error("invalid resource location in infiniburn: {0}")]
    InvalidResourceLocation(#[from] mcrs_core::resource_location::ResourceLocationError),
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
            timelines: self.timelines,
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
    pub monster_spawn_light_level: IntValueProvider,
    pub skybox: Skybox,
    pub cardinal_light: CardinalLight,
    pub has_fixed_time: Option<bool>,
    pub attributes: Option<Value>,
    pub timelines: Option<Value>,
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
    pub monster_spawn_light_level: IntValueProvider,
    pub skybox: Skybox,
    pub cardinal_light: CardinalLight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_fixed_time: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timelines: Option<Value>,
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
            skybox: dt.skybox.clone(),
            cardinal_light: dt.cardinal_light.clone(),
            has_fixed_time: dt.has_fixed_time,
            attributes: dt.attributes.clone(),
            timelines: dt.timelines.clone(),
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
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let proto: ProtoDimensionType = serde_json::from_slice(&bytes)?;
        Ok(proto.resolve(load_context)?)
    }

    fn extensions(&self) -> &[&str] {
        &[] // no extension claim — always use typed load::<DimensionType>()
    }
}

// ── Supporting enums ──

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
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
