use mcrs_minecraft_wire::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StringMode {
    Word,
    Phrase,
    Greedy,
}
