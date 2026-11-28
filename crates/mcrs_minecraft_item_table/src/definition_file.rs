#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionFile<'file> {
    pub name: &'file str,
    pub bytes: &'file [u8],
}
