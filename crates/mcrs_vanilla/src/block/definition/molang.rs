//! The Molang subset the block definition corpus writes in its permutation
//! conditions, and nothing else. Conditions are compiled against a block's
//! property table at load and evaluated once per state; no expression survives
//! into a tick.

use super::schema::{BlockProperties, PropertyValue};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MolangError {
    #[error("unexpected token `{0}`")]
    UnexpectedToken(String),
    #[error("unexpected end of condition")]
    UnexpectedEnd,
    #[error("unterminated string literal `{0}`")]
    UnterminatedString(String),
    #[error("expected `==` or `!=`, found `{0}`")]
    ExpectedComparison(String),
    #[error("expected a comparison between a block state and a literal")]
    NotAComparison,
    #[error("unknown block state property `{0}`")]
    UnknownProperty(String),
    #[error("`{value}` is not a value of block state property `{property}`")]
    UnknownValue { property: String, value: String },
    #[error("trailing input at `{0}`")]
    TrailingInput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateCondition {
    Compares { property: u8, value: u8, equal: bool },
    Not(Box<StateCondition>),
    And(Box<(StateCondition, StateCondition)>),
    Or(Box<(StateCondition, StateCondition)>),
}

impl StateCondition {
    /// `values[i]` is the index, within property `i`'s value list, that the
    /// state under test holds.
    pub fn matches(&self, values: &[u8]) -> bool {
        match self {
            StateCondition::Compares { property, value, equal } => {
                (values[*property as usize] == *value) == *equal
            }
            StateCondition::Not(inner) => !inner.matches(values),
            StateCondition::And(pair) => pair.0.matches(values) && pair.1.matches(values),
            StateCondition::Or(pair) => pair.0.matches(values) || pair.1.matches(values),
        }
    }

    pub fn compile(source: &str, properties: &BlockProperties) -> Result<Self, MolangError> {
        let tokens = tokenize(source)?;
        let mut parser = Parser { tokens: &tokens, at: 0, properties };
        let condition = parser.expression()?;
        match parser.peek() {
            None => Ok(condition),
            Some(token) => Err(MolangError::TrailingInput(token.text())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    BlockState(String),
    Literal(PropertyValue),
    Equal,
    NotEqual,
    And,
    Or,
    Not,
    Open,
    Close,
}

impl Token {
    fn text(&self) -> String {
        match self {
            Token::BlockState(name) => format!("q.block_state('{name}')"),
            Token::Literal(value) => value.to_string(),
            Token::Equal => "==".into(),
            Token::NotEqual => "!=".into(),
            Token::And => "&&".into(),
            Token::Or => "||".into(),
            Token::Not => "!".into(),
            Token::Open => "(".into(),
            Token::Close => ")".into(),
        }
    }
}

const QUERY: &str = "q.block_state";

fn tokenize(source: &str) -> Result<Vec<Token>, MolangError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        match bytes[at] {
            b' ' | b'\t' | b'\n' | b'\r' => at += 1,
            b'(' => {
                tokens.push(Token::Open);
                at += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                at += 1;
            }
            b'&' | b'|' => {
                let pair = &source[at..source.len().min(at + 2)];
                let token = match pair {
                    "&&" => Token::And,
                    "||" => Token::Or,
                    _ => return Err(MolangError::UnexpectedToken(source[at..=at].into())),
                };
                tokens.push(token);
                at += 2;
            }
            b'=' => {
                if source[at..].starts_with("==") {
                    tokens.push(Token::Equal);
                    at += 2;
                } else {
                    return Err(MolangError::UnexpectedToken("=".into()));
                }
            }
            b'!' => {
                if source[at..].starts_with("!=") {
                    tokens.push(Token::NotEqual);
                    at += 2;
                } else {
                    tokens.push(Token::Not);
                    at += 1;
                }
            }
            b'\'' => {
                at += 1;
                let end = source[at..]
                    .find('\'')
                    .ok_or_else(|| MolangError::UnterminatedString(source[start..].into()))?;
                tokens.push(Token::Literal(PropertyValue::Str(source[at..at + end].into())));
                at += end + 1;
            }
            b'0'..=b'9' | b'-' => {
                at += 1;
                while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                    at += 1;
                }
                let text = &source[start..at];
                let value = text
                    .parse::<i32>()
                    .map_err(|_| MolangError::UnexpectedToken(text.into()))?;
                tokens.push(Token::Literal(PropertyValue::Int(value)));
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                while at < bytes.len()
                    && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_' || bytes[at] == b'.')
                {
                    at += 1;
                }
                let name = &source[start..at];
                match name {
                    "true" => tokens.push(Token::Literal(PropertyValue::Bool(true))),
                    "false" => tokens.push(Token::Literal(PropertyValue::Bool(false))),
                    QUERY => {
                        at = tokenize_query(source, at, &mut tokens)?;
                    }
                    _ => return Err(MolangError::UnexpectedToken(name.into())),
                }
            }
            _ => {
                let end = source[at..].chars().next().map_or(at, |c| at + c.len_utf8());
                return Err(MolangError::UnexpectedToken(source[at..end].into()));
            }
        }
    }
    Ok(tokens)
}

fn tokenize_query(source: &str, at: usize, tokens: &mut Vec<Token>) -> Result<usize, MolangError> {
    let rest = source[at..].trim_start();
    let argument = rest
        .strip_prefix("('")
        .ok_or_else(|| MolangError::UnexpectedToken(format!("{QUERY}{}", first_char(rest))))?;
    let end = argument
        .find('\'')
        .ok_or_else(|| MolangError::UnterminatedString(rest.into()))?;
    let closing = argument[end + 1..]
        .strip_prefix(')')
        .ok_or_else(|| MolangError::UnexpectedToken(format!("{QUERY}('{}'", &argument[..end])))?;
    tokens.push(Token::BlockState(argument[..end].into()));
    Ok(source.len() - closing.len())
}

fn first_char(rest: &str) -> String {
    rest.chars().next().map(String::from).unwrap_or_default()
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    properties: &'a BlockProperties,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Result<&Token, MolangError> {
        let token = self.tokens.get(self.at).ok_or(MolangError::UnexpectedEnd)?;
        self.at += 1;
        Ok(token)
    }

    fn expression(&mut self) -> Result<StateCondition, MolangError> {
        let mut left = self.conjunction()?;
        while self.peek() == Some(&Token::Or) {
            self.at += 1;
            let right = self.conjunction()?;
            left = StateCondition::Or(Box::new((left, right)));
        }
        Ok(left)
    }

    fn conjunction(&mut self) -> Result<StateCondition, MolangError> {
        let mut left = self.unary()?;
        while self.peek() == Some(&Token::And) {
            self.at += 1;
            let right = self.unary()?;
            left = StateCondition::And(Box::new((left, right)));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<StateCondition, MolangError> {
        if self.peek() == Some(&Token::Not) {
            self.at += 1;
            return Ok(StateCondition::Not(Box::new(self.unary()?)));
        }
        if self.peek() == Some(&Token::Open) {
            self.at += 1;
            let inner = self.expression()?;
            match self.next()? {
                Token::Close => return Ok(inner),
                other => return Err(MolangError::UnexpectedToken(other.text())),
            }
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<StateCondition, MolangError> {
        let left = self.operand()?;
        let equal = match self.next()? {
            Token::Equal => true,
            Token::NotEqual => false,
            other => return Err(MolangError::ExpectedComparison(other.text())),
        };
        let right = self.operand()?;
        let (name, value) = match (left, right) {
            (Operand::BlockState(name), Operand::Literal(value))
            | (Operand::Literal(value), Operand::BlockState(name)) => (name, value),
            _ => return Err(MolangError::NotAComparison),
        };
        let property = self
            .properties
            .index_of(&name)
            .ok_or(MolangError::UnknownProperty(name))?;
        let index = self.properties.0[property]
            .values
            .iter()
            .position(|v| *v == value)
            .ok_or_else(|| MolangError::UnknownValue {
                property: self.properties.0[property].name.to_string(),
                value: value.to_string(),
            })?;
        Ok(StateCondition::Compares { property: property as u8, value: index as u8, equal })
    }

    fn operand(&mut self) -> Result<Operand, MolangError> {
        match self.next()? {
            Token::BlockState(name) => Ok(Operand::BlockState(name.clone())),
            Token::Literal(value) => Ok(Operand::Literal(value.clone())),
            other => Err(MolangError::UnexpectedToken(other.text())),
        }
    }
}

enum Operand {
    BlockState(String),
    Literal(PropertyValue),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties() -> BlockProperties {
        serde_json::from_str(
            r#"{
                "facing": ["north", "south"],
                "level": [0, 1, 2],
                "waterlogged": [true, false]
            }"#,
        )
        .unwrap()
    }

    fn compile(source: &str) -> Result<StateCondition, MolangError> {
        StateCondition::compile(source, &properties())
    }

    #[test]
    fn compiles_a_conjunction_to_value_indices() {
        let condition =
            compile("q.block_state('facing') == 'south' && q.block_state('level') != 2").unwrap();
        assert_eq!(
            condition,
            StateCondition::And(Box::new((
                StateCondition::Compares { property: 0, value: 1, equal: true },
                StateCondition::Compares { property: 1, value: 2, equal: false },
            )))
        );
        assert!(condition.matches(&[1, 0, 0]));
        assert!(!condition.matches(&[1, 2, 0]));
        assert!(!condition.matches(&[0, 0, 0]));
    }

    #[test]
    fn or_binds_looser_than_and() {
        let condition = compile(
            "q.block_state('facing') == 'north' && q.block_state('level') == 0 \
             || q.block_state('waterlogged') == true",
        )
        .unwrap();
        assert!(condition.matches(&[0, 0, 1]));
        assert!(condition.matches(&[1, 2, 0]));
        assert!(!condition.matches(&[1, 2, 1]));
    }

    #[test]
    fn parentheses_and_negation_apply() {
        let condition = compile(
            "!(q.block_state('facing') == 'north' || q.block_state('level') == 1)",
        )
        .unwrap();
        assert!(condition.matches(&[1, 0, 0]));
        assert!(!condition.matches(&[0, 0, 0]));
        assert!(!condition.matches(&[1, 1, 0]));
    }

    #[test]
    fn a_literal_may_lead_the_comparison() {
        let condition = compile("'north' == q.block_state('facing')").unwrap();
        assert_eq!(condition, StateCondition::Compares { property: 0, value: 0, equal: true });
    }

    #[test]
    fn arithmetic_is_rejected() {
        let err = compile("q.block_state('level') + 1 == 2").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("+".into()));
        assert!(err.to_string().contains('+'), "{err}");
    }

    #[test]
    fn math_calls_are_rejected() {
        let err = compile("math.floor(q.block_state('level')) == 1").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("math.floor".into()));
    }

    #[test]
    fn the_long_query_spelling_is_rejected() {
        let err = compile("query.block_state('facing') == 'north'").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("query.block_state".into()));
    }

    #[test]
    fn variables_are_rejected() {
        let err = compile("v.facing == 'north'").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("v.facing".into()));
    }

    #[test]
    fn ordering_comparisons_are_rejected() {
        let err = compile("q.block_state('level') > 1").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken(">".into()));
    }

    #[test]
    fn assignment_is_rejected() {
        let err = compile("q.block_state('level') = 1").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("=".into()));
    }

    #[test]
    fn single_ampersand_is_rejected() {
        let err = compile("q.block_state('level') == 1 & q.block_state('level') == 2").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("&".into()));
    }

    #[test]
    fn float_literals_are_rejected() {
        let err = compile("q.block_state('level') == 1.5").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("1.5".into()));
    }

    #[test]
    fn an_unterminated_string_is_rejected() {
        let err = compile("q.block_state('facing') == 'north").unwrap_err();
        assert_eq!(err, MolangError::UnterminatedString("'north".into()));
    }

    #[test]
    fn comparing_two_block_states_is_rejected() {
        let err = compile("q.block_state('facing') == q.block_state('level')").unwrap_err();
        assert_eq!(err, MolangError::NotAComparison);
    }

    #[test]
    fn an_unknown_property_is_rejected() {
        let err = compile("q.block_state('powered') == true").unwrap_err();
        assert_eq!(err, MolangError::UnknownProperty("powered".into()));
    }

    #[test]
    fn a_string_literal_never_matches_a_boolean_property() {
        let err = compile("q.block_state('waterlogged') == 'true'").unwrap_err();
        assert_eq!(
            err,
            MolangError::UnknownValue {
                property: "waterlogged".into(),
                value: "'true'".into(),
            }
        );
    }

    #[test]
    fn an_integer_literal_never_matches_a_string_property() {
        let err = compile("q.block_state('facing') == 0").unwrap_err();
        assert_eq!(
            err,
            MolangError::UnknownValue { property: "facing".into(), value: "0".into() }
        );
    }

    #[test]
    fn trailing_input_is_rejected() {
        let err = compile("q.block_state('facing') == 'north' 'south'").unwrap_err();
        assert_eq!(err, MolangError::TrailingInput("'south'".into()));
    }

    #[test]
    fn a_truncated_query_is_rejected() {
        let err = compile("q.block_state == 'north'").unwrap_err();
        assert_eq!(err, MolangError::UnexpectedToken("q.block_state=".into()));
    }
}
