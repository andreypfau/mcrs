use mcrs_minecraft_nbt::MAX_DEPTH;
use mcrs_minecraft_nbt::NbtBudget;

use crate::WireError;
use crate::data_component_type_keys::DataComponentType;
use crate::nbt_depth::STACK_LEVEL;
use crate::skip_value::skip_value;
use crate::wire::Entries;
use crate::wire_value::read_var_int;

#[derive(Clone, Copy)]
pub(crate) struct Head {
    pub(crate) count: i32,
    pub(crate) item: i32,
}

pub(crate) fn scan_stack(input: &mut &[u8], entries: Entries) -> Result<Head, WireError> {
    mcrs_minecraft_nbt::with_nbt_budget(NbtBudget::default(), || {
        scan_budgeted_stack(input, entries)
    })
    .unwrap_or(Err(WireError::QuotaExceeded))
}

fn scan_budgeted_stack(input: &mut &[u8], entries: Entries) -> Result<Head, WireError> {
    let head = read_head(input)?;
    if head.count > 0 {
        scan_patch(input, entries, STACK_LEVEL)?;
    }
    Ok(head)
}

pub(crate) fn read_head(input: &mut &[u8]) -> Result<Head, WireError> {
    let count = read_var_int(input, WireError::of_frame)?;
    if count <= 0 {
        return Ok(Head { count, item: 0 });
    }
    let item = read_var_int(input, WireError::of_frame)?;
    Ok(Head { count, item })
}

pub(crate) fn scan_template(input: &mut &[u8], depth: u32, levels: u32) -> Result<(), WireError> {
    let depth = depth.saturating_add(levels);
    if depth > MAX_DEPTH {
        return Err(WireError::TooDeep);
    }
    read_var_int(input, WireError::of_frame)?;
    read_var_int(input, WireError::of_frame)?;
    scan_patch(input, Entries::Bare, depth)
}

fn scan_patch(input: &mut &[u8], entries: Entries, depth: u32) -> Result<(), WireError> {
    let values = length(input)?;
    let removals = length(input)?;
    for _ in 0..values {
        let component = read_type(input)?;
        match entries {
            Entries::Bare => skip_value(component, input, depth)?,
            Entries::LengthPrefixed => {
                let length = length(input)?;
                let rest = input.get(length..).ok_or(WireError::UnexpectedEnd)?;
                *input = rest;
            }
        }
    }
    for _ in 0..removals {
        read_type(input)?;
    }
    Ok(())
}

pub(crate) fn skip_component_map(input: &mut &[u8], depth: u32) -> Result<(), WireError> {
    let values = length(input)?;
    for _ in 0..values {
        let component = read_type(input)?;
        skip_value(component, input, depth)?;
    }
    Ok(())
}

fn length(input: &mut &[u8]) -> Result<usize, WireError> {
    usize::try_from(read_var_int(input, WireError::of_frame)?).map_err(|_| WireError::Malformed)
}

fn read_type(input: &mut &[u8]) -> Result<DataComponentType, WireError> {
    let raw = read_var_int(input, WireError::of_frame)?;
    u16::try_from(raw)
        .ok()
        .and_then(DataComponentType::from_protocol_id)
        .ok_or(WireError::UnknownComponentType(raw))
}
