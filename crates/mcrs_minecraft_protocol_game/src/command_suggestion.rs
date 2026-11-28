use crate::text::RawText;
use mcrs_minecraft_protocol::{Decode, Encode};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct CommandSuggestion<'a> {
    pub text: Cow<'a, str>,
    pub tooltip: Option<RawText<'a>>,
}
