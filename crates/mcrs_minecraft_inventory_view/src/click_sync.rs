/// The answer to a click, chosen by its state id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClickSync {
    /// The state id of the packet is stale: send each entry and the cursor in one content packet.
    FullContent,
    /// The state id is current: send each entry and the cursor that do not match the client view.
    CompareEntries,
}
