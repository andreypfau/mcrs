use std::collections::HashMap;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{
    BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, INT_ARRAY_ID, LIST_ID, NbtBudget, NbtQuotaExceeded,
    decode_wire, split_wire, with_nbt_budget,
};
use mcrs_minecraft_wire::Decode;
use serde::de::IgnoredAny;

use crate::declared_lengths::largest_request;

const A_MEGABYTE: usize = 1 << 20;

const VECTOR_BUDGET: NbtBudget = NbtBudget {
    bytes: 2_097_152,
    tags: 262_144,
};

fn sized(kind: &[u8], declared: i32, data: usize) -> Vec<u8> {
    let mut bytes = kind.to_vec();
    bytes.extend_from_slice(&declared.to_be_bytes());
    bytes.resize(bytes.len() + data, 0);
    bytes
}

fn as_tag(budget: NbtBudget, bytes: &[u8]) -> Result<bool, NbtQuotaExceeded> {
    with_nbt_budget(budget, || NbtTag::decode(&mut &*bytes).is_ok())
}

fn split(budget: NbtBudget, bytes: &[u8]) -> Result<bool, NbtQuotaExceeded> {
    with_nbt_budget(budget, || split_wire(&mut &*bytes).is_ok())
}

fn budget(bytes: u64, tags: u64) -> NbtBudget {
    NbtBudget { bytes, tags }
}

#[test]
fn the_reader_vectors_pass_up_to_the_budget_and_stop_one_past_it() {
    let n = 2_097_152;
    let cases = [
        (sized(&[BYTE_ARRAY_ID], n, 2_097_152), true),
        (sized(&[BYTE_ARRAY_ID], n + 1, 2_097_153), false),
        (sized(&[INT_ARRAY_ID], i32::MAX, 0), false),
        (sized(&[LIST_ID, COMPOUND_ID], 600_000, 600_000), false),
        (sized(&[LIST_ID, BYTE_ID], 262_143, 262_143), true),
        (sized(&[LIST_ID, BYTE_ID], 262_144, 262_144), false),
    ];
    for (index, (bytes, passes)) in cases.iter().enumerate() {
        let expected = if *passes {
            Ok(true)
        } else {
            Err(NbtQuotaExceeded)
        };
        assert_eq!(as_tag(VECTOR_BUDGET, bytes), expected, "{index}");
        assert_eq!(split(VECTOR_BUDGET, bytes), expected, "{index}");
    }
}

#[test]
fn a_truncated_array_past_the_budget_is_a_quota_error_and_not_an_allocation() {
    let bytes = sized(&[INT_ARRAY_ID], i32::MAX, 0);
    let (read, largest) = largest_request(|| as_tag(VECTOR_BUDGET, &bytes));
    assert_eq!(read, Err(NbtQuotaExceeded));
    assert!(largest < A_MEGABYTE, "{largest}");
}

#[test]
fn a_compound_is_charged_its_kind_bytes_name_and_data_on_every_read_path() {
    let bytes = [
        COMPOUND_ID,
        BYTE_ARRAY_ID,
        0,
        1,
        b'a',
        0,
        0,
        0,
        3,
        1,
        2,
        3,
        0,
    ];
    let serde = |budget| {
        with_nbt_budget(budget, || {
            decode_wire::<HashMap<String, IgnoredAny>>(&mut &bytes[..]).is_ok()
        })
    };
    assert_eq!(as_tag(budget(7, 2), &bytes), Ok(true));
    assert_eq!(split(budget(7, 2), &bytes), Ok(true));
    assert_eq!(serde(budget(7, 2)), Ok(true));
    for short in [budget(6, 2), budget(7, 1)] {
        assert_eq!(as_tag(short, &bytes), Err(NbtQuotaExceeded));
        assert_eq!(split(short, &bytes), Err(NbtQuotaExceeded));
        assert_eq!(serde(short), Err(NbtQuotaExceeded));
    }
}

#[test]
fn a_list_of_wrapped_values_is_charged_once_on_the_serde_path() {
    let bytes = [
        LIST_ID,
        COMPOUND_ID,
        0,
        0,
        0,
        2,
        BYTE_ID,
        0,
        0,
        1,
        0,
        BYTE_ID,
        0,
        0,
        2,
        0,
    ];
    let serde =
        |budget| with_nbt_budget(budget, || decode_wire::<Vec<i8>>(&mut &bytes[..]).is_ok());
    assert_eq!(as_tag(budget(19, 5), &bytes), Ok(true));
    assert_eq!(serde(budget(19, 5)), Ok(true));
    for short in [budget(18, 5), budget(19, 4)] {
        assert_eq!(as_tag(short, &bytes), Err(NbtQuotaExceeded));
        assert_eq!(serde(short), Err(NbtQuotaExceeded));
    }
}

#[test]
fn a_nested_scope_shares_the_budget_of_the_outer_one() {
    let bytes = sized(&[BYTE_ARRAY_ID], 4, 4);
    let read = with_nbt_budget(budget(6, 2), || {
        let first = with_nbt_budget(NbtBudget::default(), || {
            NbtTag::decode(&mut bytes.as_slice()).is_ok()
        });
        let second = NbtTag::decode(&mut bytes.as_slice()).is_ok();
        (first, second)
    });
    assert_eq!(read, Err(NbtQuotaExceeded));
}
