use std::str::FromStr;
use std::str::SplitWhitespace;

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

    pub(crate) fn word(&mut self, field: &str) -> Result<&'a str, String> {
        self.rest
            .next()
            .ok_or_else(|| format!("the line ends before `{field}`"))
    }

    pub(crate) fn next_word(&mut self) -> Option<&'a str> {
        self.rest.next()
    }

    pub(crate) fn value<T: FromStr>(&mut self, field: &str) -> Result<T, String> {
        let word = self.word(field)?;
        parse_value(field, word)
    }

    pub(crate) fn array3<T: FromStr>(&mut self, field: &str) -> Result<[T; 3], String> {
        Ok([self.value(field)?, self.value(field)?, self.value(field)?])
    }

    pub(crate) fn end(mut self) -> Result<(), String> {
        match self.rest.next() {
            Some(extra) => Err(format!("extra field `{extra}`")),
            None => Ok(()),
        }
    }
}

pub(crate) fn parse_value<T: FromStr>(field: &str, word: &str) -> Result<T, String> {
    word.parse().map_err(|_| bad(field, word))
}

pub(crate) fn bad(field: &str, value: &str) -> String {
    format!("`{field}` has the bad value `{value}`")
}
