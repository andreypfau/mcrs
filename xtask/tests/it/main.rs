#![cfg_attr(
    unix,
    expect(
        clippy::disallowed_methods,
        reason = "the test puts its stub tools ahead of the real ones on the search path"
    )
)]

#[cfg(unix)]
mod mutation_shard;
