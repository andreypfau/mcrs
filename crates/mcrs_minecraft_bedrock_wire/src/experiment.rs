use crate::{BedrockDecode, BedrockEncode};

/// A named toggle of the world that turns on game content that is not final: a `string` name,
/// then a `bool`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct Experiment<'a> {
    /// The name, for example `gametest`.
    pub name: &'a str,
    /// The experiment is on.
    pub enabled: bool,
}
