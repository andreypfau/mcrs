use alloc::vec::Vec;

use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_resource::ResourceLocation;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::Count;
use crate::ItemStack;
use crate::Kind;
use crate::NameLookup;
use crate::NameTable;
use crate::PatchEntry;
use crate::UndecodedWireValue;
use crate::WireError;
use crate::check_saved_form::check_count;
use crate::check_saved_form::check_saved_form;
use crate::nbt_depth::STACK_LEVEL;
use crate::nbt_depth::check_nbt_depth;
use crate::saved::is_air;
use crate::saved_form::Fault;
use crate::wire_value::put_length;
use crate::wire_value::put_var_int;
use crate::wire_value::read_value_at;
use crate::wire_value::read_var_int;
use crate::wire_value::write_value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entries {
    Bare,
    LengthPrefixed,
}

pub fn read_wire<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<Option<ItemStack>, WireError> {
    read_frame(input, names, Entries::Bare)
}

pub fn read_wire_not_empty<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<ItemStack, WireError> {
    read_wire(input, names)?.ok_or(WireError::EmptyStack)
}

pub fn read_wire_with_length<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<Option<ItemStack>, WireError> {
    let stack = read_frame(input, names, Entries::LengthPrefixed)?;
    if let Some(stack) = &stack {
        check_count(stack)?;
    }
    Ok(stack)
}

pub fn write_wire<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    write_frame(stack, names, Entries::Bare, output)
}

pub fn write_wire_not_empty<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    if present(Some(stack), names, WireError::UnnamedId(NameTable::Item))?.is_none() {
        return Err(WireError::EmptyStack);
    }
    write_wire(Some(stack), names, output)
}

pub fn write_wire_with_length<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    write_frame(stack, names, Entries::LengthPrefixed, output)
}

fn frame_length(input: &mut &[u8]) -> Result<usize, WireError> {
    usize::try_from(read_var_int(input, WireError::of_frame)?).map_err(|_| WireError::Malformed)
}

fn read_frame<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    entries: Entries,
) -> Result<Option<ItemStack>, WireError> {
    mcrs_minecraft_nbt::with_nbt_budget(NbtBudget::default(), || {
        read_budgeted_frame(input, names, entries)
    })
    .unwrap_or(Err(WireError::QuotaExceeded))
}

fn read_budgeted_frame<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    entries: Entries,
) -> Result<Option<ItemStack>, WireError> {
    let count = read_var_int(input, WireError::of_frame)?;
    if count <= 0 {
        return Ok(None);
    }
    let count = u8::try_from(count)
        .ok()
        .and_then(Count::new)
        .ok_or(WireError::CountTooLarge)?;
    let kind = u16::try_from(read_var_int(input, WireError::of_frame)?)
        .map_err(|_| WireError::UnknownItem)?;
    let item = names
        .name(NameTable::Item, kind)
        .ok_or(WireError::UnknownItem)?;
    let mut stack = ItemStack::new(Kind(kind), count);
    read_patch(input, names, entries, STACK_LEVEL, &mut stack)?;
    if is_air(item) {
        return Ok(None);
    }
    check_nbt_depth(&stack, names).map_err(Fault::into_wire)?;
    Ok(Some(stack))
}

pub(crate) fn read_patch<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    entries: Entries,
    depth: u32,
    stack: &mut ItemStack,
) -> Result<(), WireError> {
    let values = frame_length(input)?;
    let removals = frame_length(input)?;
    for _ in 0..values {
        let component = read_type(input, names)?;
        match entries {
            Entries::Bare => read_bare_value(input, names, &component, depth),
            Entries::LengthPrefixed => read_prefixed_value(input, names, &component),
        }
        .and_then(|entry| put_new(stack, entry, component.id))?;
    }
    read_removals(input, names, removals, stack)
}

#[inline(never)]
fn read_removals<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    removals: usize,
    stack: &mut ItemStack,
) -> Result<(), WireError> {
    for _ in 0..removals {
        let component = read_type(input, names)?;
        let entry = match component.builtin {
            Some(key) => PatchEntry::Removal(key),
            None => PatchEntry::UndecodedRemoval(component.name.to_buf()),
        };
        put_new(stack, entry, component.id)?;
    }
    Ok(())
}

struct TypeRef<'names> {
    id: ComponentType,
    name: ResourceLocation<'names>,
    builtin: Option<BuiltinKey>,
}

fn read_type<'names, L: NameLookup>(
    input: &mut &[u8],
    names: &'names L,
) -> Result<TypeRef<'names>, WireError> {
    let raw = read_var_int(input, WireError::of_frame)?;
    let id = u16::try_from(raw).map_err(|_| WireError::UnknownComponentType(raw))?;
    let name = names
        .name(NameTable::ComponentType, id)
        .ok_or(WireError::UnknownComponentType(raw))?;
    Ok(TypeRef {
        id: ComponentType(id),
        name,
        builtin: BuiltinKey::from_location(name),
    })
}

fn read_bare_value<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    component: &TypeRef<'_>,
    depth: u32,
) -> Result<PatchEntry, WireError> {
    let unsupported = WireError::UnsupportedComponentType(component.id);
    let key = component.builtin.ok_or(unsupported)?;
    read_value_at(key, input, names, depth).map(PatchEntry::Value)
}

fn read_prefixed_value<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    component: &TypeRef<'_>,
) -> Result<PatchEntry, WireError> {
    let length = frame_length(input)?;
    let (bytes, rest) = input
        .split_at_checked(length)
        .ok_or(WireError::UnexpectedEnd)?;
    *input = rest;
    if let Some(key) = component.builtin {
        let mut value_input = bytes;
        let bad_length = WireError::BadLength(component.id);
        let value =
            read_value_at(key, &mut value_input, names, STACK_LEVEL).map_err(
                |error| match error {
                    WireError::UnexpectedEnd => bad_length,
                    other => other,
                },
            )?;
        if !value_input.is_empty() {
            return Err(bad_length);
        }
        return Ok(PatchEntry::Value(value));
    }
    Ok(PatchEntry::UndecodedWire(UndecodedWireValue::new(
        component.id,
        component.builtin,
        bytes,
    )))
}

fn put_new(
    stack: &mut ItemStack,
    entry: PatchEntry,
    component: ComponentType,
) -> Result<(), WireError> {
    let before = stack.patch().len();
    stack.put_entry(entry);
    if stack.patch().len() == before {
        return Err(WireError::RepeatedComponentType(component));
    }
    Ok(())
}

pub(crate) fn present<'stack, L: NameLookup, E>(
    stack: Option<&'stack ItemStack>,
    names: &L,
    unnamed: E,
) -> Result<Option<&'stack ItemStack>, E> {
    let Some(stack) = stack else {
        return Ok(None);
    };
    let item = names.name(NameTable::Item, stack.kind.0).ok_or(unnamed)?;
    Ok((!is_air(item)).then_some(stack))
}

fn write_frame<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    entries: Entries,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let start = output.len();
    let written = match present(stack, names, WireError::UnnamedId(NameTable::Item)) {
        Ok(Some(stack)) => write_present(stack, names, entries, output),
        Ok(None) => {
            put_var_int(output, 0);
            Ok(())
        }
        Err(error) => Err(error),
    };
    if written.is_err() {
        output.truncate(start);
    }
    written
}

fn write_present<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
    entries: Entries,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    check_saved_form(stack, names)?;
    put_var_int(output, i32::from(stack.count.get()));
    put_var_int(output, i32::from(stack.kind.0));
    write_patch(stack, names, entries, STACK_LEVEL, output)
}

pub(crate) fn write_patch<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
    entries: Entries,
    depth: u32,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let patch = stack.patch();
    if patch
        .iter()
        .any(|entry| matches!(entry, PatchEntry::Undecoded(_)))
    {
        return Err(WireError::SavedValueOnly);
    }
    put_length(output, patch.iter().filter(|entry| is_value(entry)).count())?;
    put_length(
        output,
        patch.iter().filter(|entry| is_removal(entry)).count(),
    )?;
    for entry in patch {
        if is_value(entry) {
            put_var_int(output, type_id(entry, names)?);
            match entries {
                Entries::Bare => write_entry_value(entry, names, depth, output)?,
                Entries::LengthPrefixed => write_prefixed_value(entry, names, depth, output)?,
            }
        }
    }
    for entry in patch {
        if is_removal(entry) {
            put_var_int(output, type_id(entry, names)?);
        }
    }
    Ok(())
}

fn is_value(entry: &PatchEntry) -> bool {
    matches!(entry, PatchEntry::Value(_) | PatchEntry::UndecodedWire(_))
}

fn is_removal(entry: &PatchEntry) -> bool {
    matches!(
        entry,
        PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_)
    )
}

fn type_id<L: NameLookup>(entry: &PatchEntry, names: &L) -> Result<i32, WireError> {
    let name = match entry {
        PatchEntry::Value(value) => value.key().location().ok_or(WireError::UnregisteredType)?,
        PatchEntry::Removal(key) => key.location().ok_or(WireError::UnregisteredType)?,
        PatchEntry::UndecodedRemoval(name) => name.as_location(),
        PatchEntry::UndecodedWire(value) => return Ok(i32::from(value.component().0)),
        PatchEntry::Undecoded(_) | PatchEntry::Custom(_) => {
            return Err(WireError::UnregisteredType);
        }
    };
    names
        .id(NameTable::ComponentType, name)
        .map(i32::from)
        .ok_or(WireError::UnregisteredType)
}

fn write_entry_value<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    depth: u32,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    match entry {
        PatchEntry::Value(value) => write_value(value, names, depth, output),
        PatchEntry::UndecodedWire(value) => {
            output.extend_from_slice(value.bytes());
            Ok(())
        }
        _ => Ok(()),
    }
}

fn write_prefixed_value<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    depth: u32,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let start = output.len();
    write_entry_value(entry, names, depth, output)?;
    let length = output.len().saturating_sub(start);
    let before = output.len();
    put_length(output, length)?;
    let size = output.len().saturating_sub(before);
    if let Some(tail) = output.get_mut(start..) {
        tail.rotate_right(size);
    }
    Ok(())
}
