use core::fmt;

use crate::piece::Piece;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatError;

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("malformed translation template")
    }
}

impl core::error::Error for FormatError {}

pub fn template_pieces(
    template: &str,
    arguments: usize,
) -> Result<impl Iterator<Item = Piece<'_>>, FormatError> {
    for piece in Template::new(template) {
        if let Piece::Argument(index) = piece?
            && index >= arguments
        {
            return Err(FormatError);
        }
    }
    Ok(Template::new(template).map_while(Result::ok))
}

struct Template<'a> {
    rest: &'a str,
    next: usize,
    failed: bool,
}

impl<'a> Template<'a> {
    fn new(template: &'a str) -> Self {
        Self {
            rest: template,
            next: 0,
            failed: false,
        }
    }

    fn directive(&mut self) -> Result<Piece<'a>, FormatError> {
        let after = self.rest.get(1..).ok_or(FormatError)?;
        if let Some(tail) = after.strip_prefix('%') {
            let literal = self.rest.get(..1).ok_or(FormatError)?;
            self.rest = tail;
            return Ok(Piece::Literal(literal));
        }
        if let Some(tail) = after.strip_prefix('s') {
            let index = self.next;
            self.next = self.next.saturating_add(1);
            self.rest = tail;
            return Ok(Piece::Argument(index));
        }
        let digits_end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        let (digits, tail) = after.split_at_checked(digits_end).ok_or(FormatError)?;
        let tail = tail.strip_prefix("$s").ok_or(FormatError)?;
        let position: i32 = digits.parse().map_err(|_| FormatError)?;
        let index = position.checked_sub(1).ok_or(FormatError)?;
        let index = usize::try_from(index).map_err(|_| FormatError)?;
        self.rest = tail;
        Ok(Piece::Argument(index))
    }
}

impl<'a> Iterator for Template<'a> {
    type Item = Result<Piece<'a>, FormatError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.rest.is_empty() {
            return None;
        }
        match self.rest.find('%') {
            None => {
                let literal = self.rest;
                self.rest = "";
                Some(Ok(Piece::Literal(literal)))
            }
            Some(0) => {
                let piece = self.directive();
                self.failed = piece.is_err();
                Some(piece)
            }
            Some(at) => {
                let (literal, tail) = self.rest.split_at_checked(at)?;
                self.rest = tail;
                Some(Ok(Piece::Literal(literal)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_gives_pieces_when_each_placeholder_has_an_argument() {
        let cases = [
            ("%s %s", 2, true),
            ("%2$s %1$s %s", 2, true),
            ("100%%", 0, true),
            ("", 0, true),
            ("%s", 0, false),
            ("%0$s", 1, false),
            ("%3$s", 2, false),
            ("%2147483648$s", 1, false),
            ("%d", 1, false),
            ("%5s", 1, false),
            ("% o", 1, false),
            ("50%", 1, false),
        ];
        for (template, arguments, valid) in cases {
            assert_eq!(
                template_pieces(template, arguments).is_ok(),
                valid,
                "{template}"
            );
        }
    }

    #[test]
    fn the_pieces_of_a_template_follow_its_order_and_positions() {
        let pieces: Vec<_> = template_pieces("%2$s then %1$s, 100%%", 2)
            .expect("valid template")
            .collect();
        assert_eq!(
            pieces,
            [
                Piece::Argument(1),
                Piece::Literal(" then "),
                Piece::Argument(0),
                Piece::Literal(", 100"),
                Piece::Literal("%"),
            ]
        );
    }
}
