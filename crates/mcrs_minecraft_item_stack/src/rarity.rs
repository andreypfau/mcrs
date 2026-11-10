/// The rarity of the stack, which sets the colour of its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}
