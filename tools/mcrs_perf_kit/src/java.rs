use std::process::Command;

use anyhow::{Context, bail};

/// The major version `java -version` reports, `1.8.0` being 8.
pub fn major_version(java: &str) -> anyhow::Result<u32> {
    let output = Command::new(java)
        .arg("-version")
        .output()
        .with_context(|| format!("running `{java} -version`"))?;
    let text = String::from_utf8_lossy(&output.stderr);
    let Some(version) = text
        .split_once("version \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(version, _)| version)
    else {
        bail!("`{java} -version` printed no version: {text}");
    };
    let mut parts = version.split(['.', '-', '+']);
    let major = match parts.next() {
        Some("1") => parts.next(),
        first => first,
    };
    match major.and_then(|major| major.parse().ok()) {
        Some(major) => Ok(major),
        None => bail!("`{java} -version` printed version {version}, which has no major number"),
    }
}
