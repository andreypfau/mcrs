use crate::source::Source;
use mcrs_command::Requirement;

/// The five ordered command levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandLevel {
    #[default]
    All,
    Moderators,
    Gamemasters,
    Admins,
    Owners,
}

/// A level above `owners` is `owners`.
impl From<u8> for CommandLevel {
    fn from(id: u8) -> Self {
        match id {
            0 => CommandLevel::All,
            1 => CommandLevel::Moderators,
            2 => CommandLevel::Gamemasters,
            3 => CommandLevel::Admins,
            _ => CommandLevel::Owners,
        }
    }
}

impl CommandLevel {
    /// The source holds `minecraft:commands/entity_selectors`, so it may use selectors that
    /// start with `@`.
    pub fn can_use_selectors(self) -> bool {
        self >= CommandLevel::Gamemasters
    }
}

/// The requirement of a command node on the level of its source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermissionCheck {
    /// Every source passes.
    #[default]
    AlwaysPass,
    /// A source at this level or above passes.
    Require(CommandLevel),
}

impl PermissionCheck {
    /// The node carries the restricted flag of the `commands` packet.
    pub const fn is_restricted(&self) -> bool {
        matches!(self, PermissionCheck::Require(_))
    }

    /// Whether a source at `level` passes the check.
    pub fn passes(&self, level: CommandLevel) -> bool {
        match *self {
            PermissionCheck::AlwaysPass => true,
            PermissionCheck::Require(required) => level >= required,
        }
    }
}

impl<L> Requirement<Source<'_, L>> for PermissionCheck {
    fn test(&self, source: &Source<'_, L>) -> bool {
        self.passes(source.level)
    }
}
