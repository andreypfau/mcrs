use alloc::vec::Vec;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::VarInt;

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
use crate::builtin_key::NAMESPACE;
use crate::check_saved_form::check_saved_form;
use crate::saved::is_air;
use crate::wire_value::put_length;
use crate::wire_value::put_var_int;
use crate::wire_value::read_value;
use crate::wire_value::write_value;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entries {
    Bare,
    LengthPrefixed,
}

/// Reads the wire stack. A count of 0 or less, or the item air, gives `None`, the empty stack.
///
/// The reader accepts a count up to 255 and does not check the ranges of the components. It
/// gives an error for a component that this crate cannot read: without a length prefix, the
/// reader cannot find the end of that component.
pub fn read_wire<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<Option<ItemStack>, WireError> {
    read_frame(input, names, Entries::Bare)
}

/// Reads the wire stack that is not empty. The empty stack is an error.
pub fn read_wire_not_empty<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<ItemStack, WireError> {
    read_wire(input, names)?.ok_or(WireError::EmptyStack)
}

/// Reads the wire stack with length prefix. A count of 0 or less, or the item air, gives `None`,
/// the empty stack.
///
/// A component that this crate does not decode becomes a [`PatchEntry::UndecodedWire`]. Each
/// decoded part of the stack must have a saved form: the count is at most 99, each number is in
/// the range of its saved form, and each numeric id has a name.
pub fn read_wire_with_length<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<Option<ItemStack>, WireError> {
    let stack = read_frame(input, names, Entries::LengthPrefixed)?;
    if let Some(stack) = &stack {
        check_saved_form(stack, names)?;
    }
    Ok(stack)
}

/// Appends the wire stack to `output`. `None` and a stack of the item air give the byte `00`.
///
/// The writer omits a custom component, which has no type id. Each decoded part of the stack must
/// have a saved form, as for [`read_wire_with_length`]. On an error, `output` keeps its length.
pub fn write_wire<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    write_frame(stack, names, Entries::Bare, output)
}

/// Appends the wire stack that is not empty to `output`. A stack of the item air is an error.
pub fn write_wire_not_empty<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    if present(Some(stack), names)?.is_none() {
        return Err(WireError::EmptyStack);
    }
    write_wire(Some(stack), names, output)
}

/// Appends the wire stack with length prefix to `output`, as [`write_wire`] does.
pub fn write_wire_with_length<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    write_frame(stack, names, Entries::LengthPrefixed, output)
}

fn frame_var_int(input: &mut &[u8]) -> Result<i32, WireError> {
    VarInt::decode(input)
        .map(|VarInt(value)| value)
        .map_err(|error| WireError::of_frame(&error))
}

fn frame_length(input: &mut &[u8]) -> Result<usize, WireError> {
    usize::try_from(frame_var_int(input)?).map_err(|_| WireError::Malformed)
}

fn read_frame<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
    entries: Entries,
) -> Result<Option<ItemStack>, WireError> {
    let count = frame_var_int(input)?;
    if count <= 0 {
        return Ok(None);
    }
    let count = u8::try_from(count)
        .ok()
        .and_then(Count::new)
        .ok_or(WireError::CountTooLarge)?;
    let kind = u16::try_from(frame_var_int(input)?).map_err(|_| WireError::UnknownItem)?;
    let item = names
        .name(NameTable::Item, kind)
        .ok_or(WireError::UnknownItem)?;
    let air = is_air(item);
    let values = frame_length(input)?;
    let removals = frame_length(input)?;
    let mut stack = ItemStack::new(Kind(kind), count);
    for _ in 0..values {
        let component = read_type(input, names)?;
        let entry = match entries {
            Entries::Bare => read_bare_value(input, names, &component)?,
            Entries::LengthPrefixed => read_prefixed_value(input, names, &component)?,
        };
        put_new(&mut stack, entry, component.id)?;
    }
    for _ in 0..removals {
        let component = read_type(input, names)?;
        let entry = match component.builtin {
            Some(key) => PatchEntry::Removal(key),
            None => PatchEntry::UndecodedRemoval(component.name.to_buf()),
        };
        put_new(&mut stack, entry, component.id)?;
    }
    Ok((!air).then_some(stack))
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
    let raw = frame_var_int(input)?;
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
) -> Result<PatchEntry, WireError> {
    let unsupported = WireError::UnsupportedComponentType(component.id);
    let key = component.builtin.ok_or(unsupported)?;
    let value = read_value(key, input, names)?.ok_or(unsupported)?;
    Ok(PatchEntry::Value(value))
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
        let read = read_value(key, &mut value_input, names).map_err(|error| match error {
            WireError::UnexpectedEnd => bad_length,
            other => other,
        })?;
        if let Some(value) = read {
            if !value_input.is_empty() {
                return Err(bad_length);
            }
            return Ok(PatchEntry::Value(value));
        }
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

fn present<'stack, L: NameLookup>(
    stack: Option<&'stack ItemStack>,
    names: &L,
) -> Result<Option<&'stack ItemStack>, WireError> {
    let Some(stack) = stack else {
        return Ok(None);
    };
    let item = names
        .name(NameTable::Item, stack.kind.0)
        .ok_or(WireError::UnnamedId(NameTable::Item))?;
    Ok((!is_air(item)).then_some(stack))
}

fn write_frame<L: NameLookup>(
    stack: Option<&ItemStack>,
    names: &L,
    entries: Entries,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let start = output.len();
    let written = match present(stack, names) {
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
    let patch = stack.patch();
    if patch
        .iter()
        .any(|entry| matches!(entry, PatchEntry::Undecoded(_)))
    {
        return Err(WireError::SavedValueOnly);
    }
    put_var_int(output, i32::from(stack.count.get()));
    put_var_int(output, i32::from(stack.kind.0));
    put_length(output, patch.iter().filter(|entry| is_value(entry)).count())?;
    put_length(
        output,
        patch.iter().filter(|entry| is_removal(entry)).count(),
    )?;
    for entry in patch {
        if is_value(entry) {
            put_var_int(output, type_id(entry, names)?);
            match entries {
                Entries::Bare => write_entry_value(entry, names, output)?,
                Entries::LengthPrefixed => write_prefixed_value(entry, names, output)?,
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
        PatchEntry::Value(value) => builtin_name(value.key())?,
        PatchEntry::Removal(key) => builtin_name(*key)?,
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

fn builtin_name(key: BuiltinKey) -> Result<ResourceLocation<'static>, WireError> {
    ResourceLocation::new(NAMESPACE, key.path()).map_err(|_| WireError::UnregisteredType)
}

fn write_entry_value<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    match entry {
        PatchEntry::Value(value) => write_value(value, names, output),
        PatchEntry::UndecodedWire(value) => {
            output.extend_from_slice(value.bytes());
            Ok(())
        }
        _ => Ok(()),
    }
}

// The length comes before the value, so the value is written first and the length is rotated in
// front of it. That needs no second buffer.
fn write_prefixed_value<L: NameLookup>(
    entry: &PatchEntry,
    names: &L,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    let start = output.len();
    write_entry_value(entry, names, output)?;
    let length = output.len().saturating_sub(start);
    let before = output.len();
    put_length(output, length)?;
    let size = output.len().saturating_sub(before);
    if let Some(tail) = output.get_mut(start..) {
        tail.rotate_right(size);
    }
    Ok(())
}
