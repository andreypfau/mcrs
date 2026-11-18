use std::ops::Range;

use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_tooltip::AttackBases;
use mcrs_minecraft_item_tooltip::TooltipInputs;
use mcrs_minecraft_item_tooltip::TooltipView;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Text;

use crate::language::ClientLanguage;
use crate::menu_item_tooltip;
use crate::menu_item_tooltip::prototype_components;

#[derive(Clone, Debug, Default)]
pub struct CreativeSearchIndex {
    entries: Vec<SearchEntry>,
    parts: Vec<String>,
    tags: Vec<usize>,
    tag_names: Vec<ResourceLocationBuf>,
}

#[derive(Clone, Debug)]
struct SearchEntry {
    item: Option<ResourceLocationBuf>,
    parts: Range<usize>,
    tags: Range<usize>,
}

impl CreativeSearchIndex {
    #[must_use]
    pub fn build(
        search_list: &[ItemStack],
        items: &ItemTable,
        registries: &Registries,
        language: Option<&ClientLanguage>,
    ) -> Self {
        let mut index = Self {
            entries: Vec::with_capacity(search_list.len()),
            ..Self::default()
        };
        let item_tags = registries.tags::<Item>();
        let item_count = registries
            .entries::<Item>()
            .map_or(0, |entries| entries.len());
        let mut tag_ids = Vec::new();
        for (tag, name) in item_tags.iter().flat_map(|tags| tags.iter()) {
            tag_ids.push(tag);
            index.tag_names.push(name.clone());
        }
        let names = RegistryNames::new(registries, items);
        let attack = menu_item_tooltip::player_attack_bases(None);
        let mut tooltip = Vec::new();
        for stack in search_list {
            let parts_start = index.parts.len();
            if let Some(language) = language {
                let sources = PartSources {
                    items,
                    names: &names,
                    attack,
                    language,
                };
                index.push_parts(stack, &sources, &mut tooltip);
            }
            let tags_start = index.tags.len();
            if let (Some(tags), Some(id)) = (&item_tags, Id::checked(stack.kind.0, item_count)) {
                let held = tag_ids.iter().enumerate();
                index.tags.extend(
                    held.filter(|(_, tag)| tags.contains(**tag, id))
                        .map(|(at, _)| at),
                );
            }
            index.entries.push(SearchEntry {
                item: items
                    .name(NameTable::Item, stack.kind.0)
                    .map(|name| name.to_buf()),
                parts: parts_start..index.parts.len(),
                tags: tags_start..index.tags.len(),
            });
        }
        index
    }

    #[must_use]
    pub fn matches(&self, stack: usize, query: &SearchQuery) -> bool {
        let Some(entry) = self.entries.get(stack) else {
            return false;
        };
        let Some(item) = &entry.item else {
            return *query == SearchQuery::new("");
        };
        let parts = self.parts.get(entry.parts.clone()).unwrap_or_default();
        let tags = self.tags.get(entry.tags.clone()).unwrap_or_default();
        query.matches(
            item.as_location(),
            tags.iter()
                .filter_map(|tag| self.tag_names.get(*tag))
                .map(ResourceLocationBuf::as_location),
            parts.iter().map(String::as_str),
        )
    }

    pub fn matching_tags<'a>(
        &'a self,
        stack: usize,
        query: &'a SearchQuery,
    ) -> impl Iterator<Item = &'a ResourceLocationBuf> + 'a {
        let entry = self.entries.get(stack);
        let item = entry.and_then(|entry| entry.item.as_ref());
        let tags = entry
            .and_then(|entry| self.tags.get(entry.tags.clone()))
            .unwrap_or_default();
        tags.iter()
            .filter_map(|tag| self.tag_names.get(*tag))
            .filter(move |tag| {
                item.is_some_and(|item| query.matches(item.as_location(), [tag.as_location()], []))
            })
    }

    fn push_parts(&mut self, stack: &ItemStack, sources: &PartSources, tooltip: &mut Vec<Text>) {
        let mut inputs =
            TooltipInputs::new(sources.attack, prototype_components(sources.items, stack));
        inputs.view = TooltipView::Creative;
        tooltip.clear();
        mcrs_minecraft_item_tooltip::append_tooltip_parts(
            stack,
            sources.items,
            sources.names,
            &inputs,
            tooltip,
        );
        for part in tooltip.iter() {
            let part = search_part(&sources.language.plain_text(part));
            if !part.is_empty() {
                self.parts.push(part);
            }
        }
    }
}

struct PartSources<'a> {
    items: &'a ItemTable,
    names: &'a RegistryNames<'a>,
    attack: AttackBases,
    language: &'a ClientLanguage,
}

fn search_part(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            plain.push(c);
        }
    }
    plain
        .trim_matches(' ')
        .chars()
        .flat_map(char::to_lowercase)
        .collect()
}
