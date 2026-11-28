use std::fmt;

use crate::fields::Fields;
use crate::fields::bad;
use crate::fields::parse_value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Click {
    pub container: i32,
    pub state: i32,
    pub slot: i16,
    pub button: u8,
    pub input: String,
    pub cursor: String,
    pub changed: Vec<ChangedSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedSlot {
    pub slot: u16,
    pub stack: String,
}

impl fmt::Display for Click {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {}",
            self.container, self.state, self.slot, self.button, self.input, self.cursor
        )?;
        for changed in &self.changed {
            write!(f, " {}={}", changed.slot, changed.stack)?;
        }
        Ok(())
    }
}

impl Click {
    pub(crate) fn parse(fields: &mut Fields<'_>) -> Result<Self, String> {
        let container = fields.value("container")?;
        let state = fields.value("state")?;
        let slot = fields.value("slot")?;
        let button = fields.value("button")?;
        let input = fields.word("input")?.to_owned();
        let cursor = fields.word("cursor")?.to_owned();
        let mut changed = Vec::new();
        while let Some(word) = fields.next_word() {
            let (slot, stack) = word.split_once('=').ok_or_else(|| bad("changed", word))?;
            changed.push(ChangedSlot {
                slot: parse_value("changed", slot)?,
                stack: stack.to_owned(),
            });
        }
        Ok(Self {
            container,
            state,
            slot,
            button,
            input,
            cursor,
            changed,
        })
    }
}
