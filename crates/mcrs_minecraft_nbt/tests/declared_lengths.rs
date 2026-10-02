use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::{self, Cursor, Read, Seek, SeekFrom};

use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{
    ArrayKind, ArrayVisitor, BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, DOUBLE_ID, END_ID, Error,
    FLOAT_ID, INT_ARRAY_ID, INT_ID, LIST_ID, LONG_ARRAY_ID, LONG_ID, SHORT_ID, from_bytes,
    nbt_array,
};
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny, SeqAccess, Visitor};

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

/// Counts the calls that reach the input: the work a read does, measured
/// without a clock.
struct Counted<'a> {
    input: Cursor<&'a [u8]>,
    calls: &'a Cell<usize>,
}

impl Read for Counted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        self.input.read(buf)
    }
}

impl Seek for Counted<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.calls.set(self.calls.get() + 1);
        self.input.seek(pos)
    }
}

fn counted<T: DeserializeOwned>(bytes: &[u8]) -> (Result<T, Error>, usize) {
    let calls = Cell::new(0);
    let read = from_bytes(Counted {
        input: Cursor::new(bytes),
        calls: &calls,
    });
    (read, calls.get())
}

const A_HANDFUL_OF_CALLS: usize = 64;

fn list_of(element: u8, declared: i32, present: &[u8]) -> Vec<u8> {
    let mut bytes = vec![element];
    bytes.extend_from_slice(&declared.to_be_bytes());
    bytes.extend_from_slice(present);
    bytes
}

/// A named root compound holding `fields` in order, closed.
fn root(fields: &[(u8, &str, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![COMPOUND_ID, 0, 0];
    for (tag, name, payload) in fields {
        bytes.push(*tag);
        bytes.extend_from_slice(&(name.len() as u16).to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(payload);
    }
    bytes.push(END_ID);
    bytes
}

/// Reads `kept` and passes over every other field.
#[derive(Deserialize, Debug)]
struct Kept {
    kept: i32,
}

#[derive(Deserialize, Debug)]
struct Elements {
    #[serde(rename = "data")]
    _data: Vec<IgnoredAny>,
}

fn refuses_an_end_list<T>(read: &Result<T, Error>) -> bool {
    matches!(read, Err(Error::SerdeError(reason)) if reason.contains("TAG_End"))
}

#[test]
fn a_list_of_end_tags_is_refused_on_every_path_that_meets_one() {
    let end_list = list_of(END_ID, i32::MAX, &[]);
    let seven = 7i32.to_be_bytes();

    let skipped = root(&[(LIST_ID, "skipped", &end_list), (INT_ID, "kept", &seven)]);
    assert!(skipped.len() < 40);
    let (read, _) = counted::<Kept>(&skipped);
    assert!(refuses_an_end_list(&read), "a skipped field: {read:?}");

    let eight = list_of(LIST_ID, 8, &end_list.repeat(8));
    let nested = root(&[(LIST_ID, "skipped", &eight), (INT_ID, "kept", &seven)]);
    assert!(nested.len() < 80);
    let (read, _) = counted::<Kept>(&nested);
    assert!(
        refuses_an_end_list(&read),
        "a skipped list of lists: {read:?}"
    );

    let (read, _) = counted::<Elements>(&root(&[(LIST_ID, "data", &end_list)]));
    assert!(refuses_an_end_list(&read), "element by element: {read:?}");

    let mut wrapper = vec![LIST_ID, 0, 0];
    wrapper.extend_from_slice(&end_list);
    wrapper.push(END_ID);
    let wrapped = list_of(COMPOUND_ID, 1, &wrapper);
    let (read, _) = counted::<Elements>(&root(&[(LIST_ID, "data", &wrapped)]));
    assert!(refuses_an_end_list(&read), "a wrapped element: {read:?}");
}

#[test]
fn an_empty_list_of_end_tags_is_still_an_empty_list() {
    #[derive(Deserialize)]
    struct Numbers {
        data: Vec<i32>,
    }

    let empty = list_of(END_ID, 0, &[]);
    let seven = 7i32.to_be_bytes();

    let skipped = root(&[(LIST_ID, "skipped", &empty), (INT_ID, "kept", &seven)]);
    assert_eq!(counted::<Kept>(&skipped).0.unwrap().kept, 7);

    let read = counted::<Numbers>(&root(&[(LIST_ID, "data", &empty)])).0;
    assert!(read.unwrap().data.is_empty());
}

const FIXED_SIZE: [(u8, usize); 6] = [
    (BYTE_ID, 1),
    (SHORT_ID, 2),
    (INT_ID, 4),
    (LONG_ID, 8),
    (FLOAT_ID, 4),
    (DOUBLE_ID, 8),
];

#[test]
fn a_skipped_list_of_fixed_size_elements_is_passed_in_one_step() {
    let seven = 7i32.to_be_bytes();
    for (element, size) in FIXED_SIZE {
        let list = list_of(element, 100_000, &vec![0x5a; 100_000 * size]);
        let bytes = root(&[(LIST_ID, "skipped", &list), (INT_ID, "kept", &seven)]);

        let (read, calls) = counted::<Kept>(&bytes);
        assert_eq!(read.unwrap().kept, 7, "element {element}");
        assert!(
            calls < A_HANDFUL_OF_CALLS,
            "element {element} went to the input {calls} times"
        );
    }
}

#[test]
fn a_list_declaring_more_than_the_input_holds_stops_at_the_first_missing_element() {
    let elements = FIXED_SIZE.map(|(element, _)| element);
    for element in elements.into_iter().chain([COMPOUND_ID]) {
        let bytes = chunk_like(LIST_ID, &list_of(element, i32::MAX, &[]));
        assert!(bytes.len() < 20);

        let (read, calls) = counted::<Kept>(&bytes);
        assert!(
            matches!(read, Err(Error::Incomplete(_))),
            "skipped, element {element}: {read:?}"
        );
        assert!(
            calls < A_HANDFUL_OF_CALLS,
            "skipped, element {element} went to the input {calls} times"
        );

        let (read, calls) = counted::<Elements>(&bytes);
        assert!(
            matches!(read, Err(Error::Incomplete(_))),
            "element by element, element {element}: {read:?}"
        );
        assert!(
            calls < A_HANDFUL_OF_CALLS,
            "element by element, element {element} went to the input {calls} times"
        );
    }
}

#[test]
fn a_compound_the_input_ends_inside_is_an_error_and_not_an_empty_compound() {
    let read = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&[COMPOUND_ID])));
    assert!(matches!(read, Err(Error::Incomplete(_))), "{read:?}");

    let mut bytes = vec![LIST_ID];
    bytes.extend(list_of(COMPOUND_ID, i32::MAX, &[]));
    let (read, largest) =
        largest_request(|| NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(&bytes))));
    assert!(matches!(read, Err(Error::Incomplete(_))));
    assert!(largest < A_MEGABYTE, "asked for {largest} bytes at once");
}

struct PayloadLength(usize);

impl<'de> Deserialize<'de> for PayloadLength {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Measure;

        impl<'de> ArrayVisitor<'de> for Measure {
            type Value = PayloadLength;

            fn visit_array<E>(self, _: ArrayKind, payload: &[u8]) -> Result<PayloadLength, E> {
                Ok(PayloadLength(payload.len()))
            }

            fn visit_other<D: serde::Deserializer<'de>>(
                self,
                _: D,
            ) -> Result<PayloadLength, D::Error> {
                Err(serde::de::Error::custom("not an array"))
            }
        }

        nbt_array(deserializer, Measure)
    }
}

#[test]
fn an_array_read_with_its_kind_goes_to_the_input_once_a_step_and_not_once_an_element() {
    #[derive(Deserialize)]
    struct Measured {
        data: PayloadLength,
    }

    for (tag, width) in [(BYTE_ARRAY_ID, 1), (INT_ARRAY_ID, 4), (LONG_ARRAY_ID, 8)] {
        let payload = vec![0x5a; 100_000 * width];
        let field = truncated(tag, 100_000, &payload);

        let (read, calls) = counted::<Measured>(&root(&[(tag, "data", &field)]));
        assert_eq!(read.unwrap().data.0, payload.len(), "tag {tag}");
        assert!(
            calls < A_HANDFUL_OF_CALLS,
            "tag {tag} went to the input {calls} times"
        );
    }
}
