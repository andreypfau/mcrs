use core::error::Error;
use core::fmt;

use crate::font_error::FontError;
use crate::font_notice::FontNotice;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontLoadFailure {
    pub error: FontError,
    pub notices: Vec<FontNotice>,
}

impl fmt::Display for FontLoadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for FontLoadFailure {}
