//! Composing the environment attribute layers of one dimension.
//!
//! Five layers stack in the order the reference applies them: the registered
//! default, the dimension's constants, the biome at the position, the timeline
//! tracks, and weather. Nothing here memoizes an effective value — a layer
//! stack is derived once from immutable assets and every read composes it
//! afresh.

use std::sync::LazyLock;

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Id, Registry, RegistrySet};

use crate::dimension_type::{DimensionType, Skybox};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_environment::attribute::{
    AttributeEntry, AttributeError, AttributeSpec, AttributeValue, ENVIRONMENT_ATTRIBUTES,
    EnvironmentAttributeMap, ModifierError, Operation, apply,
};
use mcrs_minecraft_environment::timeline::{AttributeTrackSampler, Timeline};
use mcrs_minecraft_environment::world_clock::WorldClocks;

pub use mcrs_minecraft_environment::spatial::{BiomeAttributes, SpatialAttributeInterpolator};

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
    #[error("unknown environment attribute `{0}`; the registry is behind the game version")]
    UnknownAttribute(String),
    #[error("a timeline runs on a clock the world clock registry does not hold")]
    UnknownClock,
}

#[derive(Debug, Clone)]
enum Layer {
    Biome,
    Track {
        clock: usize,
        sampler: AttributeTrackSampler,
    },
    Weather {
        rain: Option<(Operation, AttributeValue)>,
        thunder: Option<(Operation, AttributeValue)>,
    },
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
            let Some((op, argument)) = entry else {
                continue;
            };
            if level <= 0.0 {
                continue;
            }
            let Ok(modified) = apply(self.spec.ty, *op, &value, argument) else {
                continue;
            };
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
    pub attributes: &'a EnvironmentAttributeMap,
    pub skybox: Skybox,
    pub can_have_weather: bool,
}

impl<'a> DimensionEnvironment<'a> {
    /// The end has no weather by its key, whatever type it is given.
    pub fn of(dimension: &ResourceKey<keys::Dimension>, dimension_type: &'a DimensionType) -> Self {
        let mut environment = Self::of_type(dimension_type);
        environment.can_have_weather &= *dimension != keys::dimension::THE_END;
        environment
    }

    // chisle: a type table has no dimension key, so it cannot apply the end rule;
    // building environments per dimension lifts it.
    pub fn of_type(dimension_type: &'a DimensionType) -> Self {
        DimensionEnvironment {
            attributes: &dimension_type.attributes,
            skybox: dimension_type.skybox,
            can_have_weather: dimension_type.has_skylight && !dimension_type.has_ceiling,
        }
    }
}

/// The layer stacks of one dimension, one per registered attribute.
///
/// Derived once from the dimension type, the timelines its tag names and the
/// attribute registry.
#[derive(Resource, Debug, Clone)]
pub struct EnvironmentAttributes {
    skybox: Skybox,
    clocks: Vec<Id<keys::WorldClock>>,
    stacks: Vec<AttributeStack>,
}

impl EnvironmentAttributes {
    pub fn build(
        dimension: &DimensionEnvironment,
        timelines: &[&Timeline],
        world_clocks: &Registry<keys::WorldClock>,
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
            stack.base = apply(stack.spec.ty, entry.modifier, &stack.base, &entry.argument)?;
        }

        for stack in &mut stacks {
            stack.layers.push(Layer::Biome);
        }

        let mut clocks: Vec<Id<keys::WorldClock>> = Vec::new();
        for timeline in timelines {
            world_clocks
                .name(timeline.clock)
                .ok_or(EnvironmentError::UnknownClock)?;
            let clock = match clocks.iter().position(|known| *known == timeline.clock) {
                Some(known) => known,
                None => {
                    clocks.push(timeline.clock);
                    clocks.len() - 1
                }
            };
            for (id, sampler) in timeline.bake() {
                stacks[index_of(id)?]
                    .layers
                    .push(Layer::Track { clock, sampler });
            }
        }

        if dimension.can_have_weather {
            for (id, rain, thunder) in weather_layers()? {
                stacks[index_of(id)?]
                    .layers
                    .push(Layer::Weather { rain, thunder });
            }
        }

        Ok(EnvironmentAttributes {
            skybox: dimension.skybox,
            clocks,
            stacks,
        })
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

    pub fn clocks(&self) -> &[Id<keys::WorldClock>] {
        &self.clocks
    }

    /// The fractional tick of every clock this dimension's timelines run on,
    /// in the order [`Layer::Track`] indexes them.
    pub fn clock_ticks(&self, clocks: &WorldClocks, out: &mut Vec<f64>) {
        out.clear();
        out.extend(self.clocks.iter().map(|id| {
            clocks.get(*id).map_or(0.0, |state| {
                state.total_ticks as f64 + f64::from(state.partial_tick)
            })
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
/// Derived once from the dimension type registry, the `timeline` registry and
/// the tag that joins them.
#[derive(Resource, Debug, Clone, Default)]
pub struct DimensionEnvironments(Vec<Option<EnvironmentAttributes>>);

impl DimensionEnvironments {
    pub fn get(&self, dimension_type: Id<keys::DimensionType>) -> Option<&EnvironmentAttributes> {
        self.0.get(dimension_type.index())?.as_ref()
    }

    pub fn len(&self) -> usize {
        self.0.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

pub fn build_dimension_environments(
    registries: Res<RegistrySet>,
    mut environments: ResMut<DimensionEnvironments>,
) {
    let (
        Some(timelines),
        Some(timeline_tags),
        Some(world_clocks),
        Some(types),
        Some(dimension_types),
    ) = (
        registries.column::<Timeline>(keys::Timeline::KEY.as_str()),
        registries.tags::<keys::Timeline>(),
        registries.registry::<keys::WorldClock>(),
        registries.registry::<keys::DimensionType>(),
        registries.entries::<keys::DimensionType, DimensionType>(),
    )
    else {
        tracing::error!(
            "the registry set holds no dimension types, timelines and clocks to build environments from"
        );
        return;
    };

    environments.0.clear();
    environments.0.resize_with(types.len(), || None);
    for id in types.ids() {
        let dimension_type = &dimension_types[id];
        let name = types
            .name(id)
            .expect("an id of the registry has a name")
            .as_str();

        let dimension_timelines: Vec<&Timeline> = dimension_type
            .timelines
            .ids(&timeline_tags)
            .filter_map(|member| timelines.get(member.index()))
            .collect();

        match EnvironmentAttributes::build(
            &DimensionEnvironment::of_type(dimension_type),
            &dimension_timelines,
            &world_clocks,
        ) {
            Ok(attributes) => environments.0[id.index()] = Some(attributes),
            Err(error) => tracing::error!(dimension = name, %error, "could not build environment"),
        }
    }

    tracing::info!(
        dimensions = environments.len(),
        "built dimension environments"
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
    let level = |sky_gray: (f32, f32), cloud_gray: (f32, f32), tint: u32, alpha: f32| {
        let blend_to_gray = |(brightness, factor)| AttributeEntry {
            argument: AttributeValue::BlendToGray { brightness, factor },
            modifier: Operation::BlendToGray,
        };
        let alpha_blend = |value| AttributeEntry {
            argument: AttributeValue::FloatWithAlpha { value, alpha },
            modifier: Operation::AlphaBlend,
        };
        let multiply = |packed| AttributeEntry {
            argument: AttributeValue::Color(packed),
            modifier: Operation::Multiply,
        };
        let sky_light_alpha = (alpha * 255.0) as u32;
        EnvironmentAttributeMap(
            [
                ("minecraft:visual/sky_color", blend_to_gray(sky_gray)),
                ("minecraft:visual/fog_color", multiply(0xFF00_0000 | tint)),
                ("minecraft:visual/cloud_color", blend_to_gray(cloud_gray)),
                ("minecraft:gameplay/sky_light_level", alpha_blend(4.0)),
                (
                    "minecraft:visual/sky_light_color",
                    AttributeEntry {
                        argument: AttributeValue::Color(sky_light_alpha << 24 | 0x7a_7aff),
                        modifier: Operation::AlphaBlend,
                    },
                ),
                ("minecraft:visual/sky_light_factor", alpha_blend(0.24)),
                (
                    "minecraft:visual/star_brightness",
                    AttributeEntry::override_value(AttributeValue::Float(0.0)),
                ),
                (
                    "minecraft:visual/sunrise_sunset_color",
                    multiply(0xFF00_0000 | tint),
                ),
                (
                    "minecraft:gameplay/bees_stay_in_hive",
                    AttributeEntry::override_value(AttributeValue::Bool(true)),
                ),
            ]
            .into_iter()
            .map(|(id, entry)| (ResourceLocation::new_static(id).into(), entry))
            .collect(),
        )
    };
    [
        level((0.6, 0.75), (0.24, 0.5), 0x7f_7f99, 0.3125),
        level((0.24, 0.94), (0.095, 0.94), 0x3f_3f4c, 0.52734375),
    ]
});

type WeatherEntry = Option<(Operation, AttributeValue)>;

fn weather_layers() -> Result<Vec<(&'static str, WeatherEntry, WeatherEntry)>, EnvironmentError> {
    let [rain, thunder] = &*WEATHER;
    let mut ids: Vec<&str> = rain
        .0
        .keys()
        .chain(thunder.0.keys())
        .map(|id| id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();

    ids.into_iter()
        .map(|id| {
            let spec = mcrs_minecraft_environment::attribute::attribute(id)
                .ok_or_else(|| EnvironmentError::UnknownAttribute(id.to_owned()))?;
            let entry = |map: &EnvironmentAttributeMap| {
                map.get(id)
                    .map(|entry| (entry.modifier, entry.argument.clone()))
            };
            Ok((spec.id, entry(rain), entry(thunder)))
        })
        .collect()
}

#[cfg(test)]
mod tests;
