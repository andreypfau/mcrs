use mcrs_minecraft_wire::{Decode, Encode};

/// How much of the input a `brigadier:string` argument reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StringMode {
    /// One unquoted word.
    Word,
    /// One word, or a quoted phrase.
    Phrase,
    /// The rest of the input.
    Greedy,
}
