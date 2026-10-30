use mcrs_command::Reader;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Invertible {
    #[default]
    None,
    Single,
    Multiple,
}

impl Invertible {
    pub(crate) fn allows(self, negated: bool) -> bool {
        if negated {
            self != Invertible::Single
        } else {
            self == Invertible::None
        }
    }

    /// The forms a value can take now: plain and negated, only negated, or none.
    pub(crate) fn prefixes(self) -> &'static [&'static str] {
        match self {
            Invertible::None => &["", "!"],
            Invertible::Multiple => &["!"],
            Invertible::Single => &[],
        }
    }

    pub(crate) fn after(negated: bool) -> Self {
        if negated {
            Invertible::Multiple
        } else {
            Invertible::Single
        }
    }
}

pub(crate) fn read_negation(reader: &mut Reader<'_>) -> bool {
    reader.skip_whitespace();
    let negated = reader.peek_is('!');
    if negated {
        reader.skip();
        reader.skip_whitespace();
    }
    negated
}
