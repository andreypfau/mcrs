#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]
mod accept_rate;
mod pre_game_ingest;
mod proxy_protocol;
mod status;
mod version_check;
mod webtransport_session;
