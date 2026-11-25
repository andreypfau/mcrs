use mcrs_minecraft_lang::Piece;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FormatError;

pub(crate) struct Template<'a> {
    rest: &'a str,
    next: usize,
    failed: bool,
}

impl<'a> Template<'a> {
    pub(crate) fn new(template: &'a str) -> Self {
        Self {
            rest: template,
            next: 0,
            failed: false,
        }
    }

    pub(crate) fn is_valid(template: &str, arguments: usize) -> bool {
        Template::new(template).all(|piece| match piece {
            Ok(Piece::Argument(index)) => index < arguments,
            Ok(Piece::Literal(_)) => true,
            Err(FormatError) => false,
        })
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
    fn a_template_is_valid_when_each_placeholder_has_an_argument() {
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
            assert_eq!(Template::is_valid(template, arguments), valid, "{template}");
        }
    }
}
