use crate::attribute_keys::Attribute;
use crate::attribute_mask::AttributeMask;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeBases {
    set: AttributeMask,
    differing: &'static [TypeBase],
}

impl AttributeBases {
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

    pub fn attributes(self) -> impl Iterator<Item = Attribute> {
        self.set.iter()
    }

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeBase {
    pub attribute: Attribute,
    pub base: f64,
}
