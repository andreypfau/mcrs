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
use mcrs_minecraft_registry::{Id, Registry, RegistrySet};

use crate::dimension_type::DimensionTypeEnvironment;
use mcrs_minecraft_dimension::{Dimension, DimensionType, Skybox};
use mcrs_minecraft_environment::attribute::{
    AttributeEntry, AttributeError, AttributeSpec, AttributeValue, ENVIRONMENT_ATTRIBUTES,
    EnvironmentAttributeMap, ModifierError, Operation, apply,
};
use mcrs_minecraft_environment::timeline::{AttributeTrackSampler, Timeline};
use mcrs_minecraft_environment::world_clock::WorldClock;
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
    #[error("the registry set holds no `{0}` registry")]
    MissingRegistry(&'static str),
}

#[derive(Debug, Clone)]
enum Layer {
    Biome,
    Track {
        clock: usize,
        sampler: AttributeTrackSampler,
    },
    Weather {
        rain: WeatherEntry,
        thunder: WeatherEntry,
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
        rain: &WeatherEntry,
        thunder: &WeatherEntry,
        value: AttributeValue,
        ctx: &EnvironmentContext,
    ) -> AttributeValue {
        let thunder_level = ctx.weather.thunder;
        let rain_level = ctx.weather.rain - thunder_level;
        let lerp = self.spec.ty.state_change_lerp();
        let mut value = value;
        for ((op, argument), level) in [(rain, rain_level), (thunder, thunder_level)] {
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
    pub fn of(
        dimension: &ResourceKey<Dimension>,
        dimension_type: &DimensionType,
        environment: &'a DimensionTypeEnvironment,
    ) -> Self {
        DimensionEnvironment {
            attributes: &environment.attributes,
            skybox: dimension_type.skybox,
            can_have_weather: dimension_type.has_skylight
                && !dimension_type.has_ceiling
                && *dimension != mcrs_minecraft_dimension::keys::dimension::THE_END,
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
    clocks: Vec<Id<WorldClock>>,
    stacks: Vec<AttributeStack>,
}

impl EnvironmentAttributes {
    pub fn build(
        dimension: &DimensionEnvironment,
        timelines: &[&Timeline],
        world_clocks: &Registry<WorldClock>,
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

        let mut clocks: Vec<Id<WorldClock>> = Vec::new();
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
            for (id, rain, thunder) in weather_layers() {
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

    pub fn of_dimension(
        registries: &RegistrySet,
        dimension: &ResourceKey<Dimension>,
        dimension_type: Id<DimensionType>,
    ) -> Result<Self, EnvironmentError> {
        let timeline_registry = mcrs_minecraft_environment::keys::TIMELINE.location();
        let timelines = registries
            .column::<Timeline>(timeline_registry.as_static_str())
            .ok_or(EnvironmentError::MissingRegistry("timeline"))?;
        let timeline_tags = registries
            .tags::<Timeline>()
            .ok_or(EnvironmentError::MissingRegistry("timeline"))?;
        let world_clocks = registries
            .registry::<WorldClock>()
            .ok_or(EnvironmentError::MissingRegistry("world_clock"))?;
        let dimension_types = registries
            .entries::<DimensionType, DimensionType>()
            .ok_or(EnvironmentError::MissingRegistry("dimension_type"))?;
        let dimension_environments = registries
            .entries::<DimensionType, DimensionTypeEnvironment>()
            .ok_or(EnvironmentError::MissingRegistry("dimension_type"))?;

        let environment = &dimension_environments[dimension_type];
        let dimension_timelines: Vec<&Timeline> = environment
            .timelines
            .ids(&timeline_tags)
            .filter_map(|member| timelines.get(member.index()))
            .collect();

        Self::build(
            &DimensionEnvironment::of(dimension, &dimension_types[dimension_type], environment),
            &dimension_timelines,
            &world_clocks,
        )
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

    pub fn clocks(&self) -> &[Id<WorldClock>] {
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
    use mcrs_minecraft_environment::keys::EnvironmentAttribute as A;
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
                (A::VisualSkyColor.location(), blend_to_gray(sky_gray)),
                (A::VisualFogColor.location(), multiply(0xFF00_0000 | tint)),
                (A::VisualCloudColor.location(), blend_to_gray(cloud_gray)),
                (A::GameplaySkyLightLevel.location(), alpha_blend(4.0)),
                (
                    A::VisualSkyLightColor.location(),
                    AttributeEntry {
                        argument: AttributeValue::Color(sky_light_alpha << 24 | 0x7a_7aff),
                        modifier: Operation::AlphaBlend,
                    },
                ),
                (A::VisualSkyLightFactor.location(), alpha_blend(0.24)),
                (
                    A::VisualStarBrightness.location(),
                    AttributeEntry::override_value(AttributeValue::Float(0.0)),
                ),
                (
                    A::VisualSunriseSunsetColor.location(),
                    multiply(0xFF00_0000 | tint),
                ),
                (
                    A::GameplayBeesStayInHive.location(),
                    AttributeEntry::override_value(AttributeValue::Bool(true)),
                ),
            ]
            .into_iter()
            .map(|(id, entry)| (id.into(), entry))
            .collect(),
        )
    };
    [
        level((0.6, 0.75), (0.24, 0.5), 0x7f_7f99, 0.3125),
        level((0.24, 0.94), (0.095, 0.94), 0x3f_3f4c, 0.52734375),
    ]
});

fn weather_layers() -> impl Iterator<Item = (&'static str, WeatherEntry, WeatherEntry)> {
    let [rain, thunder] = &*WEATHER;
    let entry = |entry: &AttributeEntry| (entry.modifier, entry.argument.clone());
    rain.0
        .iter()
        .zip(thunder.0.values())
        .map(move |((id, rain), thunder)| (id.as_str(), entry(rain), entry(thunder)))
}

type WeatherEntry = (Operation, AttributeValue);

#[cfg(test)]
mod tests;
