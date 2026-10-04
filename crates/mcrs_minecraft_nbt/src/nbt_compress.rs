use crate::deserializer::NbtReadHelper;
use crate::{Error, Nbt, NbtCompound, deserializer, serializer};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use std::io::{Cursor, Read, Seek, Write};

pub fn read_gzip_compound_tag(input: impl Read + Seek) -> Result<NbtCompound, Error> {
    let mut decoder = GzDecoder::new(input);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(Error::Incomplete)?;
    let mut reader = NbtReadHelper::new(Cursor::new(buf));
    let nbt = Nbt::read(&mut reader)?;
    Ok(nbt.root_tag)
}

pub fn write_gzip_compound_tag_to_bytes(compound: &NbtCompound) -> Result<Vec<u8>, Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&Nbt::new(String::new(), compound.clone()).write())
        .map_err(Error::Incomplete)?;
    encoder.finish().map_err(Error::Incomplete)
}

pub fn from_gzip_bytes<'a, T: serde::Deserialize<'a>, R: Read>(input: R) -> Result<T, Error> {
    let mut decoder = GzDecoder::new(input);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(Error::Incomplete)?;
    deserializer::from_bytes(Cursor::new(buf))
}

pub fn to_gzip_bytes_vec<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    serializer::to_bytes(value, GzEncoder::new(&mut buffer, Compression::default()))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use crate::{
        NbtCompound,
        nbt_compress::{
            from_gzip_bytes, read_gzip_compound_tag, to_gzip_bytes_vec,
            write_gzip_compound_tag_to_bytes,
        },
        tag::NbtTag,
    };
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        string_field: String,
        int_field: i32,
        bool_field: bool,
        float_field: f32,
        string_list: Vec<String>,
        nested: NestedStruct,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct NestedStruct {
        value: i64,
        name: String,
    }

    #[test]
    fn test_gzip_round_trips_a_compound_and_a_struct() {
        // Create a test compound
        let mut compound = NbtCompound::new();
        compound.put_byte("byte_value", 123);
        compound.put_short("short_value", 12345);
        compound.put_int("int_value", 1234567);
        compound.put_long("long_value", 123456789);
        compound.put_float("float_value", 123.456);
        compound.put_double("double_value", 123456.789);
        compound.put_bool("bool_value", true);
        compound.put("string_value", NbtTag::String("test string".to_string()));

        // Create a nested compound
        let mut nested = NbtCompound::new();
        nested.put_int("nested_int", 42);
        compound.put_component("nested_compound", nested);

        // Write to GZip using streaming
        let buffer =
            write_gzip_compound_tag_to_bytes(&compound).expect("Failed to compress compound");

        // Read from GZip using streaming
        let read_compound =
            read_gzip_compound_tag(Cursor::new(&buffer)).expect("Failed to decompress compound");

        // Verify values
        assert_eq!(read_compound.get_byte("byte_value"), Some(123));
        assert_eq!(read_compound.get_short("short_value"), Some(12345));
        assert_eq!(read_compound.get_int("int_value"), Some(1234567));
        assert_eq!(read_compound.get_long("long_value"), Some(123456789));
        assert_eq!(
            read_compound
                .get("float_value")
                .and_then(NbtTag::extract_float),
            Some(123.456)
        );
        assert_eq!(
            read_compound
                .get("double_value")
                .and_then(NbtTag::extract_double),
            Some(123456.789)
        );
        assert_eq!(read_compound.get_bool("bool_value"), Some(true));
        assert_eq!(
            read_compound.get_string("string_value"),
            Some("test string")
        );

        // Verify nested compound
        if let Some(nested) = read_compound.get_compound("nested_compound") {
            assert_eq!(nested.get_int("nested_int"), Some(42));
        } else {
            panic!("Failed to retrieve nested compound");
        }

        let test_struct = TestStruct {
            string_field: "test string".to_string(),
            int_field: 12345,
            bool_field: true,
            float_field: 123.456,
            string_list: vec!["one".to_string(), "two".to_string(), "three".to_string()],
            nested: NestedStruct {
                value: 9876543210,
                name: "nested_test".to_string(),
            },
        };

        // Test streaming serialization
        let buffer =
            to_gzip_bytes_vec(&test_struct).expect("Failed to serialize and compress struct");

        // Test streaming deserialization
        let read_struct: TestStruct = from_gzip_bytes(Cursor::new(&buffer))
            .expect("Failed to decompress and deserialize struct");

        assert_eq!(read_struct, test_struct);
    }

    #[test]
    fn test_gzip_invalid_data() {
        // Try to read from invalid data
        let invalid_data = vec![1, 2, 3, 4, 5]; // Not valid GZip data
        let result = read_gzip_compound_tag(Cursor::new(invalid_data));
        assert!(result.is_err());
    }
}
