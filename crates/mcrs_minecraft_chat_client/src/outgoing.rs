use crate::signing::Signing;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outgoing {
    Chat(String),
    Command { line: String, signing: Signing },
}
