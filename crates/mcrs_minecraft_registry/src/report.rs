use crate::id::Id;
use crate::registry::{Registry, UnknownEntry};
use crate::set::RegistrySet;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_key::ResourceKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

const ROOT: ResourceLocation<&'static str> = rl!("minecraft:root");

type Key = (ResourceLocation<Arc<str>>, Option<String>, Option<String>);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoadReport {
    errors: BTreeMap<Key, Vec<String>>,
}

impl LoadReport {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(
        &mut self,
        registry: &ResourceLocation<Arc<str>>,
        entry: Option<&str>,
        file: Option<&str>,
        message: String,
    ) {
        let key = (
            registry.clone(),
            entry.map(str::to_owned),
            file.map(str::to_owned),
        );
        let messages = self.errors.entry(key).or_default();
        if !messages.contains(&message) {
            messages.push(message);
        }
    }

    pub(crate) fn entry(
        &mut self,
        registry: &ResourceLocation<Arc<str>>,
        entry: &str,
        file: &str,
        message: impl fmt::Display,
    ) {
        self.record(registry, Some(entry), Some(file), message.to_string());
    }

    pub(crate) fn whole_registry(
        &mut self,
        registry: &ResourceLocation<Arc<str>>,
        directory: &str,
        message: impl fmt::Display,
    ) {
        self.record(registry, None, Some(directory), message.to_string());
    }

    pub fn require<R: RegistryKey, S: AsRef<str>>(
        &mut self,
        registry: &Registry<R>,
        key: &ResourceKey<R, S>,
    ) -> Option<Id<R>> {
        self.resolved(registry.require(key))
    }

    pub fn require_by_name<R: RegistryKey>(
        &mut self,
        registry: &Registry<R>,
        name: &str,
    ) -> Option<Id<R>> {
        self.resolved(registry.require_by_name(name))
    }

    fn resolved<R>(&mut self, found: Result<Id<R>, UnknownEntry>) -> Option<Id<R>> {
        match found {
            Ok(id) => Some(id),
            Err(error) => {
                self.record(&error.registry, Some(&error.name), None, error.to_string());
                None
            }
        }
    }

    pub fn missing<R: RegistryKey>(&mut self, name: &str, message: impl fmt::Display) {
        self.record(&R::KEY.into(), Some(name), None, message.to_string());
    }

    pub fn registry<R: RegistryKey>(&mut self, set: &RegistrySet) -> Option<Registry<R>> {
        let registry = set.registry::<R>();
        if registry.is_none() {
            self.record(
                &ROOT.into(),
                Some(R::KEY.as_str()),
                None,
                format!("registry {} is absent from the registries report", R::KEY),
            );
        }
        registry
    }

    pub fn invalid(error: impl fmt::Display) -> Self {
        let mut report = Self::new();
        report.invalid_report(error);
        report
    }

    pub fn invalid_report(&mut self, error: impl fmt::Display) {
        self.record(&ROOT.into(), Some("registries"), None, error.to_string());
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for LoadReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut separator = "";
        if self.errors.keys().any(|(_, _, file)| file.is_some()) {
            let errors: usize = self.errors.values().map(Vec::len).sum();
            let mut registries: Vec<_> =
                self.errors.keys().map(|(registry, ..)| registry).collect();
            registries.dedup();
            write!(
                f,
                "registry load failed: {errors} errors in {} registries",
                registries.len()
            )?;
            separator = "\n";
        }
        for ((registry, entry, file), messages) in &self.errors {
            for message in messages {
                f.write_str(separator)?;
                separator = "\n";
                write!(f, "{registry}")?;
                if let Some(entry) = entry {
                    write!(f, "/{entry}")?;
                }
                if let Some(file) = file {
                    write!(f, " ({file})")?;
                }
                write!(f, ": {message}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Alpha;

    impl RegistryKey for Alpha {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:alpha");
    }

    struct Beta;

    impl RegistryKey for Beta {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:beta");
    }

    fn key<R>(text: &'static str) -> ResourceKey<R, &'static str> {
        ResourceKey::new(ResourceLocation::new_static(text))
    }

    fn registry<R: RegistryKey>(names: &[&str]) -> Registry<R> {
        Registry::new(
            names
                .iter()
                .map(|text| ResourceLocation::<Arc<str>>::parse(text).unwrap()),
        )
        .unwrap()
    }

    #[test]
    fn a_found_entry_is_returned_and_nothing_is_recorded() {
        let alpha = registry::<Alpha>(&["minecraft:one", "minecraft:two"]);
        let mut report = LoadReport::new();
        let id = report.require(&alpha, &key("minecraft:two"));
        assert_eq!(id, alpha.by_name("minecraft:two"));
        assert_eq!(id.unwrap().index(), 1);
        assert!(report.is_empty());
        assert_eq!(report.to_string(), "");
    }

    #[test]
    fn every_missing_entry_of_one_pass_is_named_sorted_by_registry_then_entry() {
        let alpha = registry::<Alpha>(&["minecraft:present"]);
        let beta = registry::<Beta>(&[]);
        let mut report = LoadReport::new();
        assert!(report.require(&beta, &key("minecraft:b_second")).is_none());
        assert!(report.require(&alpha, &key("minecraft:z_last")).is_none());
        assert!(report.require(&beta, &key("minecraft:a_first")).is_none());
        assert!(report.require(&alpha, &key("minecraft:present")).is_some());
        assert!(!report.is_empty());

        let text = report.to_string();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        let expected = [
            ("minecraft:alpha", "minecraft:z_last"),
            ("minecraft:beta", "minecraft:a_first"),
            ("minecraft:beta", "minecraft:b_second"),
        ];
        for (line, (registry, entry)) in lines.iter().zip(expected) {
            assert!(line.starts_with(&format!("{registry}/{entry}: ")), "{line}");
        }
    }

    #[test]
    fn a_report_names_the_key_s_own_registry() {
        let alpha = registry::<Alpha>(&[]);
        let beta = registry::<Beta>(&["minecraft:one"]);
        let mut report = LoadReport::new();
        assert!(report.require(&alpha, &key("minecraft:one")).is_none());
        assert_eq!(
            report.require(&beta, &key("minecraft:one")),
            beta.by_name("minecraft:one")
        );
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(
            text.starts_with("minecraft:alpha/minecraft:one: "),
            "{text}"
        );
    }

    #[test]
    fn a_data_name_is_reported_as_it_was_written() {
        let alpha = registry::<Alpha>(&["minecraft:one"]);
        let mut report = LoadReport::new();
        assert_eq!(report.require_by_name(&alpha, "one"), alpha.by_name("one"));
        assert!(report.require_by_name(&alpha, "One").is_none());
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.starts_with("minecraft:alpha/One: "), "{text}");
    }

    #[test]
    fn a_miss_asked_for_twice_is_named_once() {
        let alpha = registry::<Alpha>(&[]);
        let mut report = LoadReport::new();
        report.require(&alpha, &key("minecraft:absent"));
        report.require(&alpha, &key("minecraft:absent"));
        assert_eq!(report.to_string().lines().count(), 1);
    }

    #[test]
    fn a_registry_the_report_lacks_is_reported() {
        let set = RegistrySet::new()
            .with(registry::<Alpha>(&["minecraft:one"]))
            .unwrap();
        let mut report = LoadReport::new();
        assert!(report.registry::<Alpha>(&set).is_some());
        assert!(report.is_empty());
        assert!(report.registry::<Beta>(&set).is_none());
        assert!(!report.is_empty());
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("minecraft:beta"), "{text}");
        assert!(text.contains("absent from the registries report"), "{text}");
    }
}
