/// One file of the item definitions: the JSON of the prototype of one item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionFile<'file> {
    /// The name that an error gives for a file that has no readable item identifier.
    pub name: &'file str,
    pub bytes: &'file [u8],
}
