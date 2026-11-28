use alloc::string::String;
use alloc::vec::Vec;
use core::str::FromStr;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_snbt::read_compound;

use crate::nbt_path::NbtPath;
use crate::nbt_path_error::NbtPathError;
use crate::node::Node;

impl FromStr for NbtPath {
    type Err = NbtPathError;

    fn from_str(text: &str) -> Result<NbtPath, NbtPathError> {
        let mut parser = Parser { text, position: 0 };
        let mut nodes: Vec<Node> = Vec::new();
        let mut ends: Vec<usize> = Vec::new();
        nodes.push(parser.first()?);
        ends.push(parser.position);
        while parser.peek().is_some() {
            nodes.push(parser.next()?);
            ends.push(parser.position);
        }
        Ok(NbtPath::new(nodes, ends))
    }
}

struct Parser<'a> {
    text: &'a str,
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.text.get(self.position..)?.chars().next()
    }

    fn advance(&mut self, character: char) {
        self.position = self.position.saturating_add(character.len_utf8());
    }

    fn first(&mut self) -> Result<Node, NbtPathError> {
        match self.peek() {
            Some('{') => Ok(Node::Root(self.pattern()?)),
            Some('[') => self.index_node(),
            _ => self.name_node(),
        }
    }

    fn next(&mut self) -> Result<Node, NbtPathError> {
        match self.peek() {
            Some('[') => self.index_node(),
            Some('.') => {
                self.advance('.');
                match self.peek() {
                    Some('[') => self.index_node(),
                    _ => self.name_node(),
                }
            }
            _ => Err(NbtPathError::ExpectedSeparator {
                position: self.position,
            }),
        }
    }

    fn name_node(&mut self) -> Result<Node, NbtPathError> {
        let name = match self.peek() {
            Some(quote @ ('"' | '\'')) => {
                let start = self.position;
                self.advance(quote);
                let name = self.quoted(quote)?;
                if name.is_empty() {
                    return Err(NbtPathError::EmptyName { position: start });
                }
                name
            }
            _ => self.unquoted()?,
        };
        if self.peek() == Some('{') {
            let pattern = self.pattern()?;
            Ok(Node::NamePattern { name, pattern })
        } else {
            Ok(Node::Name(name))
        }
    }

    fn unquoted(&mut self) -> Result<String, NbtPathError> {
        let start = self.position;
        while let Some(character) = self.peek().filter(|character| is_unquoted(*character)) {
            self.advance(character);
        }
        match self.text.get(start..self.position) {
            Some(name) if !name.is_empty() => Ok(String::from(name)),
            _ => Err(NbtPathError::ExpectedNode { position: start }),
        }
    }

    fn quoted(&mut self, quote: char) -> Result<String, NbtPathError> {
        let mut name = String::new();
        loop {
            let Some(character) = self.peek() else {
                return Err(NbtPathError::UnterminatedQuote {
                    position: self.position,
                });
            };
            self.advance(character);
            if character == quote {
                return Ok(name);
            }
            if character != '\\' {
                name.push(character);
                continue;
            }
            match self.peek() {
                Some(escaped) if escaped == quote || escaped == '\\' => {
                    self.advance(escaped);
                    name.push(escaped);
                }
                Some(_) => {
                    return Err(NbtPathError::InvalidEscape {
                        position: self.position,
                    });
                }
                None => {
                    return Err(NbtPathError::UnterminatedQuote {
                        position: self.position,
                    });
                }
            }
        }
    }

    fn index_node(&mut self) -> Result<Node, NbtPathError> {
        self.advance('[');
        let node = match self.peek() {
            Some(']') => Node::All,
            Some('{') => Node::ElementPattern(self.pattern()?),
            Some('-' | '.' | '0'..='9') => Node::Index(self.integer()?),
            _ => {
                return Err(NbtPathError::ExpectedIndex {
                    position: self.position,
                });
            }
        };
        if self.peek() != Some(']') {
            return Err(NbtPathError::ExpectedClosingBracket {
                position: self.position,
            });
        }
        self.advance(']');
        Ok(node)
    }

    fn integer(&mut self) -> Result<i32, NbtPathError> {
        let start = self.position;
        while let Some(character) = self
            .peek()
            .filter(|character| matches!(character, '-' | '.' | '0'..='9'))
        {
            self.advance(character);
        }
        let run = self.text.get(start..self.position).unwrap_or_default();
        let digits = run.strip_prefix('-').unwrap_or(run);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(NbtPathError::ExpectedIndex { position: start });
        }
        run.parse()
            .map_err(|_| NbtPathError::IndexOutOfRange { position: start })
    }

    fn pattern(&mut self) -> Result<NbtCompound, NbtPathError> {
        let (pattern, end) =
            read_compound(self.text, self.position).map_err(NbtPathError::Pattern)?;
        self.position = end;
        Ok(pattern)
    }
}

fn is_unquoted(character: char) -> bool {
    !matches!(character, ' ' | '"' | '\'' | '[' | ']' | '.' | '{' | '}')
}
