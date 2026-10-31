#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
mod root;
mod stack;

use mcrs_minecraft_resource::ResourceLocationBuf;

pub const META: &str = r#"{"pack":{"description":"x","min_format":122,"max_format":122}}"#;

pub fn rl(text: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(text).unwrap()
}
