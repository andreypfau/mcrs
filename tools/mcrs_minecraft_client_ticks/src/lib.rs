//! The client ticks of a vanilla client as text: one recorded step of an experiment per file,
//! with the world of the step and the packets of each client tick in the order the client applied
//! or sent them. `tools/mcrs_minecraft_client_recorder` writes the files; the parity tests of the
//! physics kernel read them.

mod error;
mod event;
mod fields;
mod recording;
#[cfg(test)]
mod tests;

pub use error::ParseError;
pub use event::Abilities;
pub use event::Attribute;
pub use event::BlockChange;
pub use event::Effect;
pub use event::Event;
pub use event::Keys;
pub use event::Modifier;
pub use event::Move;
pub use event::Rotation;
pub use event::Teleport;
pub use recording::Fill;
pub use recording::Header;
pub use recording::Recording;

/// The first line of a recording; the number is the format version.
pub const FORMAT_LINE: &str = "client-ticks 1";
