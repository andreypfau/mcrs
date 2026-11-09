/// One part of a translation pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Text that shows as it is.
    Literal(&'a str),
    /// The argument at this index, from zero.
    Argument(usize),
}

/// Splits `pattern` into its pieces, in order: `%s` takes the next argument, `%n$s` the argument
/// `n`, and `%%` is a literal `%`. `%0$s` names no argument and shows nothing. A `%` that starts
/// no placeholder is a literal.
pub fn fill_pattern<'a>(pattern: &'a str, mut emit: impl FnMut(Piece<'a>)) {
    let mut next = 0;
    let mut rest = pattern;
    while let Some(at) = rest.find('%') {
        let (literal, tail) = rest.split_at(at);
        if !literal.is_empty() {
            emit(Piece::Literal(literal));
        }
        let after_percent = tail.get(1..).unwrap_or_default();
        if let Some(after) = after_percent.strip_prefix('%') {
            emit(Piece::Literal("%"));
            rest = after;
        } else if let Some(after) = after_percent.strip_prefix('s') {
            emit(Piece::Argument(next));
            next += 1;
            rest = after;
        } else if let Some((digits, after)) = after_percent.split_once("$s")
            && let Ok(position) = digits.parse::<usize>()
        {
            if let Some(index) = position.checked_sub(1) {
                emit(Piece::Argument(index));
            }
            rest = after;
        } else {
            emit(Piece::Literal("%"));
            rest = after_percent;
        }
    }
    if !rest.is_empty() {
        emit(Piece::Literal(rest));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_take_arguments_in_order_by_position_and_escape_a_percent() {
        let cases: [(&str, &[Piece<'_>]); 4] = [
            (
                "%s ticks, %s ms",
                &[
                    Piece::Argument(0),
                    Piece::Literal(" ticks, "),
                    Piece::Argument(1),
                    Piece::Literal(" ms"),
                ],
            ),
            (
                "%2$s then %1$s, 100%%",
                &[
                    Piece::Argument(1),
                    Piece::Literal(" then "),
                    Piece::Argument(0),
                    Piece::Literal(", 100"),
                    Piece::Literal("%"),
                ],
            ),
            ("%0$s", &[]),
            (
                "5% off %d",
                &[
                    Piece::Literal("5"),
                    Piece::Literal("%"),
                    Piece::Literal(" off "),
                    Piece::Literal("%"),
                    Piece::Literal("d"),
                ],
            ),
        ];
        for (pattern, expected) in cases {
            let mut pieces = Vec::new();
            fill_pattern(pattern, |piece| pieces.push(piece));
            assert_eq!(pieces, expected, "{pattern}");
        }
    }
}
