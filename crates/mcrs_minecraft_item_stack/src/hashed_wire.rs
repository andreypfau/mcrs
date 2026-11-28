use std::io::Write;

use alloc::vec::Vec;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

use crate::ComponentType;
use crate::HashedComponent;
use crate::HashedStack;
use crate::Kind;
use crate::WireError;
use crate::data_component_type_keys::DataComponentType;
use crate::wire_frame::codec_error;
use crate::wire_value::read_var_int;

const MAX_ENTRIES: usize = 256;

impl Encode for HashedStack {
    fn encode(&self, mut w: impl Write) -> mcrs_minecraft_wire::Result<()> {
        let values = check_entries(&self.values, |value| value.component).map_err(codec_error)?;
        let removals =
            check_entries(&self.removals, |component| *component).map_err(codec_error)?;
        VarInt(i32::from(self.kind.0)).encode(&mut w)?;
        VarInt(self.count).encode(&mut w)?;
        values.encode(&mut w)?;
        for value in &self.values {
            VarInt(i32::from(value.component.0)).encode(&mut w)?;
            value.hash.encode(&mut w)?;
        }
        removals.encode(&mut w)?;
        for component in &self.removals {
            VarInt(i32::from(component.0)).encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for HashedStack {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
        read_hashed(r).map_err(codec_error)
    }
}

fn read_hashed(input: &mut &[u8]) -> Result<HashedStack, WireError> {
    let kind = u16::try_from(read_var_int(input, WireError::of_frame)?)
        .map_err(|_| WireError::UnknownItem)?;
    let count = read_var_int(input, WireError::of_frame)?;
    let value_count = entry_count(input)?;
    let mut values: Vec<HashedComponent> = Vec::with_capacity(value_count);
    for _ in 0..value_count {
        let component = component_type(input)?;
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
        let component = component_type(input)?;
        if !removals.contains(&component) {
            removals.push(component);
        }
    }
    Ok(HashedStack {
        kind: Kind(kind),
        count,
        values,
        removals,
    })
}

fn entry_count(input: &mut &[u8]) -> Result<usize, WireError> {
    let count = usize::try_from(read_var_int(input, WireError::of_frame)?)
        .map_err(|_| WireError::Malformed)?;
    if count > MAX_ENTRIES {
        return Err(WireError::TooManyEntries);
    }
    Ok(count)
}

fn component_type(input: &mut &[u8]) -> Result<ComponentType, WireError> {
    let raw = read_var_int(input, WireError::of_frame)?;
    u16::try_from(raw)
        .ok()
        .filter(|id| DataComponentType::from_protocol_id(*id).is_some())
        .map(ComponentType)
        .ok_or(WireError::UnknownComponentType(raw))
}

fn check_entries<T>(
    entries: &[T],
    component: fn(&T) -> ComponentType,
) -> Result<VarInt, WireError> {
    if entries.len() > MAX_ENTRIES {
        return Err(WireError::TooManyEntries);
    }
    let length = i32::try_from(entries.len()).map_err(|_| WireError::TooManyEntries)?;
    let mut rest = entries;
    while let Some((first, others)) = rest.split_first() {
        let first = component(first);
        if others.iter().any(|other| component(other) == first) {
            return Err(WireError::RepeatedComponentType(first));
        }
        rest = others;
    }
    Ok(VarInt(length))
}
