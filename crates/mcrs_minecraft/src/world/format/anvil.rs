use mcrs_anvil::{BlockStateLookup, Properties};
use mcrs_vanilla::block::definition::BlockDefinitions;
use mcrs_vanilla::block::definition::schema::PropertyValue;
use smallvec::SmallVec;

/// Resolves a saved palette entry against the corpus.
///
/// A save states every property as text. The type is never inferred from that
/// text — `"true"` and `"5"` are a string for any block that declares them as
/// one — so each declared value is rendered and compared instead, leaving the
/// corpus the authority on what a property holds.
pub struct CorpusBlockStates<'a>(pub &'a BlockDefinitions);

impl BlockStateLookup for CorpusBlockStates<'_> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u32> {
        let block = self.0.block(name)?;
        if properties.len() != block.properties.0.len() {
            return None;
        }
        let mut values: SmallVec<[(&str, PropertyValue); 8]> = SmallVec::new();
        for property in &block.properties.0 {
            let text = properties.get(&property.name)?;
            let value = property.values.iter().find(|v| v.renders_to(text))?;
            values.push((&property.name, value.clone()));
        }
        block.state_id(&values).map(|id| id.0 as u32)
    }
}
