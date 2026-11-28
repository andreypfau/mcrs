use crate::{BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct Experiment<'a> {
    pub name: &'a str,
    pub enabled: bool,
}
