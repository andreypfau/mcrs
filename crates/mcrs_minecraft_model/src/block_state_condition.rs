use mcrs_minecraft_block::definition::schema::BlockProperties;
use mcrs_minecraft_block::definition::state_condition::{
    ConditionError, StateCondition, StateInputs,
};
use mcrs_molang::ScalarVm;

/// A Molang condition in the `block` context, compiled against the properties of one block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockStateCondition(StateCondition);

impl BlockStateCondition {
    pub fn compile(source: &str, properties: &BlockProperties) -> Result<Self, ConditionError> {
        StateCondition::compile(source, properties).map(Self)
    }

    /// Whether the condition holds for the state with these property values, given as text.
    pub fn holds(&self, properties: &BlockProperties, state: &[(&str, &str)]) -> bool {
        let indices: Vec<u8> = properties
            .0
            .iter()
            .map(|property| {
                state
                    .iter()
                    .find(|(name, _)| *name == &*property.name)
                    .and_then(|(_, text)| {
                        property
                            .values
                            .iter()
                            .position(|value| value.renders_to(text))
                    })
                    .and_then(|index| u8::try_from(index).ok())
                    .unwrap_or(u8::MAX)
            })
            .collect();
        let mut inputs = StateInputs::new(properties);
        inputs.select(&indices);
        self.0.matches(&mut ScalarVm::new(), &inputs)
    }
}
