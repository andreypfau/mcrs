use crate::vanilla_argument::VanillaArgument;
use mcrs_command::Graph;
use mcrs_minecraft_command::PermissionCheck;

/// The command tree of the vanilla argument types, with handlers of type `H`.
pub type VanillaGraph<H> = Graph<VanillaArgument, PermissionCheck, H>;
