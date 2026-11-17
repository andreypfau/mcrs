use mcrs_minecraft_text::DataSource;

pub trait BedrockServerResolver {
    fn translation_key<'a>(&'a self, key: &'a str) -> &'a str {
        key
    }

    fn resolve_nbt(&self, nbt: &str, source: &DataSource) -> Option<String> {
        let _ = (nbt, source);
        None
    }

    fn resolve_score(&self, name: &str, objective: &str) -> Option<String> {
        let _ = (name, objective);
        None
    }

    fn resolve_selector(&self, selector: &str) -> Option<String> {
        let _ = selector;
        None
    }

    fn resolve_keybind(&self, keybind: &str) -> Option<String> {
        let _ = keybind;
        None
    }
}

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
