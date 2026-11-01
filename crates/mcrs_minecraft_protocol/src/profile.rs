use crate::{Bounded, Decode, Encode};
pub use mcrs_minecraft_profile::Property;
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct GameProfile<'a> {
    pub id: Uuid,
    pub username: Bounded<&'a str, 16>,
    pub properties: Cow<'a, [Property<&'a str>]>,
}
