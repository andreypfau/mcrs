use mcrs_minecraft_protocol::text::{Color, IntoText, Text};

/// The result of a command: its number, or the error the source receives.
pub type CommandResult = Result<i32, CommandFailure>;

/// Why a command failed: the message the source receives, in red.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandFailure(pub Text);

impl CommandFailure {
    /// A failure with the translation `key` and its arguments `with`.
    pub fn translate(key: &'static str, with: Vec<Text>) -> Self {
        Self(Text::translate(key, with))
    }

    /// The failure the game reports when a command fails without a reason of its own.
    pub fn failed() -> Self {
        Self::translate("command.failed", vec![])
    }

    pub(crate) fn text(self) -> Text {
        self.0.color(Color::RED)
    }
}
