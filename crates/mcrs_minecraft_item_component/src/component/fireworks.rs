use std::ops::Not;

use mcrs_minecraft_core::codec::{self, is_default};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, LIST_ID, STRING_ID};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::book::size_limited;
use crate::component::common::{ordinal_enum, unsigned_byte};
use crate::harness::Sample;
use mcrs_minecraft_core::Bounded;

ordinal_enum! {
    FireworkShape { SmallBall, LargeBall, Star, Creeper, Burst }
}

fn int_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i32>, D::Error> {
    Ok(
        Vec::<codec::Bounded<{ i32::MIN }, { i32::MAX }>>::deserialize(d)?
            .into_iter()
            .map(|v| v.0)
            .collect(),
    )
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworkExplosion {
    pub shape: FireworkShape,
    #[serde(
        default,
        deserialize_with = "int_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub colors: Vec<i32>,
    #[serde(
        default,
        deserialize_with = "int_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub fade_colors: Vec<i32>,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub has_trail: bool,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub has_twinkle: bool,
}

impl Default for FireworkExplosion {
    fn default() -> Self {
        FireworkExplosion {
            shape: FireworkShape::SmallBall,
            colors: Vec::new(),
            fade_colors: Vec::new(),
            has_trail: false,
            has_twinkle: false,
        }
    }
}

impl Sample for FireworkExplosion {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID), ("shape", STRING_ID)];
        if self.has_trail {
            tags.extend([
                ("colors", LIST_ID),
                ("fade_colors", LIST_ID),
                ("has_trail", BYTE_ID),
                ("has_twinkle", BYTE_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            FireworkExplosion::default(),
            FireworkExplosion {
                shape: FireworkShape::Star,
                colors: vec![0xFF0000, 0x00FF00],
                fade_colors: vec![0x0000FF],
                has_trail: true,
                has_twinkle: true,
            },
        ]
    }
}

pub const MAX_EXPLOSIONS: usize = 256;

fn flight_duration<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    unsigned_byte(d).map(|b| b as u8 as i32)
}

/// The wire carries any VarInt, so a peer's value only fails here, as
/// vanilla's does.
fn unsigned_byte_tag<S: Serializer>(value: &i32, s: S) -> Result<S::Ok, S::Error> {
    if *value > 255 {
        return Err(S::Error::custom(format_args!(
            "Unsigned byte was too large: {value} > 255"
        )));
    }
    s.serialize_i8(*value as i8)
}

fn no_explosions(explosions: &Bounded<Vec<FireworkExplosion>, MAX_EXPLOSIONS>) -> bool {
    explosions.0.is_empty()
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fireworks {
    #[serde(
        default,
        deserialize_with = "flight_duration",
        serialize_with = "unsigned_byte_tag",
        skip_serializing_if = "is_default"
    )]
    pub flight_duration: i32,
    #[serde(
        default,
        deserialize_with = "size_limited",
        skip_serializing_if = "no_explosions"
    )]
    pub explosions: Bounded<Vec<FireworkExplosion>, MAX_EXPLOSIONS>,
}

impl Sample for Fireworks {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if self.flight_duration != 0 {
            tags.extend([("flight_duration", BYTE_ID), ("explosions", LIST_ID)]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Fireworks::default(),
            Fireworks {
                flight_duration: 200,
                explosions: Bounded(vec![FireworkExplosion {
                    shape: FireworkShape::Burst,
                    colors: vec![1],
                    ..Default::default()
                }]),
            },
        ]
    }
}
