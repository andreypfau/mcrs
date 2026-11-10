use mcrs_minecraft_text::DataSource;

/// Trait implemented by server-side contexts to resolve dynamic components
/// (`nbt`, `score`, `selector`, `keybind`) before sending to Bedrock.
pub trait BedrockServerResolver {
    /// Maps a canonical translation key to the key of the Bedrock language files.
    fn translation_key<'a>(&'a self, key: &'a str) -> &'a str {
        key
    }

    /// Resolves an NBT path read from `source` into a displayable string.
    fn resolve_nbt(&self, nbt: &str, source: &DataSource) -> Option<String> {
        let _ = (nbt, source);
        None
    }

    /// Resolves a scoreboard value into a displayable string.
    fn resolve_score(&self, name: &str, objective: &str) -> Option<String> {
        let _ = (name, objective);
        None
    }

    /// Resolves an entity selector into a displayable string.
    fn resolve_selector(&self, selector: &str) -> Option<String> {
        let _ = selector;
        None
    }

    /// Resolves a keybind identifier into a button or key name.
    fn resolve_keybind(&self, keybind: &str) -> Option<String> {
        let _ = keybind;
        None
    }
}

/// A default resolver that provides safe fallback representations:
/// selectors and keybinds show their identifier, NBT paths show the path,
/// and scores show empty.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultServerResolver;

impl BedrockServerResolver for DefaultServerResolver {
    fn resolve_nbt(&self, nbt: &str, _source: &DataSource) -> Option<String> {
        Some(nbt.to_string())
    }

    fn resolve_score(&self, _name: &str, _objective: &str) -> Option<String> {
        None
    }

    fn resolve_selector(&self, selector: &str) -> Option<String> {
        Some(selector.to_string())
    }

    fn resolve_keybind(&self, keybind: &str) -> Option<String> {
        Some(keybind.to_string())
    }
}
