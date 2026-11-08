use mcrs_minecraft_entity::entity_type_keys::EntityType;

/// An entity type that the server simulates as a mob row (TRK-TYPE-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MobType {
    Pig,
    Cow,
    Sheep,
    Chicken,
    ZombifiedPiglin,
}

impl MobType {
    pub const ALL: [Self; 5] = [
        Self::Pig,
        Self::Cow,
        Self::Sheep,
        Self::Chicken,
        Self::ZombifiedPiglin,
    ];

    /// The mob type of `entity_type`, or `None` when the server makes no mob of it.
    #[must_use]
    pub fn of(entity_type: EntityType) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mob| mob.entity_type() == entity_type)
    }

    #[must_use]
    pub const fn entity_type(self) -> EntityType {
        match self {
            Self::Pig => EntityType::Pig,
            Self::Cow => EntityType::Cow,
            Self::Sheep => EntityType::Sheep,
            Self::Chicken => EntityType::Chicken,
            Self::ZombifiedPiglin => EntityType::ZombifiedPiglin,
        }
    }

    /// The width and the height of the adult box of the `standing` pose.
    #[must_use]
    pub const fn size(self) -> [f32; 2] {
        match self {
            Self::Pig => [0.9, 0.9],
            Self::Cow => [0.9, 1.4],
            Self::Sheep => [0.9, 1.3],
            Self::Chicken => [0.4, 0.7],
            Self::ZombifiedPiglin => [0.6, 1.95],
        }
    }

    #[must_use]
    pub const fn eye_height(self) -> f32 {
        match self {
            Self::Pig => 0.765,
            Self::Cow => 1.3,
            Self::Sheep => 1.235,
            Self::Chicken => 0.644,
            Self::ZombifiedPiglin => 1.79,
        }
    }

    /// The client tracking range, in columns.
    #[must_use]
    pub const fn tracking_range(self) -> u32 {
        match self {
            Self::ZombifiedPiglin => 8,
            Self::Pig | Self::Cow | Self::Sheep | Self::Chicken => 10,
        }
    }

    /// The tracker ticks between two periodic updates.
    #[must_use]
    pub const fn update_interval(self) -> u32 {
        3
    }

    /// The base of `max_health`, which is also the health of a new mob.
    #[must_use]
    pub const fn max_health(self) -> f32 {
        match self {
            Self::Pig | Self::Cow => 10.0,
            Self::Sheep => 8.0,
            Self::Chicken => 4.0,
            Self::ZombifiedPiglin => 20.0,
        }
    }

    /// The base of `movement_speed`, widened from its f32 value where the type gives one.
    #[must_use]
    pub fn movement_speed(self) -> f64 {
        match self {
            Self::Pig | Self::Chicken => 0.25,
            Self::Cow => f64::from(0.2_f32),
            Self::Sheep | Self::ZombifiedPiglin => f64::from(0.23_f32),
        }
    }

    /// The farm animals, whose entity data chain is entity, living entity, mob, ageable mob and
    /// the part of the type (TRK-DATA-006).
    #[must_use]
    pub const fn is_animal(self) -> bool {
        !matches!(self, Self::ZombifiedPiglin)
    }
}
