#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "the tests speak MCP, whose messages are open JSON objects"
)]
#![expect(clippy::unwrap_used, reason = "a test fixture fails loudly")]

mod lifecycle;
mod orphans;
mod sessions;
mod support;
