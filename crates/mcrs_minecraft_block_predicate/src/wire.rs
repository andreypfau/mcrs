use std::io::Write;

use mcrs_minecraft_wire::{Decode, Encode, ErrorKind, Result};

use crate::state_properties::{StatePropertiesPredicate, ValueMatcher};

impl Encode for StatePropertiesPredicate {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for StatePropertiesPredicate {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let properties: Vec<(String, ValueMatcher)> = Vec::decode(r)?;
        for (at, (name, _)) in properties.iter().enumerate() {
            if properties[..at].iter().any(|(seen, _)| seen == name) {
                return Err(ErrorKind::DuplicateKey.into());
            }
        }
        Ok(StatePropertiesPredicate(properties))
    }
}

impl Encode for ValueMatcher {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            ValueMatcher::Exact(value) => {
                true.encode(&mut w)?;
                value.encode(w)
            }
            ValueMatcher::Ranged { min, max } => {
                false.encode(&mut w)?;
                min.encode(&mut w)?;
                max.encode(w)
            }
        }
    }
}

impl Decode<'_> for ValueMatcher {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(match bool::decode(r)? {
            true => ValueMatcher::Exact(String::decode(r)?),
            false => ValueMatcher::Ranged {
                min: Option::decode(r)?,
                max: Option::decode(r)?,
            },
        })
    }
}
