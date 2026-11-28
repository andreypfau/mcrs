use std::fmt;

use crate::fields::Fields;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub container: i32,
    pub state: i32,
    pub cursor: String,
    pub slots: Vec<String>,
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.container, self.state, self.cursor)?;
        for stack in &self.slots {
            write!(f, " {stack}")?;
        }
        Ok(())
    }
}

impl Content {
    pub(crate) fn parse(fields: &mut Fields<'_>) -> Result<Self, String> {
        let container = fields.value("container")?;
        let state = fields.value("state")?;
        let cursor = fields.word("cursor")?.to_owned();
        let mut slots = Vec::new();
        while let Some(stack) = fields.next_word() {
            slots.push(stack.to_owned());
        }
        Ok(Self {
            container,
            state,
            cursor,
            slots,
        })
    }
}
