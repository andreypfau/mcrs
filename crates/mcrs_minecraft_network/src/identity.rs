use mcrs_minecraft_core::{ResourceLocation, rl};

pub const COMMIT_HASH: &str = env!("MCRS_COMMIT_HASH");

pub const BRAND: &str = concat!("github.com/andreypfau/mcrs@", env!("MCRS_COMMIT_HASH"));

pub const MOD_ENTRY: ResourceLocation<&'static str> = rl!("github.com:andreypfau/mcrs");

pub const COMMIT_PROPERTY: ResourceLocation<&'static str> = rl!("mcrs:commit");

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::COMMIT_HASH;

    fn workspace_root() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    }

    #[test]
    fn the_commit_hash_is_the_checkouts_or_the_placeholder() {
        if !workspace_root().join(".git").exists() {
            assert_eq!(COMMIT_HASH, "unknown");
            return;
        }
        assert_eq!(COMMIT_HASH.len(), 40);
        assert!(COMMIT_HASH.bytes().all(|b| b.is_ascii_hexdigit()));
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_root())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            COMMIT_HASH,
            String::from_utf8(output.stdout).unwrap().trim()
        );
    }
}
