use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::bail;
use mcrs_minecraft_entity::attribute_bases;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeBases;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::InstanceChange;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::SavedAttributes;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::to_nbt_tag;
use mcrs_minecraft_save::parse_player_dat;
use mcrs_minecraft_save::write_atomic;

const USAGE: &str =
    "usage: mcrs_minecraft_player_attributes <player file> [--set <attribute>=<base>]...";
const ATTRIBUTES: &str = "attributes";

struct BaseEdit {
    attribute: Attribute,
    base: f64,
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args_os().skip(1);
    let path = args.next().context(USAGE)?;
    let edits = base_edits(args)?;
    let path = Path::new(&path);
    let bytes = fs::read(path).with_context(|| format!("{}", path.display()))?;
    let mut dat = parse_player_dat(&bytes, path)?;
    let bases = attribute_bases(EntityType::Player).context("the player has no attribute set")?;
    let saved: SavedAttributes = match dat.get(ATTRIBUTES) {
        Some(tag) => {
            from_tag(tag.clone()).with_context(|| format!("{}: {ATTRIBUTES}", path.display()))?
        }
        None => SavedAttributes::default(),
    };
    let mut instances = saved.into_instances(bases);
    if edits.is_empty() {
        print_header();
        for attribute in bases.attributes() {
            print_row(&instances, bases, attribute);
        }
        return Ok(());
    }
    let loaded = SavedAttributes::from_instances(&instances);
    for edit in &edits {
        if bases.base(edit.attribute).is_none() {
            bail!(
                "{} is not an attribute of the player",
                edit.attribute.as_static_str()
            );
        }
        let _change: InstanceChange =
            instances.apply(edit.attribute, &InstanceEdit::SetBase(edit.base));
    }
    let changed = instances.take_dirty();
    let edited = SavedAttributes::from_instances(&instances);
    if edited == loaded {
        println!("no change; the file is not written");
        return Ok(());
    }
    let written = to_nbt_tag(&edited)?;
    if let Some(note) = rewrite_note(dat.get(ATTRIBUTES), &written, &edits) {
        eprintln!("{note}");
    }
    dat.root.put(ATTRIBUTES, written);
    write_atomic(path, &to_gzip_bytes_vec(&dat)?).with_context(|| format!("{}", path.display()))?;
    print_header();
    for attribute in changed.iter() {
        print_row(&instances, bases, attribute);
    }
    Ok(())
}

fn rewrite_note(file: Option<&NbtTag>, written: &NbtTag, edits: &[BaseEdit]) -> Option<String> {
    let not_edited = |entry: &&NbtTag| {
        compound_attribute(entry)
            .is_none_or(|attribute| edits.iter().all(|edit| edit.attribute != attribute))
    };
    let file_entries: Vec<&NbtTag> = match file {
        Some(NbtTag::List(entries)) => entries.iter().filter(not_edited).collect(),
        Some(other) => vec![other],
        None => Vec::new(),
    };
    let written_entries: Vec<&NbtTag> = match written {
        NbtTag::List(entries) => entries.iter().filter(not_edited).collect(),
        _ => Vec::new(),
    };
    let lost: Vec<&NbtTag> = file_entries
        .iter()
        .copied()
        .filter(|entry| !written_entries.iter().any(|kept| same_tag(entry, kept)))
        .collect();
    if lost.is_empty() {
        let in_order = file_entries.len() == written_entries.len()
            && file_entries
                .iter()
                .zip(&written_entries)
                .all(|(entry, kept)| same_tag(entry, kept));
        return (!in_order)
            .then(|| format!("note: the rewrite sorts the list {ATTRIBUTES} by attribute id"));
    }
    let names = if lost.len() <= 4 {
        let labels: Vec<&str> = lost.iter().map(|entry| entry_label(entry)).collect();
        format!(" ({})", labels.join(", "))
    } else {
        String::new()
    };
    Some(format!(
        "note: the rewrite does not keep {} of {} entries of the list {ATTRIBUTES} as they are{names}; \
         it writes the list as a load and a save of the game do",
        lost.len(),
        file_entries.len()
    ))
}

fn compound_attribute(entry: &NbtTag) -> Option<Attribute> {
    let NbtTag::Compound(compound) = entry else {
        return None;
    };
    Attribute::read(compound.get_string("id")?)
}

fn entry_label(entry: &NbtTag) -> &str {
    match entry {
        NbtTag::Compound(compound) => compound
            .get_string("id")
            .unwrap_or("a compound without an id"),
        _ => "a value that is not a compound",
    }
}

fn same_tag(left: &NbtTag, right: &NbtTag) -> bool {
    match (left, right) {
        (NbtTag::Compound(left), NbtTag::Compound(right)) => {
            left.child_tags.len() == right.child_tags.len()
                && left
                    .child_tags
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|other| same_tag(value, other)))
        }
        (NbtTag::List(left), NbtTag::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(value, other)| same_tag(value, other))
        }
        _ => left == right,
    }
}

fn base_edits(args: impl Iterator<Item = OsString>) -> anyhow::Result<Vec<BaseEdit>> {
    let mut edits = Vec::new();
    let mut args = args.map(OsString::into_string);
    while let Some(flag) = args.next() {
        if flag.as_deref() != Ok("--set") {
            bail!(USAGE);
        }
        let Some(Ok(edit)) = args.next() else {
            bail!(USAGE);
        };
        let (name, base) = edit.split_once('=').context(USAGE)?;
        let attribute =
            Attribute::read(name).with_context(|| format!("unknown attribute {name}"))?;
        let base: f64 = base
            .parse()
            .with_context(|| format!("{base} is not a number"))?;
        if !base.is_finite() {
            bail!("{base} is not a finite number");
        }
        edits.push(BaseEdit { attribute, base });
    }
    Ok(edits)
}

fn print_header() {
    println!("{:<44} {:>22} {:>22}", "attribute", "base", "value");
}

fn print_row(instances: &AttributeInstances, bases: AttributeBases, attribute: Attribute) {
    let (Some(base), Some(value), Some(type_base)) = (
        instances.base(attribute),
        instances.value(attribute),
        bases.base(attribute),
    ) else {
        return;
    };
    let mark = if base.to_bits() == type_base.to_bits() {
        ' '
    } else {
        '*'
    };
    println!(
        "{:<44} {base:>22}{mark}{value:>22}",
        attribute.as_static_str()
    );
    let Some(instance) = instances.set().instance(attribute) else {
        return;
    };
    for modifier in instance.modifiers() {
        println!(
            "  modifier {} {} {}",
            modifier.id(),
            modifier.operation().as_str(),
            modifier.amount()
        );
    }
}
