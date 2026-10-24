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

use mcrs_minecraft_registries::{DataDeclaration, Declaration};

mcrs_minecraft_registries::resolve_as_is!(crate::world_clock::WorldClock);

impl crate::timeline::Timeline {
    pub fn declaration() -> DataDeclaration<Self> {
        Declaration::data::<Self>().validate(|registry, registries| {
            match registries.registry::<crate::world_clock::WorldClock>() {
                Some(clocks) => crate::world_clock::check_time_markers(registry.values(), clocks),
                None => Vec::new(),
            }
        })
    }
}
