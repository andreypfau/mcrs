use crate::text::Text;
use crate::{Decode, Encode};
use std::borrow::Cow;

/// One suggestion of a `command_suggestions` reply, with its tooltip.
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub struct CommandSuggestion<'a> {
    pub text: Cow<'a, str>,
    pub tooltip: Option<Text>,
}
