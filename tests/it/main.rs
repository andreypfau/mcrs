#![expect(
    clippy::disallowed_methods,
    reason = "the test finds the vanilla server to join through the environment"
)]

mod client_joins_embedded_server;
mod dimension_routers;
mod support;
mod vanilla_server;
