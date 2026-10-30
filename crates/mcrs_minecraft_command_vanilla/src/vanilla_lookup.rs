use mcrs_minecraft_command::Lookup;
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_block::BlockLookup;

/// What every vanilla argument reads from the world of the source.
pub trait VanillaLookup: Lookup + BlockLookup + TimeLookup {}

impl<T: Lookup + BlockLookup + TimeLookup> VanillaLookup for T {}
