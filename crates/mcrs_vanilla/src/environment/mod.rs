//! Composing the environment attribute layers of one dimension.
//!
//! Five layers stack in the order the reference applies them: the registered
//! default, the dimension's constants, the biome at the position, the timeline
//! tracks, and weather. Nothing here memoizes an effective value — a layer
//! stack is derived once from immutable assets and every read composes it
//! afresh.

pub mod sky;
pub mod spatial;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_core::registry::snapshot::rl_from_asset_path;
use mcrs_core::tag::file::TagFile;
use mcrs_core::tag::{DynRegistryIndex, resolve_tag_file_ordered};
use serde_json::json;

use crate::ResourceLocation;
use crate::attribute::{
    AttributeError, AttributeSpec, AttributeValue, ENVIRONMENT_ATTRIBUTES, EnvironmentAttributeMap,
    ModifierError, Operation, apply,
};
use crate::dimension::dimension_type::{DimensionType, Skybox};
use crate::timeline::{AttributeTrackSampler, Timeline, TimelineError};
use crate::world_clock::{ClockTimeMarkers, WorldClocks};

pub use spatial::{
    BiomeAttributes, BiomeAttributeSource, SpatialAttributeInterpolator, UniformBiomes,
};

/// How much it is raining and thundering, in `[0; 1]`.
///
/// Truth until the reader that supplies it from the save lands.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct Weather {
    pub rain: f32,
    pub thunder: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    #[error(transparent)]
    Attribute(#[from] AttributeError),
    #[error(transparent)]
    Modifier(#[from] ModifierError),
    #[error(transparent)]
    Timeline(#[from] TimelineError),
    #[error("unknown environment attribute `{0}`; the registry is behind the game version")]
    UnknownAttribute(String),
    #[error("`{0}` is not a valid clock id")]
    MalformedClock(String),
}

#[derive(Debug, Clone)]
enum Layer {
    Biome,
    Track { clock: usize, sampler: AttributeTrackSampler },
    Weather { rain: Option<(Operation, AttributeValue)>, thunder: Option<(Operation, AttributeValue)> },
}

/// One attribute's layers, with everything constant for the dimension already
/// folded into `base`.
#[derive(Debug, Clone)]
pub struct AttributeStack {
    pub spec: &'static AttributeSpec,
    index: usize,
    base: AttributeValue,
    layers: Vec<Layer>,
}

impl AttributeStack {
    /// The value with only the layers that never change for this dimension.
    pub fn base(&self) -> &AttributeValue {
        &self.base
    }

    /// Whether anything below the biome layer can still move this value.
    pub fn is_dynamic(&self) -> bool {
        self.layers
            .iter()
            .any(|layer| matches!(layer, Layer::Track { .. } | Layer::Weather { .. }))
    }

    pub fn evaluate(&self, ctx: &EnvironmentContext) -> AttributeValue {
        let mut value = self.base.clone();
        for layer in &self.layers {
            value = match layer {
                Layer::Biome if !self.spec.positional => value,
                Layer::Biome => ctx.biomes.apply(self.index, self.spec, &value),
                Layer::Track { clock, sampler } => sampler
                    .apply_at(&value, ctx.ticks.get(*clock).copied().unwrap_or(0.0))
                    .unwrap_or(value),
                Layer::Weather { rain, thunder } => self.apply_weather(rain, thunder, value, ctx),
            };
        }
        self.spec.sanitize(value)
    }

    /// Each level blends the layer beneath it towards the modified value, and
    /// thunder is measured out of the rain level rather than on top of it.
    fn apply_weather(
        &self,
        rain: &Option<(Operation, AttributeValue)>,
        thunder: &Option<(Operation, AttributeValue)>,
        value: AttributeValue,
        ctx: &EnvironmentContext,
    ) -> AttributeValue {
        let thunder_level = ctx.weather.thunder;
        let rain_level = ctx.weather.rain - thunder_level;
        let lerp = self.spec.ty.state_change_lerp();
        let mut value = value;
        for (entry, level) in [(rain, rain_level), (thunder, thunder_level)] {
            let Some((op, argument)) = entry else { continue };
            if level <= 0.0 {
                continue;
            }
            let Ok(modified) = apply(self.spec.ty, *op, &value, argument) else { continue };
            value = lerp.apply(level, &value, &modified);
        }
        value
    }
}

/// What a dimension contributes to its layer stack, apart from its timelines.
///
/// Narrower than the whole dimension type so the stack does not drag the asset
/// graph — an infiniburn tag has nothing to say about the sky.
#[derive(Debug, Clone, Copy)]
pub struct DimensionEnvironment<'a> {
    pub id: &'a str,
    pub attributes: &'a EnvironmentAttributeMap,
    pub skybox: Skybox,
    pub has_skylight: bool,
    pub has_ceiling: bool,
}

impl<'a> DimensionEnvironment<'a> {
    pub fn of(id: &'a str, dimension_type: &'a DimensionType) -> Self {
        DimensionEnvironment {
            id,
            attributes: &dimension_type.attributes,
            skybox: dimension_type.skybox,
            has_skylight: dimension_type.has_skylight,
            has_ceiling: dimension_type.has_ceiling,
        }
    }

    /// `Level.canHaveWeather`.
    pub fn can_have_weather(&self) -> bool {
        self.has_skylight && !self.has_ceiling && self.id != "minecraft:the_end"
    }
}

/// The layer stacks of one dimension, one per registered attribute.
///
/// Derived from the dimension type, the timelines its tag names and the
/// registry; rebuilt only when those assets change.
#[derive(Resource, Debug, Clone)]
pub struct EnvironmentAttributes {
    skybox: Skybox,
    clocks: Vec<ResourceLocation<Arc<str>>>,
    stacks: Vec<AttributeStack>,
}

impl EnvironmentAttributes {
    pub fn build(
        dimension: &DimensionEnvironment,
        timelines: &[&Timeline],
    ) -> Result<Self, EnvironmentError> {
        let mut stacks: Vec<AttributeStack> = ENVIRONMENT_ATTRIBUTES
            .values()
            .enumerate()
            .map(|(index, spec)| AttributeStack {
                spec,
                index,
                base: spec.default.clone(),
                layers: Vec::new(),
            })
            .collect();

        for (id, entry) in &dimension.attributes.0 {
            let stack = &mut stacks[index_of(id.as_str())?];
            let argument = entry.value(stack.spec)?;
            stack.base = apply(stack.spec.ty, entry.modifier, &stack.base, &argument)?;
        }

        for stack in &mut stacks {
            stack.layers.push(Layer::Biome);
        }

        let mut clocks: Vec<ResourceLocation<Arc<str>>> = Vec::new();
        for timeline in timelines {
            let clock = ResourceLocation::parse(&timeline.clock)
                .map_err(|_| EnvironmentError::MalformedClock(timeline.clock.clone()))?;
            let clock = match clocks.iter().position(|known| *known == clock) {
                Some(known) => known,
                None => {
                    clocks.push(clock);
                    clocks.len() - 1
                }
            };
            for (id, sampler) in timeline.bake()? {
                stacks[index_of(&id)?].layers.push(Layer::Track { clock, sampler });
            }
        }

        if dimension.can_have_weather() {
            for (id, rain, thunder) in weather_layers()? {
                stacks[index_of(id)?].layers.push(Layer::Weather { rain, thunder });
            }
        }

        Ok(EnvironmentAttributes { skybox: dimension.skybox, clocks, stacks })
    }

    /// The stack index of `id`. Resolved while building, never during a frame.
    pub fn index(id: &str) -> Option<usize> {
        ENVIRONMENT_ATTRIBUTES.keys().position(|key| *key == id)
    }

    pub fn stack(&self, index: usize) -> &AttributeStack {
        &self.stacks[index]
    }

    pub fn stacks(&self) -> &[AttributeStack] {
        &self.stacks
    }

    pub fn skybox(&self) -> Skybox {
        self.skybox
    }

    pub fn clocks(&self) -> &[ResourceLocation<Arc<str>>] {
        &self.clocks
    }

    /// The fractional tick of every clock this dimension's timelines run on,
    /// in the order [`Layer::Track`] indexes them.
    pub fn clock_ticks(&self, clocks: &WorldClocks, out: &mut Vec<f64>) {
        out.clear();
        out.extend(self.clocks.iter().map(|id| {
            clocks
                .get(id.as_str())
                .map_or(0.0, |state| state.total_ticks as f64 + f64::from(state.partial_tick))
        }));
    }

    /// Read one attribute by id. For tests and cold paths; a frame walks
    /// [`EnvironmentAttributes::stack`] by index instead.
    pub fn value(&self, id: &str, ctx: &EnvironmentContext) -> Option<AttributeValue> {
        Some(self.stacks[Self::index(id)?].evaluate(ctx))
    }
}

/// One layer stack set per loaded dimension type.
///
/// Derived from the dimension type registry, the `timeline` registry and the
/// tag that joins them; rebuilt whole rather than patched.
#[derive(Resource, Debug, Clone, Default)]
pub struct DimensionEnvironments(HashMap<ResourceLocation<Arc<str>>, EnvironmentAttributes>);

impl DimensionEnvironments {
    pub fn get(&self, dimension_type: &str) -> Option<&EnvironmentAttributes> {
        self.0.get(dimension_type)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &EnvironmentAttributes)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Fold the loaded `timeline` registry into everything derived from it: the
/// time marker table beside the clocks, and one layer stack set per dimension.
pub fn freeze_timelines(
    timelines: Res<Assets<Timeline>>,
    dimension_types: Res<Assets<DimensionType>>,
    timeline_index: Res<DynRegistryIndex<Timeline>>,
    tag_files: Res<Assets<TagFile>>,
    asset_server: Res<AssetServer>,
    mut markers: ResMut<ClockTimeMarkers>,
    mut environments: ResMut<DimensionEnvironments>,
) {
    let mut loaded: Vec<(ResourceLocation<Arc<str>>, &Timeline)> = timelines
        .iter()
        .filter_map(|(id, timeline)| {
            Some((rl_from_asset_path(asset_server.get_path(id)?.path())?, timeline))
        })
        .collect();
    loaded.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

    for error in markers.rebuild(loaded.iter().map(|(_, timeline)| *timeline)) {
        tracing::error!(%error, "rejected time marker");
    }

    let by_id: HashMap<&str, &Timeline> = loaded
        .iter()
        .map(|(id, timeline)| (id.as_str(), *timeline))
        .collect();

    environments.0.clear();
    for (asset_id, dimension_type) in dimension_types.iter() {
        let Some(id) = asset_server
            .get_path(asset_id)
            .and_then(|path| rl_from_asset_path(path.path()))
        else {
            continue;
        };

        let members = dimension_type
            .timelines
            .as_ref()
            .and_then(|tag| tag_files.get(tag.handle()))
            .map(|tag_file| resolve_tag_file_ordered(tag_file, &tag_files, &*timeline_index))
            .unwrap_or_default();
        let dimension_timelines: Vec<&Timeline> = members
            .into_iter()
            .filter_map(|member| {
                let id = timeline_index.location(member)?;
                by_id.get(id.as_str()).copied()
            })
            .collect();

        match EnvironmentAttributes::build(
            &DimensionEnvironment::of(id.as_str(), dimension_type),
            &dimension_timelines,
        ) {
            Ok(attributes) => {
                environments.0.insert(id, attributes);
            }
            Err(error) => tracing::error!(dimension = %id, %error, "could not build environment"),
        }
    }

    tracing::info!(
        dimensions = environments.len(),
        markers = markers.len(),
        "froze timelines"
    );
}

/// Everything a layer stack needs that is not fixed for the dimension.
pub struct EnvironmentContext<'a> {
    pub position: DVec3,
    pub ticks: &'a [f64],
    pub biomes: &'a SpatialAttributeInterpolator,
    pub weather: Weather,
}

fn index_of(id: &str) -> Result<usize, EnvironmentError> {
    EnvironmentAttributes::index(id)
        .ok_or_else(|| EnvironmentError::UnknownAttribute(id.to_owned()))
}

/// `WeatherAttributes.RAIN` and `THUNDER`.
///
/// The colours are the reference's float literals pushed through
/// `ARGB.as8BitChannel`, which floors: 0.6 is `0x99`, not `0x9a`.
static WEATHER: LazyLock<[EnvironmentAttributeMap; 2]> = LazyLock::new(|| {
    let level = |sky_gray: serde_json::Value, cloud_gray: serde_json::Value, tint: &str, alpha: f32| {
        json!({
            "minecraft:visual/sky_color": {"argument": sky_gray, "modifier": "blend_to_gray"},
            "minecraft:visual/fog_color": {"argument": format!("#{tint}"), "modifier": "multiply"},
            "minecraft:visual/cloud_color": {"argument": cloud_gray, "modifier": "blend_to_gray"},
            "minecraft:gameplay/sky_light_level":
                {"argument": {"value": 4.0, "alpha": alpha}, "modifier": "alpha_blend"},
            "minecraft:visual/sky_light_color":
                {"argument": format!("#{:02x}7a7aff", (alpha * 255.0) as u32), "modifier": "alpha_blend"},
            "minecraft:visual/sky_light_factor":
                {"argument": {"value": 0.24, "alpha": alpha}, "modifier": "alpha_blend"},
            "minecraft:visual/star_brightness": 0.0,
            "minecraft:visual/sunrise_sunset_color":
                {"argument": format!("#ff{tint}"), "modifier": "multiply"},
            "minecraft:gameplay/bees_stay_in_hive": true,
        })
    };
    [
        level(json!({"brightness": 0.6, "factor": 0.75}), json!({"brightness": 0.24, "factor": 0.5}), "7f7f99", 0.3125),
        level(json!({"brightness": 0.24, "factor": 0.94}), json!({"brightness": 0.095, "factor": 0.94}), "3f3f4c", 0.52734375),
    ]
    .map(|value| serde_json::from_value(value).expect("the built-in weather layers are well formed"))
});

type WeatherEntry = Option<(Operation, AttributeValue)>;

fn weather_layers() -> Result<Vec<(&'static str, WeatherEntry, WeatherEntry)>, EnvironmentError> {
    let [rain, thunder] = &*WEATHER;
    let mut ids: Vec<&str> = rain.0.keys().chain(thunder.0.keys()).map(|id| id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();

    ids.into_iter()
        .map(|id| {
            let spec = crate::attribute::attribute(id)
                .ok_or_else(|| EnvironmentError::UnknownAttribute(id.to_owned()))?;
            let entry = |map: &EnvironmentAttributeMap| match map.get(id) {
                Some(entry) => entry.value(spec).map(|value| Some((entry.modifier, value))),
                None => Ok(None),
            };
            Ok((spec.id, entry(rain)?, entry(thunder)?))
        })
        .collect()
}

#[cfg(test)]
mod tests;
