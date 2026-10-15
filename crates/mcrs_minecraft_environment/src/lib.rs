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
pub mod spatial;
pub mod timeline;
pub mod world_clock;

use mcrs_minecraft_registries::{DataDeclaration, Declaration, RegistryOf, Stage};
use mcrs_minecraft_registry::RegistrySet;

mcrs_minecraft_registries::read_with_serde!(
    crate::timeline::Timeline,
    crate::world_clock::WorldClock
);

mcrs_minecraft_registries::declare_registries!(
    crate::world_clock::WorldClock => crate::keys::WORLD_CLOCK, World;
);

impl crate::timeline::Timeline {
    pub fn declaration<S>(scope: S) -> DataDeclaration<RegistryOf<Self>>
    where
        S: Fn() -> RegistrySet + Send + Sync + 'static,
    {
        Declaration::data::<RegistryOf<Self>>(crate::keys::TIMELINE.location().into(), Stage::World)
            .validate(move |registry, _| {
                let markers = crate::world_clock::check_time_markers(registry.values(), &scope());
                registry.entry_failures(markers)
            })
    }
}
