use mcrs_protocol_macros::{Decode, Encode};
use std::borrow::Cow;
use valence_ident::Ident;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum Status {
    SuccessfullyLoaded,
    Declined,
    FailedDownload,
    Accepted,
    Downloaded,
    InvalidUrl,
    FailedReload,
    Discarded,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct KnownPack<'a> {
    pub namespace: &'a str,
    pub id: &'a str,
    pub version: &'a str,
}
