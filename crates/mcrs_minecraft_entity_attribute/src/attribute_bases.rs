use crate::attribute_keys::Attribute;
use crate::attribute_mask::AttributeMask;

/// The attribute set of a living entity type and the base of the type of each attribute in it
/// (`attributes.md`, ATR-BASE-001 to ATR-BASE-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeBases {
    set: AttributeMask,
    differing: &'static [TypeBase],
}

impl AttributeBases {
    /// The base of the type of `attribute`, or `None` when the attribute is not in the set.
    #[must_use]
    pub fn base(self, attribute: Attribute) -> Option<f64> {
        if !self.set.contains(attribute) {
            return None;
        }
        let differing = self
            .differing
            .iter()
            .find(|entry| entry.attribute == attribute);
        Some(differing.map_or(attribute.definition().default, |entry| entry.base))
    }

    /// The attributes of the set, in the order of their numeric ids.
    pub fn attributes(self) -> impl Iterator<Item = Attribute> {
        self.set.iter()
    }

    /// The bases of a type whose set is the set of every living type and `more`. An attribute of
    /// the set has the base of its entry in `differing`, or else its default.
    #[must_use]
    pub const fn new(more: &[Attribute], differing: &'static [TypeBase]) -> Self {
        let mut set = AttributeMask::COMMON;
        let mut rest = more;
        while let [attribute, tail @ ..] = rest {
            set = set.with(*attribute);
            rest = tail;
        }
        Self { set, differing }
    }
}

/// A base of a type that differs from the attribute default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeBase {
    /// The attribute of the base.
    pub attribute: Attribute,
    /// The base value of the attribute on an entity of the type.
    pub base: f64,
}
