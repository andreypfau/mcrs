use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_resource::ResourceLocation;

struct Row {
    query: &'static str,
    item: &'static str,
    tags: &'static [&'static str],
    parts: &'static [&'static str],
    matches: bool,
}

const SWORD: &str = "minecraft:diamond_sword";
const SWORD_TAGS: &[&str] = &["minecraft:swords", "neo:tools"];
const SWORD_PARTS: &[&str] = &["diamond sword", "+7 attack damage"];

const fn sword(query: &'static str, matches: bool) -> Row {
    Row {
        query,
        item: SWORD,
        tags: SWORD_TAGS,
        parts: SWORD_PARTS,
        matches,
    }
}

const ROWS: &[Row] = &[
    sword("", true),
    sword("diamond", true),
    sword("DiAmOnD", true),
    sword("diamond_sword", false),
    sword("attack", true),
    sword("minecraft:sword", true),
    sword(" mine : sword ", true),
    sword("minecraft:diamond s", true),
    sword("minecraft:attack", true),
    sword("other:sword", false),
    sword(":", true),
    sword("#swords", true),
    sword("#SWORDS", true),
    sword("#tools", true),
    sword("#axes", false),
    sword("#minecraft:swords", true),
    sword("#neo:swords", false),
    sword("#neo:tool", true),
    sword("#", true),
    Row {
        query: "#",
        item: "minecraft:stone",
        tags: &[],
        parts: &["stone"],
        matches: false,
    },
    Row {
        query: "minecraft:sword",
        item: "minecraft:stone",
        tags: &[],
        parts: &["stone"],
        matches: false,
    },
    Row {
        query: "ÉPÉE",
        item: SWORD,
        tags: SWORD_TAGS,
        parts: &["épée en diamant"],
        matches: true,
    },
    Row {
        query: "ΑΣ",
        item: SWORD,
        tags: SWORD_TAGS,
        parts: &["ασ"],
        matches: true,
    },
];

fn location(text: &str) -> ResourceLocation<'_> {
    ResourceLocation::read(text).expect("the test names a valid id")
}

#[test]
fn a_search_text_matches_a_stack_by_its_search_parts_its_item_id_or_its_item_tags() {
    for row in ROWS {
        let query = SearchQuery::new(row.query);
        let matched = query.matches(
            location(row.item),
            row.tags.iter().map(|tag| location(tag)),
            row.parts.iter().copied(),
        );
        assert_eq!(
            matched, row.matches,
            "{:?} against {} with tags {:?} and parts {:?}",
            row.query, row.item, row.tags, row.parts
        );
    }
}
