use crate::signing::Signing;

/// What a send carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outgoing {
    Chat(String),
    /// The command line without its `/`.
    Command {
        line: String,
        signing: Signing,
    },
}
