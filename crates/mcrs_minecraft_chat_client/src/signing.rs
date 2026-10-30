/// Whether a command is sent signed or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signing {
    Unsigned,
    /// The command line holds a message argument, so it goes as `chat_command_signed`.
    Signed,
}
