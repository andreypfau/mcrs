use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Cursor;

use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{
    BYTE_ARRAY_ID, COMPOUND_ID, END_ID, Error, INT_ARRAY_ID, LIST_ID, LONG_ARRAY_ID, LONG_ID,
    from_bytes,
};
use serde::Deserialize;
use serde::de::{SeqAccess, Visitor};

thread_local! {
    static LARGEST_REQUEST: Cell<usize> = const { Cell::new(0) };
}

struct RecordingAllocator;

fn record(size: usize) {
    let _ = LARGEST_REQUEST.try_with(|largest| largest.set(largest.get().max(size)));
}

unsafe impl GlobalAlloc for RecordingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: RecordingAllocator = RecordingAllocator;

/// The largest single request the calling thread made while `read` ran.
fn largest_request<T>(read: impl FnOnce() -> T) -> (T, usize) {
    LARGEST_REQUEST.with(|largest| largest.set(0));
    let value = read();
    (value, LARGEST_REQUEST.with(Cell::get))
}

const A_MEGABYTE: usize = 1 << 20;

/// The payload an array or a list declares, cut off after `present` elements.
fn truncated(tag: u8, declared: i32, present: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    if tag == LIST_ID {
        bytes.push(LONG_ID);
    }
    bytes.extend_from_slice(&declared.to_be_bytes());
    bytes.extend_from_slice(present);
    bytes
}

/// A named root compound whose only field `data` carries `payload`, with the
/// closing tag left off: the input ends inside the field.
fn chunk_like(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![COMPOUND_ID, 0, 0, tag, 0, 4];
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(payload);
    bytes
}

struct Whole(Vec<u8>);

impl<'de> Deserialize<'de> for Whole {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Bytes;

        impl<'de> Visitor<'de> for Bytes {
            type Value = Whole;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an array")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Whole, E> {
                Ok(Whole(v.to_vec()))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Whole, A::Error> {
                let mut words = Vec::<i64>::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(word) = seq.next_element()? {
                    words.push(word);
                }
                Ok(Whole(
                    words.into_iter().flat_map(i64::to_be_bytes).collect(),
                ))
            }
        }

        deserializer.deserialize_bytes(Bytes)
    }
}

#[derive(Deserialize)]
struct Holder {
    data: Whole,
}

#[test]
fn an_array_declaring_more_than_the_input_holds_is_an_error_and_not_an_allocation() {
    for (tag, width) in [(BYTE_ARRAY_ID, 1), (INT_ARRAY_ID, 4), (LONG_ARRAY_ID, 8)] {
        for declared in [100_000_000, i32::MAX] {
            let bytes = chunk_like(tag, &truncated(tag, declared, &[7; 16]));
            assert!(bytes.len() < 200);

            let (read, largest) = largest_request(|| from_bytes::<Holder>(Cursor::new(&bytes)));
            assert!(
                matches!(read, Err(Error::Incomplete(_))),
                "{declared} elements of {width} bytes"
            );
            assert!(
                largest < A_MEGABYTE,
                "{declared} elements of {width} bytes asked for {largest} bytes at once"
            );
        }
    }
}

#[test]
fn a_list_declaring_more_than_the_input_holds_is_an_error_and_not_an_allocation() {
    let bytes = chunk_like(LIST_ID, &truncated(LIST_ID, i32::MAX, &[7; 16]));

    let (read, largest) = largest_request(|| from_bytes::<Holder>(Cursor::new(&bytes)));
    assert!(matches!(read, Err(Error::Incomplete(_))));
    assert!(largest < A_MEGABYTE, "asked for {largest} bytes at once");
}

#[test]
fn a_tag_declaring_more_than_the_input_holds_is_an_error_and_not_an_allocation() {
    for tag in [BYTE_ARRAY_ID, INT_ARRAY_ID, LONG_ARRAY_ID, LIST_ID] {
        let mut bytes = vec![tag];
        bytes.extend(truncated(tag, i32::MAX, &[7; 16]));

        let (read, largest) =
            largest_request(|| NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&bytes))));
        assert!(matches!(read, Err(Error::Incomplete(_))), "tag {tag}");
        assert!(
            largest < A_MEGABYTE,
            "tag {tag} asked for {largest} bytes at once"
        );
    }
}

#[test]
fn a_list_of_end_tags_is_refused_before_it_can_grow_from_nothing() {
    let mut bytes = vec![LIST_ID, END_ID];
    bytes.extend_from_slice(&i32::MAX.to_be_bytes());

    let (read, largest) =
        largest_request(|| NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&bytes))));
    assert!(read.is_err());
    assert!(largest < A_MEGABYTE, "asked for {largest} bytes at once");

    let empty = [LIST_ID, END_ID, 0, 0, 0, 0];
    let read = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&empty)));
    assert_eq!(read.unwrap(), NbtTag::List(Vec::new()));
}

#[test]
fn an_array_longer_than_one_read_step_comes_back_whole() {
    let words: Vec<i64> = (0..100_000).map(|i| i * 0x0101_0101 - 7).collect();
    let payload: Vec<u8> = words.iter().copied().flat_map(i64::to_be_bytes).collect();

    let mut field = truncated(LONG_ARRAY_ID, words.len() as i32, &payload);
    field.push(END_ID);
    let holder: Holder = from_bytes(Cursor::new(chunk_like(LONG_ARRAY_ID, &field))).unwrap();
    assert_eq!(holder.data.0, payload);

    let mut bytes = vec![LONG_ARRAY_ID];
    bytes.extend(truncated(LONG_ARRAY_ID, words.len() as i32, &payload));
    let read = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&bytes)));
    assert_eq!(read.unwrap(), NbtTag::LongArray(words));

    let mut bytes = vec![BYTE_ARRAY_ID];
    bytes.extend(truncated(BYTE_ARRAY_ID, payload.len() as i32, &payload));
    let read = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&bytes)));
    assert_eq!(read.unwrap(), NbtTag::ByteArray(payload.into()));
}
