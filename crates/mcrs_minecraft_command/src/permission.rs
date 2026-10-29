use crate::source::Source;
use mcrs_command::Requirement;

/// The five ordered command levels (PERM-LEVEL-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandLevel {
    #[default]
    All,
    Moderators,
    Gamemasters,
    Admins,
    Owners,
}

impl CommandLevel {
    /// A level above `owners` is `owners`.
    pub const fn from_id(id: u8) -> Self {
        match id {
            0 => CommandLevel::All,
            1 => CommandLevel::Moderators,
            2 => CommandLevel::Gamemasters,
            3 => CommandLevel::Admins,
            _ => CommandLevel::Owners,
        }
    }

    /// The source holds `minecraft:commands/entity_selectors`, so it may use selectors that
    /// start with `@` (PERM-SET-001).
    pub fn can_use_selectors(self) -> bool {
        self >= CommandLevel::Gamemasters
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PermissionCheck {
    #[default]
    AlwaysPass,
    Require(CommandLevel),
}

impl PermissionCheck {
    /// The node carries the restricted flag of the `commands` packet (PERM-SET-002).
    pub const fn is_restricted(&self) -> bool {
        matches!(self, PermissionCheck::Require(_))
    }

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
