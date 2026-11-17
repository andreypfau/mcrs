use mcrs_minecraft_command::Lookup;
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;

pub trait VanillaLookup: Lookup + BlockLookup + TimeLookup {}

impl<T: Lookup + BlockLookup + TimeLookup> VanillaLookup for T {}
