use std::ops::Range;

use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_tooltip::item_name;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::language::ClientLanguage;

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
        for stack in search_list {
            let parts_start = index.parts.len();
            if let Some(language) = language {
                index.push_name(stack, items, language);
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

    fn push_name(&mut self, stack: &ItemStack, items: &ItemTable, language: &ClientLanguage) {
        if let Some(name) = item_name(stack, items) {
            let part = search_part(&language.plain_text(&name));
            if !part.is_empty() {
                self.parts.push(part);
            }
        }
    }
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
