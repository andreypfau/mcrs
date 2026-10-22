use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::{Id, ResourceLocation};
use mcrs_minecraft_registries::{
    AvailablePack, Declaration, Diagnostic, Diagnostics, Features, IdSet, Key, KeyOr, KeySet,
    Namer, PackFiles, PackSelection, PackStack, Ref, Registries, RegistryType, Resolve,
    ResolveError, Resolver, Stage, load, pack_stack,
};
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Damage {
    pub message_id: String,
    pub exhaustion: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Opaque;

impl<'de> Deserialize<'de> for Opaque {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        IgnoredAny::deserialize(d).map(|_| Opaque)
    }
}

mcrs_minecraft_registries::resolve_as_is!(Damage, Opaque);

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

#[derive(Deserialize)]
pub struct NamedEnchant {
    supported_items: Option<KeySet<Item>>,
    exclusive_set: Option<KeySet<Enchantment>>,
    damage_types: Option<KeySet<DamageType>>,
    merchant_predicate: Option<KeyOr<Predicate, Opaque>>,
    damage: Option<KeyOr<DamageType, Damage>>,
    preferred: Option<Key<Enchantment>>,
}

// `preferred` drops a name it cannot resolve, so a miss the value recovers from can be observed.
impl Resolve for Enchant {
    type Named = NamedEnchant;

    fn resolve<C: Resolver>(named: NamedEnchant, ctx: &C) -> Result<Self, ResolveError> {
        Ok(Enchant {
            supported_items: Resolve::resolve(named.supported_items, ctx)
                .map_err(|e| e.at("supported_items"))?,
            exclusive_set: Resolve::resolve(named.exclusive_set, ctx)
                .map_err(|e| e.at("exclusive_set"))?,
            damage_types: Resolve::resolve(named.damage_types, ctx)
                .map_err(|e| e.at("damage_types"))?,
            merchant_predicate: Resolve::resolve(named.merchant_predicate, ctx)
                .map_err(|e| e.at("merchant_predicate"))?,
            damage: Resolve::resolve(named.damage, ctx).map_err(|e| e.at("damage"))?,
            preferred: named.preferred.and_then(|key| ctx.id(&key).ok()),
        })
    }

    fn name<C: Namer>(&self, _: &C) -> Result<NamedEnchant, ResolveError> {
        Err(ResolveError::new("a test enchantment is never written"))
    }
}

pub fn declarations() -> Vec<Declaration> {
    declarations_with(Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into())
}

pub fn declarations_with(damage_type: Declaration) -> Vec<Declaration> {
    vec![
        Declaration::fixed::<Block>(
            rl("block"),
            ["stone", "dirt", "a", "b", "c", "d", "x", "y"]
                .map(rl)
                .to_vec(),
        ),
        Declaration::fixed::<Item>(rl("item"), vec![rl("stone"), rl("dirt")]),
        damage_type,
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
        let (stack, _) = self.stack();
        load(declarations, Arc::new(stack), &self.files, previous)
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
