use mcrs_minecraft_command::{Lookup, NoLookup};
use mcrs_minecraft_resource::ResourceLocation;

/// What the time arguments read from the world of the source besides its registries.
pub trait TimeLookup: Lookup {
    /// The time markers of a clock that a command suggests; `None` is the default clock of the
    /// source dimension.
    fn suggested_time_markers(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str>;

    /// The timelines that run on a clock; `None` is the default clock of the source dimension.
    fn timelines_of_clock(&self, clock: Option<ResourceLocation<'_>>)
    -> impl Iterator<Item = &str>;
}

impl TimeLookup for NoLookup {
    fn suggested_time_markers(
        &self,
        _: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn timelines_of_clock(&self, _: Option<ResourceLocation<'_>>) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }
}
