#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op<I, S> {
    Split {
        from: I,
        to: I,
        count: u8,
    },
    Merge {
        from: I,
        to: I,
    },
    Swap {
        left: I,
        right: I,
    },
    Drop {
        from: I,
        count: u8,
    },
    Set {
        slot: I,
        stack: Option<S>,
        count: u8,
    },
}
