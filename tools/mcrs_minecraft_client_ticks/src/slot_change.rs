use std::fmt;

use crate::fields::Fields;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotChange {
    pub container: i32,
    pub state: i32,
    pub slot: i16,
    pub stack: String,
}

impl fmt::Display for SlotChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.container, self.state, self.slot, self.stack
        )
    }
}

impl SlotChange {
    pub(crate) fn parse(fields: &mut Fields<'_>) -> Result<Self, String> {
        Ok(Self {
            container: fields.value("container")?,
            state: fields.value("state")?,
            slot: fields.value("slot")?,
            stack: fields.word("stack")?.to_owned(),
        })
    }
}
