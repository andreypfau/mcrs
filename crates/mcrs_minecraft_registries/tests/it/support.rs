use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_datapack::{
    AvailablePack, Features, PackDiagnostic, PackFiles, PackSelection, PackStack, pack_stack,
};
use mcrs_minecraft_registries::{
    Declaration, Diagnostic, Diagnostics, IdSet, KeySet, Named, Namer, Ref, Registries, Resolve,
    ResolveError, Resolved, Resolver, Stage,
};
use mcrs_minecraft_resource::{
    Id, Registered, RegistryKey, RegistryValue, ResourceKeyBuf, ResourceLocationBuf, TagKeyBuf, rl,
};
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};

pub fn rl(text: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(text).unwrap()
}

pub const META: &str = r#"{"pack":{"description":"x","min_format":122,"max_format":122}}"#;

pub fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

macro_rules! registries {
    ($($marker:ident = $key:literal),+ $(,)?) => {$(
        pub struct $marker;

        impl Registered for $marker {
            const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!($key));
        }
    )+};
}

registries!(
    Block = "minecraft:block",
    Item = "minecraft:item",
    DamageType = "minecraft:damage_type",
    Enchantment = "minecraft:enchantment",
);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Damage {
    pub message_id: String,
    pub exhaustion: f32,
}

impl RegistryValue for Damage {
    type Registry = DamageType;
}

/// A registry whose values the tests never look into.
macro_rules! opaque_registries {
    ($($name:ident = $key:literal),+ $(,)?) => {$(
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name;

        impl Registered for $name {
            const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!($key));
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                IgnoredAny::deserialize(d).map(|_| $name)
            }
        }
    )+};
}

opaque_registries!(
    LootTable = "minecraft:loot_table",
    Predicate = "minecraft:predicate",
    VillagerTrade = "minecraft:villager_trade",
    Things = "t:things",
);

mcrs_minecraft_registries::resolve_as_is!(Damage, LootTable, Predicate, VillagerTrade, Things);

#[derive(Debug)]
pub struct Enchant {
    pub supported_items: Option<IdSet<Item>>,
    pub exclusive_set: Option<IdSet<Enchantment>>,
    pub damage_types: Option<IdSet<DamageType>>,
    pub merchant_predicate: Option<Ref<Resolved, Predicate, Predicate>>,
    pub damage: Option<Ref<Resolved, DamageType, Damage>>,
    pub preferred: Option<Id<Enchantment>>,
}

#[derive(Deserialize)]
pub struct NamedEnchant {
    supported_items: Option<KeySet<Item>>,
    exclusive_set: Option<KeySet<Enchantment>>,
    damage_types: Option<KeySet<DamageType>>,
    merchant_predicate: Option<Ref<Named, Predicate, Predicate>>,
    damage: Option<Ref<Named, DamageType, Damage>>,
    preferred: Option<ResourceKeyBuf<Enchantment>>,
}

impl RegistryValue for Enchant {
    type Registry = Enchantment;
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
    declarations_with(Declaration::data::<Damage>().into())
}

pub fn declarations_with(damage_type: Declaration) -> Vec<Declaration> {
    vec![
        Declaration::fixed(
            rl("block"),
            ["stone", "dirt", "a", "b", "c", "d", "x", "y"]
                .map(rl)
                .to_vec(),
        ),
        Declaration::fixed(rl("item"), vec![rl("stone"), rl("dirt")]),
        damage_type,
        Declaration::data::<Enchant>().into(),
        Declaration::from(Declaration::data::<LootTable>()).in_stage(Stage::Reloadable),
        Declaration::from(Declaration::data::<Predicate>()).in_stage(Stage::Reloadable),
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
        let mut features = Features::vanilla();
        features.known.push(rl("trade_rebalance"));
        Self {
            selection: PackSelection::default(),
            available: vec![AvailablePack::vanilla()],
            files: PackFiles::default(),
            features,
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

    pub fn stack(&self) -> (PackStack, Vec<PackDiagnostic>) {
        pack_stack(&self.selection, &self.available, &self.features, false)
    }

    pub fn load(
        self,
        declarations: impl IntoIterator<Item = Declaration>,
    ) -> Result<(Registries, Diagnostics), Diagnostics> {
        let (stack, _) = self.stack();
        Registries::load(
            declarations.into_iter().collect(),
            Arc::new(stack),
            self.files,
        )
    }

    pub fn reload(self, previous: &Registries) -> Result<(Registries, Diagnostics), Diagnostics> {
        let (stack, _) = self.stack();
        previous.reload(Arc::new(stack), self.files)
    }
}

pub fn names<R: Registered>(registries: &Registries) -> Vec<String> {
    let entries = registries.entries::<R>().expect("the registry is loaded");
    entries.names().iter().map(ToString::to_string).collect()
}

pub fn tag<R: Registered>(registries: &Registries, tag: &str) -> Option<Vec<String>> {
    let tags = registries.tags::<R>().expect("every registry has tags");
    let id = tags.id(&TagKeyBuf::from_location(rl(tag)))?;
    Some(
        tags.members(id)
            .map(|member| registries.name(member).unwrap().to_string())
            .collect(),
    )
}

pub fn get<R: Registered>(registries: &Registries, name: &str) -> Id<R> {
    registries
        .id(&ResourceKeyBuf::from_location(rl(name)))
        .unwrap_or_else(|e| panic!("{name} is an entry: {e}"))
}

pub fn value<'r, V: RegistryValue + 'static>(registries: &'r Registries, name: &str) -> &'r V {
    registries
        .registry::<V>()
        .expect("the registry is loaded")
        .get(get::<V::Registry>(registries, name))
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
