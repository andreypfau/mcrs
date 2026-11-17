use std::fmt;

use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize};

use mcrs_minecraft_core::codec::BoundedString;

use crate::ProfileError;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Property<S = String> {
    pub name: S,
    pub value: S,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<S>,
}

pub const MAX_PROPERTIES: usize = 16;

pub fn grouped_by_name(properties: Vec<Property>) -> Vec<Property> {
    let mut grouped: Vec<Property> = Vec::with_capacity(properties.len());
    for property in properties {
        let end = grouped
            .iter()
            .rposition(|p| p.name == property.name)
            .map_or(grouped.len(), |i| i + 1);
        grouped.insert(end, property);
    }
    grouped
}

pub(crate) fn is_grouped_by_name(properties: &[Property]) -> bool {
    (1..properties.len()).all(|end| match properties.split_at_checked(end) {
        Some((before, [next, ..])) => {
            before.last().is_some_and(|last| last.name == next.name)
                || before.iter().all(|property| property.name != next.name)
        }
        _ => true,
    })
}

pub(crate) fn check_property(property: &Property) -> Result<(), ProfileError<'_>> {
    BoundedString::<64>::check(&property.name)?;
    BoundedString::<32767>::check(&property.value)?;
    if let Some(signature) = &property.signature {
        BoundedString::<1024>::check(signature)?;
    }
    Ok(())
}

pub(crate) fn properties<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Property>, D::Error> {
    struct PropertiesVisitor;

    impl<'de> Visitor<'de> for PropertiesVisitor {
        type Value = Vec<Property>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of properties or a map of values by name")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
            let properties = Vec::<Property>::deserialize(value::SeqAccessDeserializer::new(seq))?;
            if properties.len() > MAX_PROPERTIES {
                return Err(A::Error::custom(ProfileError::TooManyProperties(
                    properties.len(),
                )));
            }
            for property in &properties {
                check_property(property).map_err(A::Error::custom)?;
            }
            Ok(grouped_by_name(properties))
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut properties = Vec::new();
            let mut names = 0;
            while let Some((name, values)) = map.next_entry::<String, Vec<String>>()? {
                names += 1;
                if names > MAX_PROPERTIES {
                    return Err(A::Error::custom(format_args!(
                        "Cannot have more than {MAX_PROPERTIES} properties, but was {names}"
                    )));
                }
                for value in values {
                    let property = Property {
                        name: name.clone(),
                        value,
                        signature: None,
                    };
                    check_property(&property).map_err(A::Error::custom)?;
                    properties.push(property);
                }
            }
            if properties.len() > MAX_PROPERTIES {
                return Err(A::Error::custom(ProfileError::TooManyProperties(
                    properties.len(),
                )));
            }
            Ok(grouped_by_name(properties))
        }
    }

    d.deserialize_any(PropertiesVisitor)
}
