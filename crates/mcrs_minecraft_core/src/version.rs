use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackVersion {
    pub resource_major: u32,
    pub resource_minor: u32,
    pub data_major: u32,
    pub data_minor: u32,
}

/// The game's `version.json`. The embedded one is [`VERSION`]; another is read
/// with owned strings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version<S = &'static str> {
    pub id: S,
    pub name: S,
    pub world_version: i32,
    pub series_id: S,
    pub protocol_version: i32,
    pub pack_version: PackVersion,
    pub build_time: S,
    pub java_component: S,
    pub java_version: u32,
    pub stable: bool,
    pub use_editor: bool,
}

pub const VERSION_JSON: &str = include_str!("../../../assets/minecraft/version.json");

pub const VERSION: Version = include!(concat!(env!("OUT_DIR"), "/version.rs"));

pub fn check_corpus_version(corpus: &[u8]) -> Result<(), String> {
    if VERSION_JSON.as_bytes() == corpus {
        return Ok(());
    }
    let embedded = VERSION.id;
    Err(match serde_json::from_slice::<Version<String>>(corpus) {
        Ok(version) => format!(
            "the corpus version.json (id {}) is not the version.json this build embeds (id {embedded})",
            version.id
        ),
        Err(reason) => format!(
            "the corpus version.json does not parse ({reason}); this build embeds id {embedded}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replaced(from: &str, to: &str) -> String {
        let text = VERSION_JSON.replace(from, to);
        assert_ne!(text, VERSION_JSON, "{from} is not in the file");
        text
    }

    #[test]
    fn only_the_embedded_bytes_pass_and_another_id_is_refused_naming_both_ids() {
        assert!(check_corpus_version(VERSION_JSON.as_bytes()).is_ok());
        let other = replaced(VERSION.id, "other-id");
        let message = check_corpus_version(other.as_bytes()).unwrap_err();
        assert!(message.contains("other-id"), "{message}");
        assert!(message.contains(VERSION.id), "{message}");
    }

    #[test]
    fn a_difference_in_whitespace_alone_is_refused() {
        for suffix in [" ", "\n"] {
            let padded = format!("{VERSION_JSON}{suffix}");
            let message = check_corpus_version(padded.as_bytes()).unwrap_err();
            assert_eq!(message.matches(VERSION.id).count(), 2, "{message}");
        }
    }

    #[test]
    fn an_unparsable_corpus_is_refused_naming_the_embedded_id() {
        let message = check_corpus_version(b"not json").unwrap_err();
        assert!(message.contains("does not parse"), "{message}");
        assert!(message.contains(VERSION.id), "{message}");
    }
}
