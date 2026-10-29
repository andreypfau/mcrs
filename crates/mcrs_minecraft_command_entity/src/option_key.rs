/// The key of a selector option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Name,
    Distance,
    Level,
    X,
    Y,
    Z,
    Dx,
    Dy,
    Dz,
    XRotation,
    YRotation,
    Limit,
    Sort,
    GameMode,
    Team,
    Type,
    Tag,
    Nbt,
    Scores,
    Advancements,
    Predicate,
}

pub(crate) struct KeyEntry {
    pub(crate) name: &'static str,
    pub(crate) key: Key,
    pub(crate) tooltip: &'static str,
}

pub(crate) const KEYS: [KeyEntry; 21] = [
    KeyEntry {
        name: "name",
        key: Key::Name,
        tooltip: "argument.entity.options.name.description",
    },
    KeyEntry {
        name: "distance",
        key: Key::Distance,
        tooltip: "argument.entity.options.distance.description",
    },
    KeyEntry {
        name: "level",
        key: Key::Level,
        tooltip: "argument.entity.options.level.description",
    },
    KeyEntry {
        name: "x",
        key: Key::X,
        tooltip: "argument.entity.options.x.description",
    },
    KeyEntry {
        name: "y",
        key: Key::Y,
        tooltip: "argument.entity.options.y.description",
    },
    KeyEntry {
        name: "z",
        key: Key::Z,
        tooltip: "argument.entity.options.z.description",
    },
    KeyEntry {
        name: "dx",
        key: Key::Dx,
        tooltip: "argument.entity.options.dx.description",
    },
    KeyEntry {
        name: "dy",
        key: Key::Dy,
        tooltip: "argument.entity.options.dy.description",
    },
    KeyEntry {
        name: "dz",
        key: Key::Dz,
        tooltip: "argument.entity.options.dz.description",
    },
    KeyEntry {
        name: "x_rotation",
        key: Key::XRotation,
        tooltip: "argument.entity.options.x_rotation.description",
    },
    KeyEntry {
        name: "y_rotation",
        key: Key::YRotation,
        tooltip: "argument.entity.options.y_rotation.description",
    },
    KeyEntry {
        name: "limit",
        key: Key::Limit,
        tooltip: "argument.entity.options.limit.description",
    },
    KeyEntry {
        name: "sort",
        key: Key::Sort,
        tooltip: "argument.entity.options.sort.description",
    },
    KeyEntry {
        name: "gamemode",
        key: Key::GameMode,
        tooltip: "argument.entity.options.gamemode.description",
    },
    KeyEntry {
        name: "team",
        key: Key::Team,
        tooltip: "argument.entity.options.team.description",
    },
    KeyEntry {
        name: "type",
        key: Key::Type,
        tooltip: "argument.entity.options.type.description",
    },
    KeyEntry {
        name: "tag",
        key: Key::Tag,
        tooltip: "argument.entity.options.tag.description",
    },
    KeyEntry {
        name: "nbt",
        key: Key::Nbt,
        tooltip: "argument.entity.options.nbt.description",
    },
    KeyEntry {
        name: "scores",
        key: Key::Scores,
        tooltip: "argument.entity.options.scores.description",
    },
    KeyEntry {
        name: "advancements",
        key: Key::Advancements,
        tooltip: "argument.entity.options.advancements.description",
    },
    KeyEntry {
        name: "predicate",
        key: Key::Predicate,
        tooltip: "argument.entity.options.predicate.description",
    },
];
