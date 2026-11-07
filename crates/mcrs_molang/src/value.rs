use core::fmt;

/// The static type of a Molang value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// An `f32`.
    Number,
    /// A string, carried as its [`string_hash`].
    String,
    /// A resource of a render controller, carried as [`Resource::to_bits`].
    Resource,
}

impl ValueType {
    /// Whether the value lives in the 64-bit register bank rather than the `f32` bank.
    #[must_use]
    pub const fn is_bits(self) -> bool {
        !matches!(self, Self::Number)
    }
}

/// The kind of a resource reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Geometry,
    Material,
    Texture,
}

/// A resource that a render controller selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resource {
    pub kind: ResourceKind,
    pub id: u32,
}

impl Resource {
    #[must_use]
    pub const fn to_bits(self) -> u64 {
        let kind = match self.kind {
            ResourceKind::Geometry => 0u64,
            ResourceKind::Material => 1,
            ResourceKind::Texture => 2,
        };
        (kind << 32) | self.id as u64
    }

    #[must_use]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        let kind = match bits >> 32 {
            0 => ResourceKind::Geometry,
            1 => ResourceKind::Material,
            2 => ResourceKind::Texture,
            _ => return None,
        };
        Some(Self {
            kind,
            id: bits as u32,
        })
    }
}

/// The result of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Number(f32),
    /// The [`string_hash`] of the string.
    String(u64),
    Resource(Resource),
}

impl Value {
    #[must_use]
    pub const fn value_type(self) -> ValueType {
        match self {
            Self::Number(_) => ValueType::Number,
            Self::String(_) => ValueType::String,
            Self::Resource(_) => ValueType::Resource,
        }
    }

    /// The number, or `0.0` for a value of another type.
    #[must_use]
    pub const fn as_number(self) -> f32 {
        match self {
            Self::Number(number) => number,
            Self::String(_) | Self::Resource(_) => 0.0,
        }
    }

    #[must_use]
    pub(crate) fn from_raw(value_type: ValueType, raw: u64) -> Self {
        match value_type {
            ValueType::Number => Self::Number(f32::from_bits(raw as u32)),
            ValueType::String => Self::String(raw),
            ValueType::Resource => match Resource::from_bits(raw) {
                Some(resource) => Self::Resource(resource),
                None => Self::Number(0.0),
            },
        }
    }

    /// The value in the raw form of its register bank.
    #[must_use]
    pub const fn to_raw(self) -> u64 {
        match self {
            Self::Number(number) => number.to_bits() as u64,
            Self::String(hash) => hash,
            Self::Resource(resource) => resource.to_bits(),
        }
    }
}

/// The 64-bit FNV-1a hash of a string. Two strings are equal when their hashes are equal.
#[must_use]
pub fn string_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// How often a value can change, from the slowest to the fastest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rate {
    Constant,
    Load,
    Frame,
    EntityTick,
    EntityFrame,
    AnimationFrame,
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Constant => "constant",
            Self::Load => "load",
            Self::Frame => "frame",
            Self::EntityTick => "entity_tick",
            Self::EntityFrame => "entity_frame",
            Self::AnimationFrame => "animation_frame",
        })
    }
}

/// The implementation milestone that the compiler targets. A construct of a later milestone
/// parses and gives the load error `unsupported`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Milestone {
    One,
    #[default]
    Two,
    Later,
}
