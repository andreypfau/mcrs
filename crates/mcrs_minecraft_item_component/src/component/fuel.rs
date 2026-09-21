use std::fmt;

use mcrs_minecraft_core::codec::{Number, float_value};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, STRING_ID};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::{RegistryName, resolvable};
use crate::harness::Sample;

pub enum NumberProviderReg {}

impl RegistryName for NumberProviderReg {
    const NAME: &'static str = "number_provider";
}

resolvable!(
    ResolvableNumber,
    f32,
    NumberProviderReg,
    "a number or a number provider id",
    float_value
);

impl ResolvableNumber {
    pub fn reference(location: ResourceLocation) -> Self {
        ResolvableNumber::Reference(ResourceKey::from_location(location))
    }
}

fn tag_of(number: &ResolvableNumber) -> u8 {
    match number {
        ResolvableNumber::Constant(_) => FLOAT_ID,
        ResolvableNumber::Reference(_) => STRING_ID,
    }
}

macro_rules! fuel {
    ($($ty:ident { $($field:ident),+ }),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $ty {
            $(pub $field: ResolvableNumber,)+
        }

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", COMPOUND_ID), $((stringify!($field), tag_of(&self.$field)),)+]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty { $($field: ResolvableNumber::Constant(3.0),)+ },
                    $ty { $($field: ResolvableNumber::reference(ResourceLocation::minecraft(stringify!($field))),)+ },
                ]
            }
        }
    )*};
}

fuel! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
