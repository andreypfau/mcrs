use core::fmt;

use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;

use crate::context_param::ContextParam;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamNotAllowed {
    pub param: ContextParam,
    pub key_set: ContextKeySet,
}

impl fmt::Display for ParamNotAllowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the parameter {} is not allowed in the key set {}",
            self.param,
            self.key_set.as_static_str()
        )
    }
}

impl core::error::Error for ParamNotAllowed {}
