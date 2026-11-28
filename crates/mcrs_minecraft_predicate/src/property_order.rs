use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::{BlockProperty, PropertyValue};
use mcrs_minecraft_block_predicate::state_properties::{StatePropertiesPredicate, ValueMatcher};
use mcrs_voxel::BlockStateId;

const DIRECTIONS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const DIRECTION_PROPERTIES: [&str; 2] = ["facing", "vertical_direction"];

pub(crate) fn passes(
    predicate: &StatePropertiesPredicate,
    block: &BlockEntry,
    state: BlockStateId,
) -> bool {
    predicate.0.iter().all(|(name, matcher)| {
        let Some(property) = block
            .properties
            .0
            .iter()
            .find(|property| *property.name == **name)
        else {
            return false;
        };
        let Some(value) = block.value_of(state, name) else {
            return false;
        };
        match matcher {
            ValueMatcher::Exact(text) => value.renders_to(text),
            ValueMatcher::Ranged { min, max } => {
                let Some(rank) = rank(property, value) else {
                    return false;
                };
                let limit = |text: &Option<String>| match text {
                    None => Some(None),
                    Some(text) => named_rank(property, text).map(Some),
                };
                let (Some(min), Some(max)) = (limit(min), limit(max)) else {
                    return false;
                };
                min.is_none_or(|min| min <= rank) && max.is_none_or(|max| rank <= max)
            }
        }
    })
}

fn named_rank(property: &BlockProperty, text: &str) -> Option<i64> {
    let value = property
        .values
        .iter()
        .find(|value| value.renders_to(text))?;
    rank(property, value)
}

fn rank(property: &BlockProperty, value: &PropertyValue) -> Option<i64> {
    match value {
        PropertyValue::Int(number) => Some(i64::from(*number)),
        PropertyValue::Bool(flag) => Some(i64::from(*flag)),
        PropertyValue::Str(name) => {
            let order = if DIRECTION_PROPERTIES.contains(&&*property.name) {
                DIRECTIONS
                    .iter()
                    .position(|direction| **name == **direction)
            } else {
                property.values.iter().position(|listed| listed == value)
            };
            order.and_then(|place| i64::try_from(place).ok())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_of_directions_with_another_name_keeps_its_listed_order_pred_blk_009() {
        let value = |name: &str| PropertyValue::Str(name.into());
        let property = BlockProperty {
            name: "direction".into(),
            values: vec![value("east"), value("west")],
        };
        assert!(
            rank(&property, &value("east")) < rank(&property, &value("west")),
            "the listed order puts east first"
        );
    }
}
