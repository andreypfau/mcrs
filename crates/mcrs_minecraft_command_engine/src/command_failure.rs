use mcrs_minecraft_protocol::text::{Color, IntoText, Text};

/// The result of a command: its number, or the error the source receives.
pub type CommandResult = Result<i32, CommandFailure>;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandFailure(pub Text);

impl CommandFailure {
    pub fn translate(key: &'static str, with: Vec<Text>) -> Self {
        Self(Text::translate(key, with))
    }

    pub fn failed() -> Self {
        Self::translate("command.failed", vec![])
    }

    pub(crate) fn text(self) -> Text {
        self.0.color(Color::RED)
    }
}
