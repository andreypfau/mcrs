#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Registry, RegistryError};
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::resource_location::ResourceLocation;
    use mcrs_minecraft_core::rl;

    struct TestRegistry;

    impl RegistryKey for TestRegistry {
        const KEY: ResourceLocation<&'static str> = rl!("minecraft:test_registry");
    }

    fn registry(names: &[&str]) -> Registry<TestRegistry> {
        Registry::new(
            names
                .iter()
                .map(|text| ResourceLocation::parse(text).unwrap()),
            std::iter::empty(),
        )
        .unwrap()
    }

    const NAMES: [&str; 3] = ["minecraft:plains", "minecraft:desert", "minecraft:forest"];

    #[test]
    fn a_column_is_indexed_by_id() {
        let registry = registry(&NAMES);
        let column = Entries::new(&registry, vec!["first", "second", "third"]).unwrap();
        assert_eq!(column[registry.get("minecraft:plains").unwrap()], "first");
        assert_eq!(column[registry.get("minecraft:desert").unwrap()], "second");
        assert_eq!(column[registry.get("minecraft:forest").unwrap()], "third");
        for id in registry.ids() {
            assert!(column.get(id).is_some());
        }
    }

    #[test]
    fn a_column_of_the_wrong_length_does_not_build() {
        let registry = registry(&NAMES);
        for found in [2usize, 4] {
            let error = Entries::new(&registry, vec![0u8; found]).err().unwrap();
            let message = error.to_string();
            assert!(
                matches!(
                    error,
                    RegistryError::LengthMismatch { expected: 3, found: f, .. } if f == found
                ),
                "{message}"
            );
            assert!(message.contains("minecraft:test_registry"), "{message}");
            assert!(message.contains('3'), "{message}");
            assert!(message.contains(&found.to_string()), "{message}");
        }
    }

    #[test]
    fn an_id_past_the_column_has_no_value() {
        let larger = registry(&NAMES);
        let smaller = registry(&["minecraft:plains"]);
        let column = Entries::new(&smaller, vec![7]).unwrap();
        assert_eq!(
            column.get(smaller.get("minecraft:plains").unwrap()),
            Some(&7)
        );
        assert_eq!(column.get(larger.get("minecraft:desert").unwrap()), None);
    }

    #[test]
    #[should_panic]
    fn indexing_past_the_column_panics() {
        let larger = registry(&NAMES);
        let smaller = registry(&["minecraft:plains"]);
        let column = Entries::new(&smaller, vec![7]).unwrap();
        let _ = column[larger.get("minecraft:forest").unwrap()];
    }

    #[test]
    fn an_empty_column_over_an_empty_registry_builds() {
        let registry = registry(&[]);
        let column = Entries::<TestRegistry, u8>::new(&registry, Vec::new()).unwrap();
        assert!(column.get(crate::id::Id::from_number(0)).is_none());
    }
}
