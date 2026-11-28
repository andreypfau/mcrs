#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InflatedFile {
    pub name: String,
    pub bytes: Vec<u8>,
}
