use std::fmt;

/// A JSON value that keeps the order of object members and the text of numbers.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<Member>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub key: String,
    pub value: Json,
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        let Json::Object(members) = self else {
            return None;
        };
        members
            .iter()
            .find(|member| member.key == key)
            .map(|member| &member.value)
    }

    pub fn members(&self) -> &[Member] {
        match self {
            Json::Object(members) => members,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: &'static str,
}

impl fmt::Display for JsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for JsonError {}

/// Parses JSON with the extensions that pack files use: a byte order mark, `//` and `/* */`
/// comments, and a trailing comma before `]` or `}`.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        pos: 0,
    };
    if reader.bytes.starts_with("\u{feff}".as_bytes()) {
        reader.pos = 3;
    }
    let value = reader.value()?;
    reader.skip_trivia()?;
    if reader.pos != reader.bytes.len() {
        return Err(reader.error("text after the value"));
    }
    Ok(value)
}

struct Reader<'text> {
    bytes: &'text [u8],
    pos: usize,
}

impl Reader<'_> {
    fn error(&self, message: &'static str) -> JsonError {
        JsonError {
            offset: self.pos,
            message,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.pos + ahead).copied()
    }

    fn skip_trivia(&mut self) -> Result<(), JsonError> {
        loop {
            match (self.peek(), self.peek_at(1)) {
                (Some(b' ' | b'\t' | b'\r' | b'\n'), _) => self.pos += 1,
                (Some(b'/'), Some(b'/')) => {
                    while self.peek().is_some_and(|byte| byte != b'\n') {
                        self.pos += 1;
                    }
                }
                (Some(b'/'), Some(b'*')) => {
                    self.pos += 2;
                    while !(self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/')) {
                        if self.peek().is_none() {
                            return Err(self.error("unterminated comment"));
                        }
                        self.pos += 1;
                    }
                    self.pos += 2;
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        self.skip_trivia()?;
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::String),
            Some(b'-' | b'0'..=b'9') => Ok(self.number()),
            Some(b't') => self.keyword("true", Json::Bool(true)),
            Some(b'f') => self.keyword("false", Json::Bool(false)),
            Some(b'n') => self.keyword("null", Json::Null),
            _ => Err(self.error("expected a value")),
        }
    }

    fn keyword(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self
            .bytes
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(word.as_bytes()))
        {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("unknown keyword"))
        }
    }

    fn number(&mut self) -> Json {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|byte| matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.pos += 1;
        }
        let text = self.bytes.get(start..self.pos).unwrap_or_default();
        Json::Number(String::from_utf8_lossy(text).into_owned())
    }

    fn array(&mut self) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut items = Vec::new();
        loop {
            self.skip_trivia()?;
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Json::Array(items));
            }
            items.push(self.value()?);
            self.skip_trivia()?;
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {}
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, JsonError> {
        self.pos += 1;
        let mut members = Vec::new();
        loop {
            self.skip_trivia()?;
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(members));
                }
                Some(b'"') => {}
                _ => return Err(self.error("expected a key")),
            }
            let key = self.string()?;
            self.skip_trivia()?;
            if self.peek() != Some(b':') {
                return Err(self.error("expected `:`"));
            }
            self.pos += 1;
            let value = self.value()?;
            members.push(Member { key, value });
            self.skip_trivia()?;
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {}
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.pos += 1;
        let mut bytes = Vec::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("unterminated string"));
            };
            self.pos += 1;
            match byte {
                b'"' => break,
                b'\\' => self.escape(&mut bytes)?,
                _ => bytes.push(byte),
            }
        }
        String::from_utf8(bytes).map_err(|_| self.error("string is not UTF-8"))
    }

    fn escape(&mut self, bytes: &mut Vec<u8>) -> Result<(), JsonError> {
        let Some(byte) = self.peek() else {
            return Err(self.error("unterminated escape"));
        };
        self.pos += 1;
        let decoded = match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => self.unicode_escape()?,
            _ => return Err(self.error("unknown escape")),
        };
        let mut buffer = [0; 4];
        bytes.extend_from_slice(decoded.encode_utf8(&mut buffer).as_bytes());
        Ok(())
    }

    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let high = self.hex4()?;
        if (0xd800..0xdc00).contains(&high) && self.peek() == Some(b'\\') {
            self.pos += 2;
            let low = self.hex4()?;
            let code = 0x10000 + ((high - 0xd800) << 10) + (low.wrapping_sub(0xdc00) & 0x3ff);
            return Ok(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
        }
        Ok(char::from_u32(high).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.error("short unicode escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.error("bad unicode escape"))?;
        let value = u32::from_str_radix(text, 16).map_err(|_| self.error("bad unicode escape"))?;
        self.pos += 4;
        Ok(value)
    }
}
