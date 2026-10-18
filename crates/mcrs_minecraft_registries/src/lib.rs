#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod diagnostics;
mod id;
mod json;
mod load;
mod order;
mod pack;
mod registry;
mod tags;

pub use diagnostics::{Diagnostic, Diagnostics, Level, Missing, Referrer};
pub use id::Key;
pub use load::{
    DataDeclaration, Declaration, IdSeed, IdSetSeed, LoadCtx, RefSeed, Registries, RegistryView,
    Stage, StageScope, StageView, ValueSeed, load, load_with,
};
pub use pack::{
    AvailablePack, Features, KnownPack, PackFiles, PackIndex, PackSelection, PackStack,
    StackedPack, pack_stack,
};
pub use registry::{ReadValue, Registry, RegistryOf, RegistryType};
pub use tags::{IdSet, Ref, Tags};

#[doc(hidden)]
pub use serde as __serde;

// chisle: a value read this way resolves its references through whatever thread-local scope the
// caller installs around the stage, not through the `LoadCtx` seeds; giving the type a seed-based
// `ReadValue` retires its use of this macro, and the macro goes with the last one.
#[doc(hidden)]
#[macro_export]
macro_rules! read_with_serde {
    ($($value:ty),+ $(,)?) => {$(
        impl $crate::ReadValue for $value {
            fn read<'de, D: $crate::__serde::Deserializer<'de>>(
                _: &$crate::LoadCtx<'_>,
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                <Self as $crate::__serde::Deserialize<'de>>::deserialize(deserializer)
            }
        }
    )+};
}

#[macro_export]
macro_rules! declare_registries {
    ($($value:ty => $key:expr, $stage:ident $(, $non_empty:ident)?;)+) => {$(
        impl $value {
            pub fn declaration() -> $crate::DataDeclaration<$crate::RegistryOf<$value>> {
                $crate::Declaration::data($key.location().into(), $crate::Stage::$stage)
                    $(.$non_empty())?
            }
        }
    )+};
}
