#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloseOrigin {
    Server,
    Client,
}
