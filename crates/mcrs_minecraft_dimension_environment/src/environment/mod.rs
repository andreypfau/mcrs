//! Composing the environment attribute layers of one dimension.
//!
//! Three layers stack: the registered default, the dimension's constants and
//! the timeline tracks. Nothing here memoizes an effective value — a layer
//! stack is derived once from immutable assets and every read composes it
//! afresh.

#[cfg(feature = "bevy")]
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceKey;

use crate::dimension_type::{DimensionTypeEnvironment, DimensionTypeFile};
use mcrs_minecraft_dimension::{Dimension, DimensionType, Skybox};
use mcrs_minecraft_environment::attribute::{
    AttributeError, AttributeSpec, AttributeValue, ENVIRONMENT_ATTRIBUTES, EnvironmentAttributeMap,
    apply,
};
use mcrs_minecraft_environment::timeline::{AttributeTrackSampler, Timeline};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_environment::world_clock::WorldClocks;

/// How much it is raining and thundering, in `[0; 1]`.
///
/// Truth until the reader that supplies it from the save lands.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct Weather {
    pub rain: f32,
    pub thunder: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    #[error(transparent)]
    Attribute(#[from] AttributeError),
    #[error("unknown environment attribute `{0}`; the registry is behind the game version")]
    UnknownAttribute(String),
    #[error("a timeline runs on a clock the world clock registry does not hold")]
    UnknownClock,
    #[error("the registry set holds no `{0}` registry")]
    MissingRegistry(&'static str),
    #[error("the dimension type registry holds no entry {0}")]
    UnknownDimensionType(u16),
}

#[derive(Debug, Clone)]
struct TrackLayer {
    clock: usize,
    sampler: AttributeTrackSampler,
}

/// One attribute's layers, with everything constant for the dimension already
/// folded into `base`.
#[derive(Debug, Clone)]
pub struct AttributeStack {
    pub spec: &'static AttributeSpec,
    base: AttributeValue,
    layers: Vec<TrackLayer>,
}

impl AttributeStack {
    /// Whether a timeline track can still move this value.
    pub fn is_dynamic(&self) -> bool {
        !self.layers.is_empty()
    }

    pub fn evaluate(&self, ctx: &EnvironmentContext) -> AttributeValue {
        self.layers
            .iter()
            .fold(self.base.clone(), |value, TrackLayer { clock, sampler }| {
                sampler
                    .apply_at(&value, ctx.ticks.get(*clock).copied().unwrap_or(0.0))
                    .unwrap_or(value)
            })
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
#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct EnvironmentAttributes {
    skybox: Skybox,
    clocks: Vec<Id<WorldClock>>,
    stacks: Vec<AttributeStack>,
}

impl EnvironmentAttributes {
    pub fn build(
        dimension: &DimensionEnvironment,
        timelines: &[&Timeline],
        clock_count: usize,
    ) -> Result<Self, EnvironmentError> {
        let mut stacks: Vec<AttributeStack> = ENVIRONMENT_ATTRIBUTES
            .values()
            .map(|spec| AttributeStack {
                spec,
                base: spec.default.clone(),
                layers: Vec::new(),
            })
            .collect();

        for (id, entry) in &dimension.attributes.0 {
            let stack = stack_of(&mut stacks, id.as_str())?;
            // A modifier other than `override` has no specification yet, so the
            // entry is skipped rather than failing the whole dimension.
            if let Ok(value) = apply(entry.modifier, &stack.base, &entry.argument) {
                stack.base = value;
            }
        }

        let mut clocks: Vec<Id<WorldClock>> = Vec::new();
        for timeline in timelines {
            if timeline.clock.index() >= clock_count {
                return Err(EnvironmentError::UnknownClock);
            }
            let clock = match clocks.iter().position(|known| *known == timeline.clock) {
                Some(known) => known,
                None => {
                    clocks.push(timeline.clock);
                    clocks.len() - 1
                }
            };
            for (id, sampler) in timeline.bake() {
                stack_of(&mut stacks, id)?
                    .layers
                    .push(TrackLayer { clock, sampler });
            }
        }

        Ok(EnvironmentAttributes {
            skybox: dimension.skybox,
            clocks,
            stacks,
        })
    }

    pub fn of_dimension(
        registries: &Registries,
        dimension: &ResourceKey<Dimension>,
        dimension_type: Id<DimensionType>,
    ) -> Result<Self, EnvironmentError> {
        let timelines = registries
            .registry::<Timeline>()
            .ok_or(EnvironmentError::MissingRegistry("timeline"))?;
        let timeline_tags = registries
            .tags::<Timeline>()
            .ok_or(EnvironmentError::MissingRegistry("timeline"))?;
        let world_clocks = registries
            .registry::<WorldClock>()
            .ok_or(EnvironmentError::MissingRegistry("world_clock"))?;
        let dimension_types = registries
            .registry::<DimensionTypeFile>()
            .ok_or(EnvironmentError::MissingRegistry("dimension_type"))?;
        let (dimension_type, environment) =
            Id::checked(dimension_type.number(), dimension_types.len())
                .and_then(|id| dimension_types.get(id))
                .ok_or(EnvironmentError::UnknownDimensionType(
                    dimension_type.number(),
                ))?
                .split();

        let dimension_timelines: Vec<&Timeline> = environment
            .timelines
            .ids(&timeline_tags)
            .filter_map(|id| timelines.values().get(id.index()))
            .collect();

        Self::build(
            &DimensionEnvironment::of(dimension, &dimension_type, &environment),
            &dimension_timelines,
            world_clocks.len(),
        )
    }

    /// The stack index of `id`. Resolved while building, never during a frame.
    pub fn index(id: &str) -> Option<usize> {
        ENVIRONMENT_ATTRIBUTES.keys().position(|key| *key == id)
    }

    pub fn stack(&self, index: usize) -> Option<&AttributeStack> {
        self.stacks.get(index)
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
    /// in the order a track layer indexes them.
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
        self.stack(Self::index(id)?)
            .map(|stack| stack.evaluate(ctx))
    }
}

/// Everything a layer stack needs that is not fixed for the dimension.
pub struct EnvironmentContext<'a> {
    pub ticks: &'a [f64],
}

fn stack_of<'s>(
    stacks: &'s mut [AttributeStack],
    id: &str,
) -> Result<&'s mut AttributeStack, EnvironmentError> {
    stacks
        .iter_mut()
        .find(|stack| stack.spec.id == id)
        .ok_or_else(|| EnvironmentError::UnknownAttribute(id.to_owned()))
}

#[cfg(test)]
mod tests;
