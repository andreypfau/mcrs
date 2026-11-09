//! The environment attribute layers of one dimension.
//!
//! A value starts at the default of its attribute. The entry of the dimension type applies,
//! then the track of each timeline of the dimension type's tag, in the order of the tag. The
//! biome layer goes between the dimension type and the timelines, and the weather layer after
//! the timelines; neither exists yet. A float that leaves its range after the last layer
//! becomes the nearer bound. Nothing memoizes an effective value: every read composes the
//! layers afresh, so a read after a change of a clock sees the new clock state.

#[cfg(feature = "bevy")]
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::dimension_type::{DimensionTypeEnvironment, DimensionTypeFile};
use mcrs_minecraft_dimension::{Dimension, DimensionType, Skybox};
use mcrs_minecraft_environment::attribute::{
    AttributeValue, Interpolation, ModifierMap, default_value, info,
};
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::timeline::{Timeline, Track};
use mcrs_minecraft_environment::world_clock::{WorldClock, WorldClocks};

/// How much it is raining and thundering, in `[0; 1]`.
///
/// Truth until the reader that supplies it from the save lands.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct WeatherLevel {
    pub rain: f32,
    pub thunder: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
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
    track: Track,
}

/// The layers of one attribute, with the default and the dimension type's entry already
/// folded into `base`.
#[derive(Debug, Clone)]
pub struct AttributeStack {
    attribute: EnvironmentAttribute,
    base: AttributeValue,
    tracks: Vec<TrackLayer>,
}

impl AttributeStack {
    #[must_use]
    pub fn attribute(&self) -> EnvironmentAttribute {
        self.attribute
    }

    /// Whether a timeline track can still move this value.
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        !self.tracks.is_empty()
    }

    /// The value at the whole ticks of `ctx`. A clock's partial tick has no effect.
    #[must_use]
    pub fn evaluate(&self, ctx: &EnvironmentContext) -> AttributeValue {
        self.tracks
            .iter()
            .fold(self.base.clone(), |value, layer| {
                let ticks = ctx.ticks.get(layer.clock).copied().unwrap_or(0);
                layer.track.apply(value, ticks)
            })
            .sanitized(self.attribute)
    }

    /// The value in a frame: the values of the previous and the current tick, moved between
    /// by `partial`, the frame's fraction of the current tick.
    #[must_use]
    pub fn frame_value(
        &self,
        previous: &EnvironmentContext,
        current: &EnvironmentContext,
        partial: f32,
    ) -> AttributeValue {
        let now = self.evaluate(current);
        if !self.is_dynamic() {
            return now;
        }
        AttributeValue::interpolate(
            info(self.attribute).kind,
            Interpolation::PartialTick,
            partial,
            &self.evaluate(previous),
            &now,
        )
    }
}

/// What a dimension contributes to its layer stack, apart from its timelines.
///
/// Narrower than the whole dimension type so the stack does not drag the asset
/// graph — an infiniburn tag has nothing to say about the sky.
#[derive(Debug, Clone, Copy)]
pub struct DimensionEnvironment<'a> {
    pub attributes: &'a ModifierMap,
    pub skybox: Skybox,
    pub can_have_weather: bool,
}

impl<'a> DimensionEnvironment<'a> {
    /// The end has no weather by its key, whatever type it is given.
    pub fn of(
        dimension: &ResourceKeyBuf<Dimension>,
        dimension_type: &DimensionType,
        environment: &'a DimensionTypeEnvironment,
    ) -> Self {
        DimensionEnvironment {
            attributes: &environment.attributes,
            skybox: dimension_type.skybox,
            can_have_weather: dimension_type.has_skylight
                && !dimension_type.has_ceiling
                && *dimension != mcrs_minecraft_dimension::dimension_keys::THE_END,
        }
    }
}

/// The layer stacks of one dimension, one per attribute, in registry order.
///
/// Derived once from the dimension type and the timelines its tag names.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct DimensionAttributes {
    skybox: Skybox,
    clocks: Vec<Id<WorldClock>>,
    stacks: Vec<AttributeStack>,
}

impl DimensionAttributes {
    pub fn build(
        dimension: &DimensionEnvironment,
        timelines: &[&Timeline],
        clock_count: usize,
    ) -> Result<Self, EnvironmentError> {
        let mut stacks: Vec<AttributeStack> = EnvironmentAttribute::ALL
            .iter()
            .map(|attribute| {
                let base = default_value(*attribute);
                AttributeStack {
                    attribute: *attribute,
                    base: match dimension.attributes.get(*attribute) {
                        Some(entry) => entry.apply(base),
                        None => base,
                    },
                    tracks: Vec::new(),
                }
            })
            .collect();

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
            for (attribute, track) in &timeline.tracks {
                if let Some(stack) = stacks.get_mut(*attribute as usize) {
                    stack.tracks.push(TrackLayer {
                        clock,
                        track: track.clone(),
                    });
                }
            }
        }

        Ok(DimensionAttributes {
            skybox: dimension.skybox,
            clocks,
            stacks,
        })
    }

    pub fn of_dimension(
        registries: &Registries,
        dimension: &ResourceKeyBuf<Dimension>,
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

    #[must_use]
    pub fn stack(&self, attribute: EnvironmentAttribute) -> Option<&AttributeStack> {
        self.stacks.get(attribute as usize)
    }

    #[must_use]
    pub fn stacks(&self) -> &[AttributeStack] {
        &self.stacks
    }

    #[must_use]
    pub fn skybox(&self) -> Skybox {
        self.skybox
    }

    #[must_use]
    pub fn clocks(&self) -> &[Id<WorldClock>] {
        &self.clocks
    }

    /// The total ticks of every clock this dimension's timelines run on, in the order a track
    /// layer indexes them. A clock without a state stands at 0.
    pub fn clock_ticks(&self, clocks: &WorldClocks, out: &mut Vec<i64>) {
        out.clear();
        out.extend(
            self.clocks
                .iter()
                .map(|id| clocks.get(*id).map_or(0, |state| state.total_ticks)),
        );
    }

    /// The dimension value of `attribute` at the whole ticks of `ctx`.
    #[must_use]
    pub fn value(
        &self,
        attribute: EnvironmentAttribute,
        ctx: &EnvironmentContext,
    ) -> Option<AttributeValue> {
        self.stack(attribute).map(|stack| stack.evaluate(ctx))
    }
}

/// Everything a layer stack needs that is not fixed for the dimension: the total ticks of each
/// clock, as [`DimensionAttributes::clock_ticks`] orders them.
pub struct EnvironmentContext<'a> {
    pub ticks: &'a [i64],
}
