use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Syntax {
    line: bool,
    nested: bool,
    quotes: &'static [u8],
    triple: bool,
    script: bool,
}

pub const SHADER: Syntax = Syntax {
    line: true,
    nested: true,
    quotes: b"\"",
    triple: false,
    script: false,
};

pub const JAVA: Syntax = Syntax {
    line: true,
    nested: false,
    quotes: b"\"'",
    triple: true,
    script: false,
};

pub const KOTLIN: Syntax = Syntax {
    nested: true,
    ..JAVA
};

pub const CSS: Syntax = Syntax {
    line: false,
    nested: false,
    quotes: b"\"'",
    triple: false,
    script: false,
};

pub const SCRIPT: Syntax = Syntax {
    line: true,
    nested: false,
    quotes: b"\"'",
    triple: false,
    script: true,
};

const KEYWORDS_BEFORE_VALUE: [&[u8]; 13] = [
    b"return", b"typeof", b"case", b"do", b"else", b"in", b"of", b"void", b"new", b"delete",
    b"throw", b"yield", b"await",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Previous {
    Operator,
    Value,
}

struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
    syntax: Syntax,
    found: Vec<Range<usize>>,
}

pub fn comments(text: &str, syntax: Syntax) -> Vec<Range<usize>> {
    let mut scanner = Scanner::new(text, syntax);
    scanner.code(false);
    scanner.found
}

pub fn expression(text: &str) -> (Vec<Range<usize>>, usize) {
    let mut scanner = Scanner::new(text, SCRIPT);
    scanner.code(true);
    (scanner.found, scanner.at)
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str, syntax: Syntax) -> Self {
        Scanner {
            bytes: text.as_bytes(),
            at: 0,
            syntax,
            found: Vec::new(),
        }
    }

    fn peek(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.at + ahead).copied()
    }

    fn code(&mut self, bounded: bool) {
        let mut depth = 0usize;
        let mut previous = Previous::Operator;
        while let Some(byte) = self.peek(0) {
            match byte {
                b'/' if self.syntax.line && self.peek(1) == Some(b'/') => {
                    self.line_comment();
                    continue;
                }
                b'/' if self.peek(1) == Some(b'*') => {
                    self.block_comment();
                    continue;
                }
                b'/' if self.syntax.script
                    && previous == Previous::Operator
                    && self.at.checked_sub(1).and_then(|i| self.bytes.get(i)) != Some(&b'<') =>
                {
                    self.regex();
                    previous = Previous::Value;
                    continue;
                }
                b'`' if self.syntax.script => {
                    self.template();
                    previous = Previous::Value;
                    continue;
                }
                quote if self.syntax.quotes.contains(&quote) => {
                    self.string(quote);
                    previous = Previous::Value;
                    continue;
                }
                b'{' => depth += 1,
                b'}' if depth == 0 && bounded => return,
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric() {
                let start = self.at;
                while self
                    .peek(0)
                    .is_some_and(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric())
                {
                    self.at += 1;
                }
                let word = self.bytes.get(start..self.at).unwrap_or_default();
                previous = if KEYWORDS_BEFORE_VALUE.contains(&word) {
                    Previous::Operator
                } else {
                    Previous::Value
                };
                continue;
            }
            if !byte.is_ascii_whitespace() {
                previous = if b")]}".contains(&byte) {
                    Previous::Value
                } else {
                    Previous::Operator
                };
            }
            self.at += 1;
        }
    }

    fn line_comment(&mut self) {
        let start = self.at;
        while self.peek(0).is_some_and(|b| b != b'\n') {
            self.at += 1;
        }
        let end = if self.bytes.get(self.at - 1) == Some(&b'\r') {
            self.at - 1
        } else {
            self.at
        };
        self.found.push(start..end);
    }

    fn block_comment(&mut self) {
        let start = self.at;
        self.at += 2;
        let mut depth = 1usize;
        while let Some(byte) = self.peek(0) {
            if self.syntax.nested && byte == b'/' && self.peek(1) == Some(b'*') {
                depth += 1;
                self.at += 2;
            } else if byte == b'*' && self.peek(1) == Some(b'/') {
                self.at += 2;
                depth -= 1;
                if depth == 0 {
                    break;
                }
            } else {
                self.at += 1;
            }
        }
        self.found.push(start..self.at.min(self.bytes.len()));
    }

    fn string(&mut self, quote: u8) {
        if self.syntax.triple && self.peek(1) == Some(quote) && self.peek(2) == Some(quote) {
            self.at += 3;
            while let Some(byte) = self.peek(0) {
                if byte == b'\\' {
                    self.at += 2;
                } else if byte == quote
                    && self.peek(1) == Some(quote)
                    && self.peek(2) == Some(quote)
                {
                    self.at += 3;
                    return;
                } else {
                    self.at += 1;
                }
            }
            return;
        }
        self.at += 1;
        while let Some(byte) = self.peek(0) {
            match byte {
                b'\\' => self.at += 2,
                b'\n' => return,
                _ if byte == quote => {
                    self.at += 1;
                    return;
                }
                _ => self.at += 1,
            }
        }
    }

    fn regex(&mut self) {
        self.at += 1;
        let mut class = false;
        while let Some(byte) = self.peek(0) {
            match byte {
                b'\\' => self.at += 2,
                b'\n' => return,
                b'[' => {
                    class = true;
                    self.at += 1;
                }
                b']' => {
                    class = false;
                    self.at += 1;
                }
                b'/' if !class => {
                    self.at += 1;
                    return;
                }
                _ => self.at += 1,
            }
        }
    }

    fn template(&mut self) {
        self.at += 1;
        while let Some(byte) = self.peek(0) {
            match byte {
                b'\\' => self.at += 2,
                b'`' => {
                    self.at += 1;
                    return;
                }
                b'$' if self.peek(1) == Some(b'{') => {
                    self.at += 2;
                    self.code(true);
                    self.at += 1;
                }
                _ => self.at += 1,
            }
        }
    }
}
