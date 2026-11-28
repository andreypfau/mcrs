use crate::source::Source;
use mcrs_command::Requirement;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandLevel {
    #[default]
    All,
    Moderators,
    Gamemasters,
    Admins,
    Owners,
}

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
