#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwingRecipients {
    #[default]
    Observers,
    SelfAndObservers,
}
