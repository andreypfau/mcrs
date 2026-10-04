use crate::id::Id;
use crate::registry::Registry;
use crate::set::RegistrySet;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

const ROOT: ResourceLocation<&'static str> = rl!("minecraft:root");

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoadReport {
    misses: BTreeMap<(ResourceLocation<Arc<str>>, String), String>,
}

impl LoadReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn require<R: RegistryKey>(&mut self, registry: &Registry<R>, name: &str) -> Option<Id<R>> {
        match registry.require(name) {
            Ok(id) => Some(id),
            Err(error) => {
                self.misses.insert(
                    (error.registry.clone(), error.name.clone()),
                    error.to_string(),
                );
                None
            }
        }
    }

    pub fn registry<R: RegistryKey>(&mut self, set: &RegistrySet) -> Option<Registry<R>> {
        let registry = set.registry::<R>();
        if registry.is_none() {
            self.misses.insert(
                (ROOT.into(), R::KEY.as_str().to_owned()),
                format!("registry {} is absent from the registries report", R::KEY),
            );
        }
        registry
    }

    pub fn invalid_report(&mut self, error: impl fmt::Display) {
        self.misses
            .insert((ROOT.into(), "registries".to_owned()), error.to_string());
    }

    pub fn is_empty(&self) -> bool {
        self.misses.is_empty()
    }
}

impl fmt::Display for LoadReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, ((registry, name), message)) in self.misses.iter().enumerate() {
            if position > 0 {
                writeln!(f)?;
            }
            write!(f, "{registry}/{name}: {message}")?;
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

    fn registry<R: RegistryKey>(names: &[&str]) -> Registry<R> {
        Registry::new(
            names
                .iter()
                .map(|text| ResourceLocation::<Arc<str>>::parse(text).unwrap()),
            std::iter::empty(),
        )
        .unwrap()
    }

    #[test]
    fn a_found_entry_is_returned_and_nothing_is_recorded() {
        let alpha = registry::<Alpha>(&["minecraft:one", "minecraft:two"]);
        let mut report = LoadReport::new();
        let id = report.require(&alpha, "minecraft:two");
        assert_eq!(id, alpha.get("minecraft:two"));
        assert_eq!(id.unwrap().index(), 1);
        assert!(report.is_empty());
        assert_eq!(report.to_string(), "");
    }

    #[test]
    fn every_missing_entry_of_one_pass_is_named_sorted_by_registry_then_entry() {
        let alpha = registry::<Alpha>(&["minecraft:present"]);
        let beta = registry::<Beta>(&[]);
        let mut report = LoadReport::new();
        assert!(report.require(&beta, "minecraft:b_second").is_none());
        assert!(report.require(&alpha, "minecraft:z_last").is_none());
        assert!(report.require(&beta, "minecraft:a_first").is_none());
        assert!(report.require(&alpha, "minecraft:present").is_some());
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
    fn a_miss_asked_for_twice_is_named_once() {
        let alpha = registry::<Alpha>(&[]);
        let mut report = LoadReport::new();
        report.require(&alpha, "minecraft:absent");
        report.require(&alpha, "minecraft:absent");
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
