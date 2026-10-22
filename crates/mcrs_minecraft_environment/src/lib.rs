#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

pub mod attribute;
#[rustfmt::skip]
pub mod keys;
pub mod timeline;
pub mod world_clock;

use mcrs_minecraft_registries::{DataDeclaration, Declaration, RegistryOf, Stage};

mcrs_minecraft_registries::resolve_as_is!(crate::world_clock::WorldClock);

impl crate::world_clock::WorldClock {
    pub fn declaration() -> DataDeclaration<RegistryOf<Self>> {
        Declaration::data::<RegistryOf<Self>>(
            crate::keys::WORLD_CLOCK.location().into(),
            Stage::World,
        )
        .named_by::<Self>()
    }
}

impl crate::timeline::Timeline {
    pub fn declaration() -> DataDeclaration<RegistryOf<Self>> {
        Declaration::data::<RegistryOf<Self>>(crate::keys::TIMELINE.location().into(), Stage::World)
            .named_by::<Self>()
            .validate(|registry, ctx| {
                let clocks = ctx
                    .registry::<RegistryOf<crate::world_clock::WorldClock>>()
                    .ok_or_else(|| "the world clocks are not loaded".to_owned())?;
                let markers = crate::world_clock::check_time_markers(registry.values(), clocks);
                registry.entry_failures(markers)
            })
    }
}
