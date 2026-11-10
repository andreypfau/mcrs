use alloc::vec::Vec;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::VarInt;

use crate::ComponentType;
use crate::HashedComponent;
use crate::HashedStack;
use crate::Kind;
use crate::NameLookup;
use crate::NameTable;
use crate::WireError;
use crate::wire_value::put_length;
use crate::wire_value::put_var_int;

const MAX_ENTRIES: usize = 256;
const ABSENT: u8 = 0x00;
const PRESENT: u8 = 0x01;

/// Reads the hashed stack of a click. The byte `00` gives `None`, the empty stack.
///
/// The reader accepts each count and each item id that fits a numeric id; only the comparison
/// with a stack gives them a meaning. A list of more than 256 entries, or a type id that names no
/// component type, is an error. A value type two times keeps the last hash, and a removal type two
/// times is one removal.
pub fn read_hashed<L: NameLookup>(
    input: &mut &[u8],
    names: &L,
) -> Result<Option<HashedStack>, WireError> {
    let Some((&present, rest)) = input.split_first() else {
        return Err(WireError::UnexpectedEnd);
    };
    *input = rest;
    match present {
        ABSENT => return Ok(None),
        PRESENT => {}
        _ => return Err(WireError::Malformed),
    }
    let kind = u16::try_from(var_int(input)?).map_err(|_| WireError::UnknownItem)?;
    let count = var_int(input)?;
    let value_count = entry_count(input)?;
    let mut values: Vec<HashedComponent> = Vec::with_capacity(value_count);
    for _ in 0..value_count {
        let component = component_type(input, names)?;
        let (hash, rest) = input
            .split_first_chunk::<4>()
            .ok_or(WireError::UnexpectedEnd)?;
        *input = rest;
        let hash = i32::from_be_bytes(*hash);
        match values.iter_mut().find(|value| value.component == component) {
            Some(value) => value.hash = hash,
            None => values.push(HashedComponent { component, hash }),
        }
    }
    let removal_count = entry_count(input)?;
    let mut removals: Vec<ComponentType> = Vec::with_capacity(removal_count);
    for _ in 0..removal_count {
        let component = component_type(input, names)?;
        if !removals.contains(&component) {
            removals.push(component);
        }
    }
    Ok(Some(HashedStack {
        kind: Kind(kind),
        count,
        values,
        removals,
    }))
}

/// Appends the hashed stack to `output`. `None` gives the byte `00`. On an error, `output` keeps
/// its length.
pub fn write_hashed(stack: Option<&HashedStack>, output: &mut Vec<u8>) -> Result<(), WireError> {
    let Some(stack) = stack else {
        output.push(ABSENT);
        return Ok(());
    };
    check_entries(&stack.values, |value| value.component)?;
    check_entries(&stack.removals, |component| *component)?;
    output.push(PRESENT);
    put_var_int(output, i32::from(stack.kind.0));
    put_var_int(output, stack.count);
    put_length(output, stack.values.len())?;
    for value in &stack.values {
        put_var_int(output, i32::from(value.component.0));
        output.extend_from_slice(&value.hash.to_be_bytes());
    }
    put_length(output, stack.removals.len())?;
    for component in &stack.removals {
        put_var_int(output, i32::from(component.0));
    }
    Ok(())
}

fn var_int(input: &mut &[u8]) -> Result<i32, WireError> {
    VarInt::decode(input)
        .map(|VarInt(value)| value)
        .map_err(|error| WireError::of_frame(&error))
}

fn entry_count(input: &mut &[u8]) -> Result<usize, WireError> {
    let count = usize::try_from(var_int(input)?).map_err(|_| WireError::Malformed)?;
    if count > MAX_ENTRIES {
        return Err(WireError::TooManyEntries);
    }
    Ok(count)
}

fn component_type<L: NameLookup>(input: &mut &[u8], names: &L) -> Result<ComponentType, WireError> {
    let raw = var_int(input)?;
    u16::try_from(raw)
        .ok()
        .filter(|id| names.name(NameTable::ComponentType, *id).is_some())
        .map(ComponentType)
        .ok_or(WireError::UnknownComponentType(raw))
}

fn check_entries<T>(entries: &[T], component: fn(&T) -> ComponentType) -> Result<(), WireError> {
    if entries.len() > MAX_ENTRIES {
        return Err(WireError::TooManyEntries);
    }
    let mut rest = entries;
    while let Some((first, others)) = rest.split_first() {
        let first = component(first);
        if others.iter().any(|other| component(other) == first) {
            return Err(WireError::RepeatedComponentType(first));
        }
        rest = others;
    }
    Ok(())
}
