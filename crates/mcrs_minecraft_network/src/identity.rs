use mcrs_minecraft_core::{ResourceLocation, rl};

// A build that reports its commit sets MCRS_COMMIT_HASH; reading it from git in a
// build script would rebuild this crate and everything above it on every commit.
pub const COMMIT_HASH: &str = match option_env!("MCRS_COMMIT_HASH") {
    Some(hash) => hash,
    None => "unknown",
};

pub const BRAND: &str = concat!("github.com/andreypfau/mcrs@", env!("CARGO_PKG_VERSION"));

pub const MOD_ENTRY: ResourceLocation<&'static str> = rl!("github.com:andreypfau/mcrs");

pub const COMMIT_PROPERTY: ResourceLocation<&'static str> = rl!("mcrs:commit");
