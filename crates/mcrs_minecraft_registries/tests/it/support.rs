use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{
    AvailablePack, Declaration, Diagnostic, Diagnostics, Features, Id, IdSet, LoadCtx, PackFiles,
    PackSelection, PackStack, ReadValue, Ref, Registries, RegistryType, Stage, StageScope,
    load_with, pack_stack,
};
use serde::de::value::StrDeserializer;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub fn rl(text: &str) -> ResourceLocation {
    ResourceLocation::read(text).unwrap()
}

pub const META: &str = r#"{"pack":{"description":"x","min_format":122,"max_format":122}}"#;

pub fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub struct Block;
impl RegistryType for Block {
    type Value = ();
}

pub struct Item;
impl RegistryType for Item {
    type Value = ();
}

pub struct DamageType;
impl RegistryType for DamageType {
    type Value = Damage;
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Damage {
    pub message_id: String,
    pub exhaustion: f32,
}

impl ReadValue for Damage {
    fn read<'de, D: Deserializer<'de>>(_: &LoadCtx<'_>, d: D) -> Result<Self, D::Error> {
        Damage::deserialize(d)
    }
}

#[derive(Debug)]
pub struct Opaque;

impl ReadValue for Opaque {
    fn read<'de, D: Deserializer<'de>>(_: &LoadCtx<'_>, d: D) -> Result<Self, D::Error> {
        IgnoredAny::deserialize(d).map(|_| Opaque)
    }
}

pub struct LootTable;
impl RegistryType for LootTable {
    type Value = Opaque;
}

pub struct Predicate;
impl RegistryType for Predicate {
    type Value = Opaque;
}

pub struct VillagerTrade;
impl RegistryType for VillagerTrade {
    type Value = Opaque;
}

pub struct Enchantment;
impl RegistryType for Enchantment {
    type Value = Enchant;
}

#[derive(Debug)]
pub struct Enchant {
    pub supported_items: Option<IdSet<Item>>,
    pub exclusive_set: Option<IdSet<Enchantment>>,
    pub damage_types: Option<IdSet<DamageType>>,
    pub merchant_predicate: Option<Ref<Predicate>>,
    pub damage: Option<Ref<DamageType>>,
    pub preferred: Option<Id<Enchantment>>,
}

impl ReadValue for Enchant {
    fn read<'de, D: Deserializer<'de>>(ctx: &LoadCtx<'_>, d: D) -> Result<Self, D::Error> {
        struct Fields<'c, 'a>(&'c LoadCtx<'a>);

        impl<'de> Visitor<'de> for Fields<'_, '_> {
            type Value = Enchant;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an enchantment")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Enchant, A::Error> {
                let mut out = Enchant {
                    supported_items: None,
                    exclusive_set: None,
                    damage_types: None,
                    merchant_predicate: None,
                    damage: None,
                    preferred: None,
                };
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "supported_items" => {
                            out.supported_items = Some(map.next_value_seed(self.0.id_set())?)
                        }
                        "exclusive_set" => {
                            out.exclusive_set = Some(map.next_value_seed(self.0.id_set())?)
                        }
                        "damage_types" => {
                            out.damage_types = Some(map.next_value_seed(self.0.id_set())?)
                        }
                        "merchant_predicate" => {
                            out.merchant_predicate = Some(map.next_value_seed(self.0.reference())?)
                        }
                        "damage" => out.damage = Some(map.next_value_seed(self.0.reference())?),
                        "preferred" => {
                            let text = map.next_value::<String>()?;
                            let seed = self.0.id::<Enchantment>();
                            out.preferred = seed
                                .deserialize(StrDeserializer::<A::Error>::new(&text))
                                .ok();
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(out)
            }
        }

        d.deserialize_map(Fields(ctx))
    }
}

pub fn declarations() -> Vec<Declaration> {
    vec![
        Declaration::fixed::<Block>(
            rl("block"),
            ["stone", "dirt", "a", "b", "c", "d", "x", "y"]
                .map(rl)
                .to_vec(),
        ),
        Declaration::fixed::<Item>(rl("item"), vec![rl("stone"), rl("dirt")]),
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
        Declaration::data::<Enchantment>(rl("enchantment"), Stage::World).into(),
        Declaration::data::<LootTable>(rl("loot_table"), Stage::Reloadable).into(),
        Declaration::data::<Predicate>(rl("predicate"), Stage::Reloadable).into(),
    ]
}

pub fn damage(message_id: &str) -> String {
    format!(r#"{{"message_id":"{message_id}","exhaustion":0.1}}"#)
}

pub struct Packs {
    selection: PackSelection,
    available: Vec<AvailablePack>,
    files: PackFiles,
    features: Features,
}

impl Packs {
    pub fn new() -> Self {
        Self {
            selection: PackSelection::default(),
            available: vec![AvailablePack::vanilla()],
            files: PackFiles::default(),
            features: Features {
                known: vec![rl("vanilla"), rl("trade_rebalance")],
                vanilla: vec![rl("vanilla")],
                world: None,
            },
        }
    }

    pub fn file(mut self, pack: &str, path: &str, content: impl Into<Vec<u8>>) -> Self {
        self.files.insert(pack, path, content);
        self
    }

    pub fn world(self, name: &str, files: &[(&str, &str)]) -> Self {
        self.world_with_meta(name, META, files)
    }

    pub fn zip(self, name: &str, files: &[(&str, &str)]) -> Self {
        self.with_pack(AvailablePack::zip(name, Some(META.into())), files)
    }

    pub fn world_with_meta(self, name: &str, meta: &str, files: &[(&str, &str)]) -> Self {
        self.with_pack(AvailablePack::directory(name, meta), files)
    }

    fn with_pack(mut self, pack: AvailablePack, files: &[(&str, &str)]) -> Self {
        let id = pack.id().to_owned();
        self.selection.enabled.push(id.clone());
        self.available.push(pack);
        for (path, content) in files {
            self.files.insert(&id, *path, *content);
        }
        self
    }

    pub fn vanilla(mut self, files: &[(&str, &str)]) -> Self {
        for (path, content) in files {
            self.files.insert("vanilla", *path, *content);
        }
        self
    }

    pub fn feature(mut self, id: &str, meta: Vec<u8>) -> Self {
        self.selection.enabled.push(id.to_owned());
        self.available.push(AvailablePack::feature(id, meta));
        self
    }

    pub fn files_mut(&mut self) -> &mut PackFiles {
        &mut self.files
    }

    pub fn stack(&self) -> (PackStack, Diagnostics) {
        pack_stack(&self.selection, &self.available, &self.features, false)
    }

    pub fn load(
        &self,
        declarations: &[Declaration],
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        self.load_after(declarations, None)
    }

    pub fn load_after(
        &self,
        declarations: &[Declaration],
        previous: Option<&Registries>,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        self.load_with(declarations, previous, &mut |_, decode| decode())
    }

    pub fn load_with(
        &self,
        declarations: &[Declaration],
        previous: Option<&Registries>,
        scope: &mut StageScope<'_>,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        let (stack, _) = self.stack();
        load_with(declarations, Arc::new(stack), &self.files, previous, scope)
    }
}

pub fn names<R: RegistryType>(registries: &Registries) -> Vec<String> {
    registries
        .registry::<R>()
        .expect("the registry is loaded")
        .iter()
        .map(|(_, name, _)| name.to_string())
        .collect()
}

pub fn tag<R: RegistryType>(registries: &Registries, tag: &str) -> Option<Vec<String>> {
    let registry = registries.registry::<R>().expect("the registry is loaded");
    let tags = registries.tags::<R>().expect("every registry has tags");
    let id = tags.by_name(&rl(tag))?;
    Some(
        tags.members(id)
            .map(|member| registry.name(member).unwrap().to_string())
            .collect(),
    )
}

pub fn get<R: RegistryType>(registries: &Registries, name: &str) -> Id<R> {
    registries
        .registry::<R>()
        .expect("the registry is loaded")
        .by_name(&rl(name))
        .unwrap_or_else(|| panic!("{name} is an entry"))
}

pub fn value<'r, R: RegistryType>(registries: &'r Registries, name: &str) -> &'r R::Value {
    registries
        .registry::<R>()
        .expect("the registry is loaded")
        .get(get::<R>(registries, name))
        .unwrap()
}

pub fn report_errors<T>(result: &Result<T, Diagnostics>) -> Vec<Diagnostic> {
    let Err(report) = result else {
        return Vec::new();
    };
    let errors = report.iter().filter(|d| d.report_key().is_some());
    errors.cloned().collect()
}

pub fn report(diagnostics: &Diagnostics) -> Vec<String> {
    diagnostics.iter().map(Diagnostic::to_string).collect()
}
