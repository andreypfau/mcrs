use core::fmt;
use mcrs_minecraft_text::{Color, IntoText, ItemHover, Text, TranslateArg};

pub type CommandResult = Result<i32, CommandFailure>;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandFailure(pub Text);

impl fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for CommandFailure {}

impl CommandFailure {
    pub fn translate(key: &'static str, with: Vec<TranslateArg<ItemHover>>) -> Self {
        Self(Text::translate_args(key, with))
    }

    pub fn failed() -> Self {
        Self::translate("command.failed", vec![])
    }

    pub(crate) fn text(self) -> Text {
        self.0.color(Color::RED)
    }
}
