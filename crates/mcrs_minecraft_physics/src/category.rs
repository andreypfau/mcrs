/// The category of an entity type: its parameters and the order of its steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// A living entity on land and in air, with the vertical drag `0.98F`.
    Walker,
    /// A living entity that uses the horizontal air drag on the vertical axis too.
    Omnidirectional,
    /// An item: gravity, move, block effects, drag, then the bounce; the rest cycle of 4 ticks.
    Item,
    /// A falling block: gravity, move, block effects, the ground rule, then drag.
    FallingBlock,
    /// A primed TNT: gravity, move, block effects, drag, then the ground rule.
    PrimedTnt,
    /// An experience orb: gravity in free space, move, block effects, drag, then the bounce.
    ExperienceOrb,
}

impl Category {
    #[must_use]
    pub const fn is_living(self) -> bool {
        matches!(self, Category::Walker | Category::Omnidirectional)
    }
}
