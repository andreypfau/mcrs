use mcrs_minecraft_block::definition::schema::BlockProperties;

/// The properties of a block, each with the text of each of its values, in the order of the
/// block definition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyTable {
    pub properties: Vec<PropertyDomain>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyDomain {
    pub name: String,
    pub values: Vec<String>,
}

impl PropertyDomain {
    /// The index of the value that `text` names. An integer property also accepts a decimal
    /// integer with a sign or leading zeros.
    #[must_use]
    pub fn value_index(&self, text: &str) -> Option<usize> {
        if let Some(index) = self.values.iter().position(|value| value == text) {
            return Some(index);
        }
        let number: i64 = text.parse().ok()?;
        self.values
            .iter()
            .position(|value| value.parse::<i64>().ok() == Some(number))
    }
}

impl PropertyTable {
    #[must_use]
    pub fn of_block(properties: &BlockProperties) -> Self {
        Self {
            properties: properties
                .0
                .iter()
                .map(|property| PropertyDomain {
                    name: property.name.to_string(),
                    values: property
                        .values
                        .iter()
                        .map(|value| value.to_text())
                        .collect(),
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.properties
            .iter()
            .position(|property| property.name == name)
    }

    /// The value index of each property for the state with these property values, or `None`
    /// when a property has no value or an unknown one.
    #[must_use]
    pub fn state(&self, values: &[(&str, &str)]) -> Option<Vec<usize>> {
        self.properties
            .iter()
            .map(|property| {
                let (_, text) = values.iter().find(|(name, _)| *name == property.name)?;
                property.value_index(text)
            })
            .collect()
    }

    /// The text of the value of `name` in `state`.
    #[must_use]
    pub fn value_of<'a>(&'a self, state: &[usize], name: &str) -> Option<&'a str> {
        let index = self.index_of(name)?;
        let property = self.properties.get(index)?;
        property.values.get(*state.get(index)?).map(String::as_str)
    }
}
