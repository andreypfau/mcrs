use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{EnchantmentId, Kind};
use mcrs_minecraft_loot_condition::IntProvider;
use mcrs_minecraft_resource::{Id, ResourceLocation, ResourceLocationBuf, TagKey};

use crate::baker::Baker;
use crate::loot_issue::LootProblem;
use crate::loot_table_file::TagName;
use crate::modifier::{Modifier, ModifierOp, SetLevel};
use crate::modifier_file::{
    BonusFormula, KeySetFile, LimitFile, ModifierElement, ModifierFile, ModifierKind,
    ModifierObject, ModifierSetFile,
};

impl Baker<'_, '_> {
    pub(crate) fn modifiers(&mut self, file: Option<&ModifierFile>, path: &str) -> Box<[Modifier]> {
        match file {
            None => Box::new([]),
            Some(ModifierFile::Many(list)) => self.elements(list, path),
            Some(ModifierFile::One(one)) => Box::new([self.modifier(one, path)]),
            Some(ModifierFile::Key(_) | ModifierFile::Tag(_)) => Box::new([unresolved()]),
        }
    }

    fn elements(&mut self, list: &[ModifierElement], path: &str) -> Box<[Modifier]> {
        list.iter()
            .enumerate()
            .map(|(number, element)| match element {
                ModifierElement::Key(_) => unresolved(),
                ModifierElement::Inline(one) => self.modifier(one, &format!("{path}[{number}]")),
            })
            .collect()
    }

    fn modifier(&mut self, file: &ModifierObject, path: &str) -> Modifier {
        let condition = file
            .condition
            .as_ref()
            .map(|condition| self.condition(condition, &format!("{path}.condition")));
        let op = match &file.kind {
            ModifierKind::SetCount { count, add } => ModifierOp::SetCount {
                count: self.conditions.int_provider(count),
                add: *add,
            },
            ModifierKind::ExplosionDecay => ModifierOp::ExplosionDecay,
            ModifierKind::LimitCount {
                limit: LimitFile::Exact(value),
            } => {
                let value = IntProvider::constant(*value);
                ModifierOp::Limit {
                    min: Some(value.clone()),
                    max: Some(value),
                }
            }
            ModifierKind::LimitCount {
                limit: LimitFile::Range { min, max },
            } => ModifierOp::Limit {
                min: min.as_ref().map(|min| self.conditions.int_provider(min)),
                max: max.as_ref().map(|max| self.conditions.int_provider(max)),
            },
            ModifierKind::ApplyBonus {
                enchantment,
                formula,
            } => {
                let enchantment = self.enchantment(enchantment.as_location(), path);
                bonus(*formula, enchantment)
            }
            ModifierKind::EnchantedCountIncrease {
                enchantment, count, ..
            } => {
                self.enchantment(enchantment.as_location(), path);
                self.conditions.float_provider(count);
                ModifierOp::EnchantedCountIncrease
            }
            ModifierKind::Discard => ModifierOp::Discard,
            ModifierKind::SetItem { item } if is_air(item) => {
                self.problems
                    .push((path.to_owned(), LootProblem::SetItemAir));
                ModifierOp::Unresolved
            }
            ModifierKind::SetItem { item } => match self.lookup::<Item>(item.as_location()) {
                Some(item) => ModifierOp::SetItem(Kind(item.number())),
                None => {
                    self.problems
                        .push((path.to_owned(), LootProblem::UnknownItem(item.to_string())));
                    ModifierOp::Unresolved
                }
            },
            ModifierKind::SetDamage { damage, add } => ModifierOp::SetDamage {
                damage: self.conditions.float_provider(damage),
                add: *add,
            },
            ModifierKind::EnchantRandomly {
                options,
                only_compatible,
                ..
            } => ModifierOp::EnchantRandomly {
                options: options
                    .as_ref()
                    .map(|options| self.enchantment_set(options, path)),
                only_compatible: *only_compatible,
            },
            ModifierKind::SetEnchantments { enchantments, add } => {
                let levels = enchantments
                    .0
                    .iter()
                    .filter_map(|entry| {
                        let level = self.conditions.int_provider(&entry.number);
                        let enchantment = self.enchantment(entry.key.as_location(), path)?;
                        Some(SetLevel {
                            enchantment: enchantment_id(enchantment)?,
                            level,
                        })
                    })
                    .collect();
                ModifierOp::SetEnchantments { levels, add: *add }
            }
            ModifierKind::Sequence { functions } => ModifierOp::Sequence(match functions {
                ModifierSetFile::Many(list) => self.elements(list, &format!("{path}.functions")),
                ModifierSetFile::One(ModifierElement::Inline(one)) => {
                    Box::new([self.modifier(one, &format!("{path}.functions"))])
                }
                ModifierSetFile::One(ModifierElement::Key(_)) | ModifierSetFile::Tag(_) => {
                    Box::new([unresolved()])
                }
            }),
            _ => ModifierOp::Pending,
        };
        self.take_problems(path);
        self.check(op.reads(), path);
        Modifier { condition, op }
    }

    /// The enchantments of `options` in set order (LOOT-MOD-018).
    fn enchantment_set(&mut self, options: &KeySetFile, path: &str) -> Box<[EnchantmentId]> {
        let path = format!("{path}.options");
        match options {
            KeySetFile::Tag(TagName(name)) => {
                let tags = self.registries.tags::<Enchantment>();
                let members: Option<Box<[EnchantmentId]>> = tags.as_ref().and_then(|tags| {
                    let tag = tags.id(TagKey::new(name.as_location()))?;
                    Some(tags.members(tag).filter_map(enchantment_id).collect())
                });
                members.unwrap_or_else(|| {
                    self.problems
                        .push((path, LootProblem::UnknownTag(name.to_string())));
                    Box::new([])
                })
            }
            KeySetFile::Many(names) => names
                .iter()
                .filter_map(|name| self.enchantment(name.as_location(), &path))
                .filter_map(enchantment_id)
                .collect(),
            KeySetFile::One(name) => self
                .enchantment(name.as_location(), &path)
                .and_then(enchantment_id)
                .into_iter()
                .collect(),
        }
    }

    pub(crate) fn enchantment(
        &mut self,
        name: ResourceLocation<'_>,
        path: &str,
    ) -> Option<Id<Enchantment>> {
        let id = self.lookup::<Enchantment>(name);
        if id.is_none() {
            self.problems.push((
                path.to_owned(),
                LootProblem::UnknownEnchantment(name.to_string()),
            ));
        }
        id
    }
}

// chisle: the registry `item_modifier` is not declared, so a key or a tag of it applies nothing;
// lifted by its declaration and a step of the reference walk of the table links.
fn unresolved() -> Modifier {
    Modifier {
        condition: None,
        op: ModifierOp::Unresolved,
    }
}

fn bonus(formula: BonusFormula, enchantment: Option<Id<Enchantment>>) -> ModifierOp {
    match formula {
        BonusFormula::OreDrops { .. } => ModifierOp::OreDrops(enchantment),
        BonusFormula::UniformBonusCount { parameters } => ModifierOp::UniformBonus {
            enchantment,
            multiplier: parameters.bonus_multiplier,
        },
        BonusFormula::BinomialWithBonusCount { parameters } => ModifierOp::BinomialBonus {
            enchantment,
            extra: parameters.extra,
            probability: parameters.probability,
        },
    }
}

fn is_air(item: &ResourceLocationBuf) -> bool {
    item.namespace() == "minecraft" && item.path() == "air"
}

fn enchantment_id(id: Id<Enchantment>) -> Option<EnchantmentId> {
    u16::try_from(id.index()).ok().map(EnchantmentId)
}
