# Low-value tests and why they fail

## Circular oracle

```rust
let expected = parse_packet(&input).unwrap();
assert_eq!(parse_packet(&input).unwrap(), expected);
```

Both sides use the same implementation; a consistently wrong parser passes.

## Weak assertion for a stronger contract

```rust
let items = list_visible_items(&db).await.unwrap();
assert!(!items.is_empty());
```

If the contract requires filtering by owner, a non-empty list does not detect cross-user leakage. `is_empty()` or `is_ok()` can still be correct when emptiness or success is the entire contract.

## Incidental implementation coupling

```rust
assert_eq!(cache.get_call_count(), 2);
```

If the contract only requires the correct returned value, this test may break under harmless refactoring. Call counts are appropriate if they enforce a real constraint (e.g., no duplicate charges).

## Test gaming

A browser test injects JavaScript that fixes the page before asserting it works. The test passes, but the deployed application remains broken. Tests must not repair the system under test to create a pass.

## Duplicate scenarios

Ten tests all use different strings but exercise the same valid-input path without testing any new equivalence class, invariant, or boundary. Additional tests add runtime without meaningful detection value.
