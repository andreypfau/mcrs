# High-value tests with Rust examples

## Specification-derived test vector

Assume a protocol specifies that an unsigned 16-bit integer is encoded big-endian:

```rust
#[test]
fn decodes_big_endian_u16() {
    assert_eq!(decode_u16(&[0x01, 0x00]), Ok(256));
}
```

The expected value comes from the protocol, not the implementation. Add a truncated-input test if rejection is part of the contract.

## Regression test for a real defect

Suppose a parser previously accepted trailing bytes despite a specification requiring exact consumption:

```rust
#[test]
fn rejects_trailing_bytes() {
    assert!(decode_frame(&[0x01, 0xFF]).is_err());
}
```

This is illustrative: the bytes and result must be adapted to the actual frame format. Prefer confirming that the test fails on the buggy revision and passes on the fix.

## Property-based testing

For an encoder and decoder whose contract requires lossless round trips:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn roundtrip_u16(value in any::<u16>()) {
        let bytes = encode_u16(value);
        prop_assert_eq!(decode_u16(&bytes), Ok(value));
    }
}
```

A shared defect in encode/decode can still pass. Pair with an independently specified vector where risk warrants it.

## Metamorphic invariant

If sorting must preserve the multiset of elements, test that output is ordered **and** is a permutation of the input. Checking only ordering could accept an implementation that drops elements.

## Risk-based test choice

- Rename a private helper with no behavioral change: run existing checks; do not add a test.
- Fix a pagination off-by-one error: add a boundary regression case for the affected page size.
- Implement a binary protocol parser: add specification vectors, malformed-input rejection, and property/fuzz testing when appropriate.
- Change a payment retry mechanism: verify idempotency across retries using an integration test at the payment boundary.
