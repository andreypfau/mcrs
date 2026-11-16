/// Who a [`Request`](crate::Request) comes from, as the denial point sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequestKind {
    /// A request of a player or of a plugin. It reaches [`RequestCheck`](crate::RequestCheck).
    Action,
    /// The moves of a close. They never reach [`RequestCheck`](crate::RequestCheck)
    /// (ITX-ATM-012).
    Close,
}
