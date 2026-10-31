use mcrs_minecraft_resource::Id;

mcrs_minecraft_registries::static_registry! {
    pub enum Fruit;
    Zebra = "minecraft:zebra",
    Apple = "minecraft:apple",
}

impl mcrs_minecraft_resource::Registered for Fruit {
    const REGISTRY: mcrs_minecraft_resource::RegistryKey<Self> =
        mcrs_minecraft_resource::RegistryKey::new(mcrs_minecraft_resource::rl!("minecraft:fruit"));
}

#[test]
fn an_entry_is_numbered_by_its_position_and_names_its_location() {
    assert_eq!(Fruit::Zebra.id().number(), 0);
    assert_eq!(Fruit::Apple.id().number(), 1);
    assert_eq!(Fruit::Apple.as_static_str(), "minecraft:apple");
    assert_eq!(
        Fruit::ENTRIES[Fruit::Apple.id().index()],
        Fruit::Apple.location()
    );
}

#[test]
fn an_entry_is_found_by_id_and_by_location() {
    assert_eq!(Fruit::from_id(Fruit::Apple.id()), Some(Fruit::Apple));
    assert_eq!(Fruit::read("minecraft:zebra"), Some(Fruit::Zebra));
    assert_eq!(Fruit::read("minecraft:mango"), None);
    let beyond: Id<Fruit> = Id::checked(2, Fruit::ALL.len() + 1).unwrap();
    assert_eq!(Fruit::from_id(beyond), None);
    assert_eq!(Fruit::from_protocol_id(1), Some(Fruit::Apple));
    assert_eq!(Fruit::from_protocol_id(2), None);
}

#[test]
fn an_entry_reads_and_writes_as_its_location() {
    assert_eq!(
        serde_json::to_string(&Fruit::Apple).unwrap(),
        r#""minecraft:apple""#
    );
    let cases = [
        (r#""minecraft:apple""#, Ok(Fruit::Apple)),
        (r#""zebra""#, Ok(Fruit::Zebra)),
        (
            r#""minecraft:mango""#,
            Err(
                "Unknown registry key in ResourceKey[minecraft:root / minecraft:fruit]: minecraft:mango",
            ),
        ),
        (
            r#""mango""#,
            Err(
                "Unknown registry key in ResourceKey[minecraft:root / minecraft:fruit]: minecraft:mango",
            ),
        ),
    ];
    assert_eq!(Fruit::read("apple"), Some(Fruit::Apple));
    assert_eq!(Fruit::read("minecraft:apple"), Some(Fruit::Apple));
    assert_eq!(Fruit::read("mango"), None);
    assert_eq!(Fruit::read("Not A Location"), None);
    for (json, expected) in cases {
        let read = serde_json::from_str::<Fruit>(json).map_err(|error| error.to_string());
        match expected {
            Ok(fruit) => assert_eq!(read, Ok(fruit)),
            Err(message) => assert!(read.unwrap_err().contains(message)),
        }
    }
}
