use alloc::vec::Vec;

use mcrs_minecraft_resource::ResourceLocation;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::HashError;
use crate::HashScratch;
use crate::HashedComponent;
use crate::ItemStack;
use crate::Kind;
use crate::NameLookup;
use crate::NameTable;
use crate::PatchEntry;
use crate::builtin_key::NAMESPACE;
use crate::component_hash::component_hash;
use crate::saved::is_air;

/// The stack that a client states in a click: the item, the count, the hash of each value entry
/// of the patch and the type of each removal entry. The order of the entries has no meaning.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HashedStack {
    pub kind: Kind,
    /// The count as the client sent it. It can be out of the range of a count.
    pub count: i32,
    pub values: Vec<HashedComponent>,
    pub removals: Vec<ComponentType>,
}

/// Computes the hashed stack of `stack`. `None` and a stack of the item air give `None`, the
/// empty stack. A custom component has no type id and is not in the result.
pub fn hash_stack<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    scratch: &mut HashScratch,
) -> Result<Option<HashedStack>, HashError> {
    let Some(stack) = present(stack, names)? else {
        return Ok(None);
    };
    let mut hashed = HashedStack {
        kind: stack.kind,
        count: i32::from(stack.count.get()),
        values: Vec::new(),
        removals: Vec::new(),
    };
    for entry in stack.patch() {
        match hashed_entry(entry, names, scratch)? {
            HashedEntry::Value(value) => hashed.values.push(value),
            HashedEntry::Removal(component) => hashed.removals.push(component),
            HashedEntry::Skipped => {}
        }
    }
    Ok(Some(hashed))
}

/// True when the hashed stack that a client sent matches `stack`. The check makes no allocation
/// beyond `scratch`.
///
/// A present hashed stack of the item air with the count 0 and no entries matches the empty
/// stack. A component that this crate cannot hash gives an error only when each other part
/// matches: the result is then not decided.
pub fn matches_hashed<L: NameLookup>(
    hashed: Option<&HashedStack>,
    stack: Option<&ItemStack>,
    names: &L,
    scratch: &mut HashScratch,
) -> Result<bool, HashError> {
    let (hashed, stack) = match (hashed, present(stack, names)?) {
        (None, None) => return Ok(true),
        (Some(hashed), None) => return Ok(is_empty(hashed, names)),
        (Some(hashed), Some(stack)) => (hashed, stack),
        (None, Some(_)) => return Ok(false),
    };
    if hashed.kind != stack.kind || hashed.count != i32::from(stack.count.get()) {
        return Ok(false);
    }
    let mut values = 0_usize;
    let mut removals = 0_usize;
    let mut undecided: Option<HashError> = None;
    for entry in stack.patch() {
        match hashed_entry(entry, names, scratch) {
            Ok(HashedEntry::Value(value)) => {
                values = values.saturating_add(1);
                if !hashed.values.contains(&value) {
                    return Ok(false);
                }
            }
            Ok(HashedEntry::Removal(component)) => {
                removals = removals.saturating_add(1);
                if !hashed.removals.contains(&component) {
                    return Ok(false);
                }
            }
            Ok(HashedEntry::Skipped) => {}
            Err(error) => {
                if let PatchEntry::UndecodedRemoval(_) = entry {
                    removals = removals.saturating_add(1);
                } else {
                    values = values.saturating_add(1);
                }
                undecided.get_or_insert(error);
            }
        }
    }
    if values != hashed.values.len() || removals != hashed.removals.len() {
        return Ok(false);
    }
    match undecided {
        Some(error) => Err(error),
        None => Ok(true),
    }
}

fn is_empty<L: NameLookup>(hashed: &HashedStack, names: &L) -> bool {
    hashed.count == 0
        && hashed.values.is_empty()
        && hashed.removals.is_empty()
        && names
            .name(NameTable::Item, hashed.kind.0)
            .is_some_and(is_air)
}

enum HashedEntry {
    Value(HashedComponent),
    Removal(ComponentType),
    Skipped,
}

fn hashed_entry<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    scratch: &mut HashScratch,
) -> Result<HashedEntry, HashError> {
    match entry {
        PatchEntry::Value(value) => Ok(HashedEntry::Value(HashedComponent {
            component: builtin_type(value.key(), names)?,
            hash: component_hash(value, names, scratch)?.as_i32(),
        })),
        PatchEntry::Removal(key) => Ok(HashedEntry::Removal(builtin_type(*key, names)?)),
        PatchEntry::UndecodedRemoval(name) => {
            Ok(HashedEntry::Removal(type_of(name.as_location(), names)?))
        }
        PatchEntry::Undecoded(value) => Err(HashError::UndecodedComponent(type_of(
            value.name.as_location(),
            names,
        )?)),
        PatchEntry::UndecodedWire(value) => Err(HashError::UndecodedComponent(value.component())),
        PatchEntry::Custom(_) => Ok(HashedEntry::Skipped),
    }
}

fn builtin_type<L: NameLookup>(key: BuiltinKey, names: &L) -> Result<ComponentType, HashError> {
    let name =
        ResourceLocation::new(NAMESPACE, key.path()).map_err(|_| HashError::UnregisteredType)?;
    type_of(name, names)
}

fn type_of<L: NameLookup>(
    name: ResourceLocation<'_>,
    names: &L,
) -> Result<ComponentType, HashError> {
    names
        .id(NameTable::ComponentType, name)
        .map(ComponentType)
        .ok_or(HashError::UnregisteredType)
}

fn present<'stack, L: NameLookup>(
    stack: Option<&'stack ItemStack>,
    names: &L,
) -> Result<Option<&'stack ItemStack>, HashError> {
    let Some(stack) = stack else {
        return Ok(None);
    };
    let item = names
        .name(NameTable::Item, stack.kind.0)
        .ok_or(HashError::UnknownItem)?;
    Ok((!is_air(item)).then_some(stack))
}
