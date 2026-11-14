/// The settings of the falling block that the host passes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Settings {
    /// The maximum age of FBK-TCK-004: a falling block that is older and has not landed drops.
    /// 0 disables that limit.
    pub max_age: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { max_age: 600 }
    }
}
