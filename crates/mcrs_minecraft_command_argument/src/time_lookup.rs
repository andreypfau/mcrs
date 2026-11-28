use mcrs_minecraft_command::{Lookup, NoLookup};
use mcrs_minecraft_resource::ResourceLocation;

pub trait TimeLookup: Lookup {
    fn suggested_time_markers(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str>;

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
