use std::str::FromStr;
use std::str::SplitWhitespace;

use crate::ParseErrorKind;

/// The space-separated fields of one line, read in order.
pub(crate) struct Fields<'a> {
    rest: SplitWhitespace<'a>,
}

impl<'a> Fields<'a> {
    pub(crate) fn new(line: &'a str) -> Self {
        Self {
            rest: line.split_whitespace(),
        }
    }

    pub(crate) fn word(&mut self, field: &'static str) -> Result<&'a str, ParseErrorKind> {
        self.rest.next().ok_or(ParseErrorKind::MissingField(field))
    }

    pub(crate) fn next_word(&mut self) -> Option<&'a str> {
        self.rest.next()
    }

    pub(crate) fn value<T: FromStr>(&mut self, field: &'static str) -> Result<T, ParseErrorKind> {
        let word = self.word(field)?;
        parse_value(field, word)
    }

    pub(crate) fn array3<T: FromStr>(
        &mut self,
        field: &'static str,
    ) -> Result<[T; 3], ParseErrorKind> {
        Ok([self.value(field)?, self.value(field)?, self.value(field)?])
    }

    /// The rest of the line, with single spaces between its words.
    pub(crate) fn rest_text(&mut self) -> String {
        let words: Vec<&str> = self.rest.by_ref().collect();
        words.join(" ")
    }

    pub(crate) fn end(mut self) -> Result<(), ParseErrorKind> {
        match self.rest.next() {
            Some(extra) => Err(ParseErrorKind::ExtraField(extra.to_owned())),
            None => Ok(()),
        }
    }
}

pub(crate) fn parse_value<T: FromStr>(
    field: &'static str,
    word: &str,
) -> Result<T, ParseErrorKind> {
    word.parse().map_err(|_| bad(field, word))
}

pub(crate) fn bad(field: &'static str, value: &str) -> ParseErrorKind {
    ParseErrorKind::BadField {
        field,
        value: value.to_owned(),
    }
}
