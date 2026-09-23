use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, INT_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::component::common::{ResolvableFloat, ResolvableInt};
use crate::harness::Sample;

impl ResolvableInt {
    pub fn reference(location: ResourceLocation) -> Self {
        ResolvableInt::Reference(ResourceKey::from_location(location))
    }

    fn nbt_tag(&self) -> u8 {
        match self {
            ResolvableInt::Constant(_) => INT_ID,
            ResolvableInt::Reference(_) => STRING_ID,
        }
    }
}

impl ResolvableFloat {
    pub fn reference(location: ResourceLocation) -> Self {
        ResolvableFloat::Reference(ResourceKey::from_location(location))
    }

    fn nbt_tag(&self) -> u8 {
        match self {
            ResolvableFloat::Constant(_) => FLOAT_ID,
            ResolvableFloat::Reference(_) => STRING_ID,
        }
    }
}

macro_rules! fuel {
    ($($ty:ident { $($field:ident: $field_ty:ident = $constant:expr),+ }),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $ty {
            $(pub $field: $field_ty,)+
        }

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", COMPOUND_ID), $((stringify!($field), self.$field.nbt_tag()),)+]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty { $($field: $field_ty::Constant($constant),)+ },
                    $ty { $($field: $field_ty::reference(ResourceLocation::minecraft(stringify!($field))),)+ },
                ]
            }
        }
    )*};
}

fuel! {
    Compostable { layers: ResolvableInt = 3 },
    CookingFuel { burn_time: ResolvableInt = 3, speed_multiplier: ResolvableFloat = 3.0 },
    BrewingFuel { uses: ResolvableInt = 3, speed_multiplier: ResolvableFloat = 3.0 },
}
