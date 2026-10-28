pub(crate) use mcrs_command::is_whitespace;

pub(crate) const fn is_unquoted(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')
}

pub(crate) const fn is_number_start(c: char) -> bool {
    matches!(c, '+' | '-' | '.' | '0'..='9')
}

pub(crate) const fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | ' ')
}

pub(crate) const ESCAPE_LETTERS: [&str; 13] = [
    "b", "s", "t", "n", "f", "r", "\\", "'", "\"", "x", "u", "U", "N",
];

pub(crate) const fn simple_escape(letter: char) -> Option<char> {
    Some(match letter {
        'b' => '\u{8}',
        's' => ' ',
        't' => '\t',
        'n' => '\n',
        'f' => '\u{c}',
        'r' => '\r',
        '\\' | '\'' | '"' => letter,
        _ => return None,
    })
}

pub(crate) const fn hex_length(letter: char) -> Option<usize> {
    match letter {
        'x' => Some(2),
        'u' => Some(4),
        'U' => Some(8),
        _ => None,
    }
}

pub(crate) const fn is_high_surrogate(code: u32) -> bool {
    matches!(code, 0xd800..=0xdbff)
}

pub(crate) const fn is_low_surrogate(code: u32) -> bool {
    matches!(code, 0xdc00..=0xdfff)
}

/// `length` hex digits at the start of `text`, as a value.
pub(crate) fn hex_digits(text: &str, length: usize) -> Option<u32> {
    let digits = text.get(..length)?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

const LONGEST_NAME: usize = 128;

/// The code point whose Unicode Name property is `name`, ignoring the case of ASCII letters and
/// the spaces around it. Aliases and loose matches do not count.
pub(crate) fn char_by_name(name: &str) -> Option<char> {
    let name = name.trim_matches(' ');
    // unicode_names2 underflows an index on a leading `-`, and no name starts with one.
    if name.is_empty() || name.len() > LONGEST_NAME || name.starts_with('-') {
        return None;
    }
    let found = unicode_names2::character(name)?;
    let mut canonical = unicode_names2::name(found)?.flat_map(str::chars);
    let same = name
        .chars()
        .all(|c| canonical.next().is_some_and(|d| d.eq_ignore_ascii_case(&c)));
    (same && canonical.next().is_none()).then_some(found)
}

/// The text of a string tag as the read found it: a word, or the inside of a quoted string with
/// its escapes, which the read has already validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrText<'a> {
    Word(&'a str),
    Quoted(&'a str),
}

impl<'a> StrText<'a> {
    fn raw(self) -> &'a str {
        match self {
            StrText::Word(text) | StrText::Quoted(text) => text,
        }
    }

    pub fn is_empty(self) -> bool {
        self.raw().is_empty()
    }

    pub fn chars(self) -> impl Iterator<Item = char> + 'a {
        let (word, quoted) = match self {
            StrText::Word(text) => (text, ""),
            StrText::Quoted(text) => ("", text),
        };
        word.chars().chain(Unescape { rest: quoted })
    }

    pub(crate) fn len_hint(self) -> usize {
        self.raw().len()
    }
}

struct Unescape<'a> {
    rest: &'a str,
}

impl Unescape<'_> {
    fn bump(&mut self) -> Option<char> {
        let mut chars = self.rest.chars();
        let c = chars.next()?;
        self.rest = chars.as_str();
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        self.rest = self.rest.trim_start_matches(is_whitespace);
    }

    fn hex(&mut self, length: usize) -> Option<u32> {
        let value = hex_digits(self.rest, length)?;
        self.rest = self.rest.get(length..)?;
        Some(value)
    }

    fn escape(&mut self) -> Option<char> {
        self.skip_whitespace();
        let letter = self.bump()?;
        if let Some(c) = simple_escape(letter) {
            return Some(c);
        }
        if let Some(length) = hex_length(letter) {
            let code = self.hex(length)?;
            if !is_high_surrogate(code) {
                return char::from_u32(code);
            }
            self.bump()?;
            self.skip_whitespace();
            let length = hex_length(self.bump()?)?;
            let low = self.hex(length)?;
            return char::from_u32(0x10000 + ((code - 0xd800) << 10) + (low.checked_sub(0xdc00)?));
        }
        self.skip_whitespace();
        self.bump()?;
        let end = self.rest.find(|c| !is_name_char(c))?;
        let name = self.rest.get(..end)?;
        self.rest = self.rest.get(end..)?;
        self.skip_whitespace();
        self.bump()?;
        char_by_name(name)
    }
}

impl Iterator for Unescape<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self.bump()? {
            '\\' => self.escape(),
            c => Some(c),
        }
    }
}
