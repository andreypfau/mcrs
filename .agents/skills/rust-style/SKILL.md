---
name: rust-style
description: The Rust style of mcrs - crate size and layout, modules, imports, types and signatures, naming, implementation rules and the review checklist. Load before writing new Rust, splitting a crate or module, naming a public item, or reviewing a Rust diff. Triggers - rust style, style review, crate split, small crates, module layout, naming, bool parameter, tuple return, non_exhaustive, must_use, glob import, extension trait.
---

# Rust style for mcrs

The goals are readability over writeability, code that tools and large
mechanical changes can handle, and alignment with the Rust ecosystem: std
naming, the Rust API Guidelines and rustfmt. Every rule here must pay for
itself. CLAUDE.md wins on conflict.

## Errors and panics

- Library code never panics. No `unwrap`, `expect`, `panic!`,
  `unreachable!`, `todo!` or out-of-bounds indexing outside tests.
- Return `Option` for a simple failure with an obvious cause. Return
  `Result` when the caller needs context. Shape the data so that an internal
  invariant needs no panicking operation.
- Counters use `checked_*` and return an error, or `wrapping_*` for ids that
  wrap by design. Never use a bare `+=` that can overflow.
- Tests and developer-only binaries may panic.

## Ownership

- One owner; the owner lends the data. No `Rc`, no `Arc`, no locks, no
  interior mutability in new code.
- Prefer a borrow to a deep copy. Call `clone()` only when you truly need a
  second copy.
- No `dyn` and no `Any`. Use a generic parameter or an enum that the
  application composes. A callback is a `fn` pointer or a `SystemId`.

## Crates and modules

- **Prefer small crates.** A crate is one abstraction, ideally a handful of
  files. Split a crate only when a dependency forces the split: a part that
  needs a heavy dependency makes every user of the other parts pay for it.
  Do not split by topic alone.
- **Flat layout.** All `.rs` files of a crate go in `src/`, with no
  subdirectories. If you need `foo/mod.rs`, the crate is too big: make
  `foo.rs`, `foo_bar.rs` and so on, or split the crate. Older crates with
  directories move when you touch them.
- **Non-leaf modules** (`lib.rs`, any module with submodules) hold only `mod`
  lines, re-exports and at most a few lines of glue. No types, no functions.
- **Name the file after its primary item.** `EditBox` goes in `edit_box.rs`,
  `Graph` in `graph.rs`. A file that holds three primary items is three files.
- **One public path per item.** Either the modules are private and every item is
  re-exported at the root, or the modules are public and nothing is
  re-exported. Prefer the first option.
- **Layering beats convenience.** A lower crate never depends on a higher one
  just for a helper. Move the helper down instead.
- **About 500 lines** for a file and **about 80 lines** for a function are the
  points to look for a split by meaning. Do not split a flat `match` only to
  meet a number.
- A new crate gets `[lints] workspace = true` and `[lib] doctest = false`, an
  entry in `[workspace.dependencies]`, and `#![no_std]` + `alloc`, or a
  `// std: <dependency>` note. A crate without Bevy goes in
  `NO_BEVY_PACKAGES` in `xtask/src/checks.rs`.

## Formatting

- Run `cargo fmt`. Use `#[rustfmt::skip]` only for manual alignment or
  generated code, never for personal taste.

## Imports

- One module per `use`. Never write `use a::{b::C, d::E}`. Merge the `use`
  lines that name the same module.
- Import types and use them unqualified, as `Limits` and not
  `mcrs_command::Limits`. Call free functions through their module:
  `mem::swap`, `fmt::Display`, `suggest::recycle`, `dim::deliver`. Exceptions:
  a name that collides (`io::Error`, or `Dimension` as `DimensionKey`), and
  very common names.
- Scope a trait import locally unless the whole file uses it.
- No glob imports, with these exceptions:
  - `use super::*` in an inline test module.
  - `use Enum::*` inside a function that matches on that enum.
  - `pub use sub::*` in a non-leaf module.
  - The Bevy preludes (`bevy_ecs::prelude::*`, `bevy::prelude::*`).

  Do not define a prelude of our own.

## Types and signatures

- **Use an enum instead of a bool parameter** when `f(x, true)` does not say
  what `true` means. Examples: `ForkMode`, `Polarity { If, Unless }`,
  `Feedback`, `Step { Char, Word }`. Two named functions are also fine.
- **Use named fields instead of tuples.** A return of 2 or more values that
  are not a pair by nature (`(min, max)` of a range can stay) becomes a
  struct. So does a tuple inside a table row, inside a `Vec`, or inside an
  `Option` on a packet. A single-field newtype and a very local tuple are
  fine.
- **Use enums instead of magic values:** no `Option<Result<T, ()>>`, no
  `Option<Option<T>>`, no `char` or `u8` codes, and no `bool` field that
  picks a variant. Give the states names.
- **Use a newtype instead of a type alias** when the meaning differs, the type
  needs its own API, the underlying type may change, or the two are easy to
  mix up.
- **Accept `&str`, `&[T]`, `&T` and `Option<&T>`.** Never accept `&String`,
  `&Vec<T>`, `&Box<T>` or `&Option<T>`.
- **Prefer iterators.** Library input that is only iterated is
  `impl IntoIterator`. Return an iterator rather than a `Vec`, unless the
  caller needs ownership or random access, or the `Vec` already exists.
- `impl Into<T>` parameters are only for library APIs where the call site
  really matters.
- **Implement the std trait** instead of a custom method: `FromStr` and not
  `from_name`, `From<u8>` and not `from_id`, `PartialEq<str>` and not
  `eq_str`, `AsRef<[T]>` and not `Deref`. `Deref` is for smart pointers only.
  `len()` implies `is_empty()`. A type with `new()` implements `Default`, and
  so does an options struct. An important `From` impl is paired with a named
  constructor.
- **Derive traits** where a derive works. A library type gets `Clone`,
  `Debug` and `PartialEq` up front. It gets `Copy` if it will stay `Copy`. A
  map key also gets `Eq` and `Hash`, plus `Ord` when the order is clear.
- **Define a trait only for generics or for extension.** Never define one just
  to group methods.
- **Public errors** implement `Display` and `core::error::Error` (short plain
  English; translated `Text` is rendered elsewhere). They are `Copy` when
  possible and have a `///` line on each variant.
- **`#[non_exhaustive]`** goes on a public enum that will grow, and on a
  public struct with public fields that will grow. Add a constructor when
  callers outside the crate build the struct. A fixed ontology (`Key`,
  `ErrorKind`) stays exhaustive. A match on a non-exhaustive enum from another
  crate lists the known variants and ends with a wildcard.
- **`#[must_use]`** goes on pure functions and on lazy types. It does not go
  on functions that return `Result`, which is already `#[must_use]`.
- **Generics over trait objects.** A layer-4 engine is generic over the set
  that the application supplies: `CommandPlugin<S: CommandSet>`, with no
  default that names vanilla.
- Mark a function `const fn` only if it will stay `const`. Overload an
  operator only for its mathematical or logical meaning. Run no code before
  `main`.

## Naming

- Use the std conventions: `as_`/`to_`/`into_`, getters without `get_`,
  `foo_mut`, `set_foo`, `take_foo`, `replace_foo`. Constructors are `new`,
  `new_with_foo`, `try_new` and `from_foo`.
- An extension trait ends in `Ext`, for example `AppCommandsExt`.
- Do not abbreviate and do not drop letters. No single-letter names except
  trivial closures and loop indices. `registrar`, not `r`; `selector`, not
  `s`; `cos_yaw`, not `cy`.
- A generic parameter is short and hints at its bound, like `S: CommandSet`
  or `L: Lookup`. `T` and `U` are for unconstrained types.
- Do not shadow std or prelude names. A type named `Send`, `Box` or `Result`
  is wrong.
- A type does not reuse the name of a type that its users import from another
  crate. That forces the user to qualify one of them.
- A lifetime name says what it borrows when more than one lifetime is in
  play.

## Documentation and comments

- Zero comments by default. A comment states a non-obvious why, in one or
  two lines.
- A public item gets a short `///` that states its contract, not a
  restatement of its name. A crate gets a `//!` line. Write docs in Markdown.
- Document every case that can panic in a test helper. Document the
  contract that an embedder must meet, such as a resource it must insert.
- An `unsafe` block gets a `// SAFETY:` line, even in tests. Use `unsafe`
  only when safe code cannot do the job, and keep it in its own small
  module.

## Implementation

- Do not overuse type inference. Annotate the variable rather than using a
  turbofish on `into()`, `parse()` or `collect()`.
- Prefer `if let` / `let`-`else` over check-then-unwrap and over long
  `and_then`/`flatten` chains.
- Keep iterator chains short. Do not build a chain in a `for` header. Do not
  use `for_each`.
- Every `assert!` and `debug_assert!` gets a message that says why the
  condition holds.
- Prefer plain code to `macro_rules!`, and generics to a macro that stamps
  out types. A macro is acceptable only for long repeated `From` or trait
  impls. A macro never takes a function body as `tt`.
- Use `format!`/`write!` instead of `+` on more than two strings, and instead
  of `str::replace` for substitution.
- No `catch_unwind`. No unstable features.

## Review checklist

Run the checklist on `git diff origin/main...HEAD` and report findings as
`path:line — rule — change`, ranked by impact:

1. Project rules: panics, overflow, allocation on hot paths, `Arc`/`dyn`,
   layering, comments.
2. Crate scope: deps a user pays for, a subdirectory in `src/`, a crate that
   holds more than one abstraction.
3. Module layout: code in non-leaf modules, file names, two public paths.
4. Signatures: bool params, tuples, magic values, `&Vec`/`&String`, std
   traits.
5. Public API: `#[non_exhaustive]`, `#[must_use]`, `Display`/`Error` on errors,
   `///` docs.
6. Imports and names: qualification, globs, `Ext`, abbreviations, shadowing.
7. Implementation: chains, `for_each`, assert messages, macros, `SAFETY`.

For a large diff, fan out one reviewer per crate group with this checklist. Run
fixes one at a time in the worktree: two builds in one target directory slow
each other down.
