use core::fmt;

use mcrs_molang::{
    Argument, CompileOptions, Diagnostic, ErrorKind, Expression, Literal, Milestone, QueryBinding,
    QueryError, QueryHost, QuerySlot, QueryValues, Rate, Resolution, ScalarVm, Subject, UseContext,
    Value, ValueType, VariableLayout, Variables, compile_text, string_hash,
};

use super::schema::{BlockProperties, PropertyValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionError(pub Vec<Diagnostic>);

impl fmt::Display for ConditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut errors = self.0.iter();
        match errors.next() {
            Some(first) => write!(f, "{}", first.message)?,
            None => f.write_str("the condition does not compile")?,
        }
        for error in errors {
            write!(f, "; {}", error.message)?;
        }
        Ok(())
    }
}

impl core::error::Error for ConditionError {}

struct BlockQueries<'a> {
    properties: &'a BlockProperties,
}

fn value_type(values: &[PropertyValue]) -> ValueType {
    match values.first() {
        Some(PropertyValue::Str(_)) => ValueType::String,
        _ => ValueType::Number,
    }
}

fn accepts(value: &PropertyValue, literal: Literal<'_>) -> bool {
    match (value, literal) {
        (PropertyValue::Bool(value), Literal::Bool(literal)) => *value == literal,
        (PropertyValue::Int(value), Literal::Number(literal)) => *value as f32 == literal,
        (PropertyValue::Str(value), Literal::String(literal)) => **value == *literal,
        _ => false,
    }
}

fn literal_text(literal: Literal<'_>) -> String {
    match literal {
        Literal::Number(value) => value.to_string(),
        Literal::Bool(value) => value.to_string(),
        Literal::String(text) => format!("'{text}'"),
    }
}

impl QueryHost for BlockQueries<'_> {
    fn use_context(&self) -> UseContext {
        UseContext::Block
    }

    fn resolve_query(
        &mut self,
        name: &str,
        arguments: Option<&[Argument<'_>]>,
        _milestone: Milestone,
    ) -> Result<Resolution, QueryError> {
        if name != "block_state" {
            return Ok(Resolution::Unknown);
        }
        let property = match arguments {
            Some([Argument::Literal(Literal::String(property))]) => *property,
            Some([_]) => {
                return Err(QueryError::new(
                    ErrorKind::StaticType,
                    "`query.block_state` takes the name of a property as a string literal",
                ));
            }
            _ => {
                return Err(QueryError::new(
                    ErrorKind::Arity,
                    "`query.block_state` takes one argument",
                ));
            }
        };
        let index = self.properties.index_of(property).ok_or_else(|| {
            QueryError::new(
                ErrorKind::StaticType,
                format!("unknown block state property `{property}`"),
            )
        })?;
        let slot = u16::try_from(index).map_err(|_| {
            QueryError::new(ErrorKind::StaticType, "too many block state properties")
        })?;
        let values = self
            .properties
            .0
            .get(index)
            .map_or(&[][..], |property| property.values.as_slice());
        Ok(Resolution::Bound(QueryBinding {
            slot: QuerySlot(slot),
            value_type: value_type(values),
            rate: Rate::Load,
        }))
    }

    fn check_comparison(
        &self,
        binding: QueryBinding,
        literal: Literal<'_>,
    ) -> Result<(), QueryError> {
        let Some(property) = self.properties.0.get(usize::from(binding.slot.0)) else {
            return Ok(());
        };
        if property.values.iter().any(|value| accepts(value, literal)) {
            return Ok(());
        }
        Err(QueryError::new(
            ErrorKind::StaticType,
            format!(
                "`{}` is not a value of block state property `{}`",
                literal_text(literal),
                property.name
            ),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct StateInputs {
    properties: Vec<PropertyInputs>,
    queries: QueryValues,
}

#[derive(Clone, Debug)]
struct PropertyInputs {
    values: Vec<u64>,
    absent: u64,
}

impl StateInputs {
    #[must_use]
    pub fn new(properties: &BlockProperties) -> Self {
        let properties: Vec<PropertyInputs> = properties
            .0
            .iter()
            .map(|property| PropertyInputs {
                values: property.values.iter().map(raw_value).collect(),
                absent: match value_type(&property.values) {
                    ValueType::Number => u64::from(f32::NAN.to_bits()),
                    _ => u64::MAX,
                },
            })
            .collect();
        let queries = QueryValues::new(properties.len());
        Self {
            properties,
            queries,
        }
    }

    pub fn select(&mut self, indices: &[u8]) {
        for (slot, property) in self.properties.iter().enumerate() {
            let raw = indices
                .get(slot)
                .and_then(|index| property.values.get(usize::from(*index)))
                .copied()
                .unwrap_or(property.absent);
            if let Ok(slot) = u16::try_from(slot) {
                self.queries.set(QuerySlot(slot), Some(raw));
            }
        }
    }
}

fn raw_value(value: &PropertyValue) -> u64 {
    match value {
        PropertyValue::Bool(value) => u64::from(if *value { 1.0f32 } else { 0.0 }.to_bits()),
        PropertyValue::Int(value) => u64::from((*value as f32).to_bits()),
        PropertyValue::Str(value) => string_hash(value),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateCondition {
    expression: Expression,
    layout: VariableLayout,
}

impl StateCondition {
    pub fn compile(source: &str, properties: &BlockProperties) -> Result<Self, ConditionError> {
        let mut layout = VariableLayout::new();
        let mut host = BlockQueries { properties };
        let compiled = compile_text(source, &mut layout, &mut host, &CompileOptions::default())
            .map_err(ConditionError)?;
        Ok(Self {
            expression: compiled.expression,
            layout,
        })
    }

    pub fn matches(&self, vm: &mut ScalarVm, inputs: &StateInputs) -> bool {
        let value = match &self.expression {
            Expression::Constant(value) => *value,
            expression => {
                let mut variables = Variables::new(&self.layout);
                let mut subject = Subject {
                    variables: &mut variables,
                    queries: &inputs.queries,
                    this: 0.0,
                };
                expression.evaluate(vm, &mut subject)
            }
        };
        matches!(value, Value::Number(number) if number != 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties() -> BlockProperties {
        serde_json::from_str(
            r#"{
                "facing": ["north", "south"],
                "level": [0, 1, 2],
                "waterlogged": [true, false]
            }"#,
        )
        .unwrap()
    }

    fn holds(source: &str, indices: &[u8]) -> bool {
        let properties = properties();
        let condition = StateCondition::compile(source, &properties).unwrap();
        let mut inputs = StateInputs::new(&properties);
        inputs.select(indices);
        condition.matches(&mut ScalarVm::new(), &inputs)
    }

    fn error(source: &str) -> String {
        StateCondition::compile(source, &properties())
            .unwrap_err()
            .to_string()
    }

    struct ConditionCase {
        source: &'static str,
        state: [u8; 3],
        holds: bool,
    }

    #[test]
    fn a_condition_holds_for_the_states_its_expression_selects() {
        const CONJUNCTION: &str =
            "q.block_state('facing') == 'south' && q.block_state('level') != 2";
        const OR_LOOSER_THAN_AND: &str = "q.block_state('facing') == 'north' \
             && q.block_state('level') == 0 || q.block_state('waterlogged') == true";
        const NEGATED_GROUP: &str =
            "!(q.block_state('facing') == 'north' || q.block_state('level') == 1)";
        let case = |source, state, holds| ConditionCase {
            source,
            state,
            holds,
        };
        let cases = [
            case(CONJUNCTION, [1, 0, 0], true),
            case(CONJUNCTION, [1, 2, 0], false),
            case(CONJUNCTION, [0, 0, 0], false),
            case(OR_LOOSER_THAN_AND, [0, 0, 1], true),
            case(OR_LOOSER_THAN_AND, [1, 2, 0], true),
            case(OR_LOOSER_THAN_AND, [1, 2, 1], false),
            case(NEGATED_GROUP, [1, 0, 0], true),
            case(NEGATED_GROUP, [0, 0, 0], false),
            case(NEGATED_GROUP, [1, 1, 0], false),
            case("'north' == q.block_state('facing')", [0, 0, 0], true),
            case(
                "q.block_state('waterlogged') == false",
                [0, 0, u8::MAX],
                false,
            ),
            case("q.block_state('facing') != 'north'", [u8::MAX, 0, 0], true),
        ];

        for case in cases {
            assert_eq!(
                holds(case.source, &case.state),
                case.holds,
                "`{}` at the state {:?}",
                case.source,
                case.state
            );
        }
    }

    #[test]
    fn a_literal_outside_the_domain_of_the_property_is_a_load_error() {
        assert_eq!(
            error("q.block_state('waterlogged') == 'true'"),
            "`'true'` is not a value of block state property `waterlogged`"
        );
        assert_eq!(
            error("q.block_state('facing') == 0"),
            "`0` is not a value of block state property `facing`"
        );
        assert_eq!(
            error("q.block_state('powered') == true"),
            "unknown block state property `powered`"
        );
        assert!(error("q.is_open == true").contains("query.is_open"));
    }

    #[test]
    fn the_block_state_query_takes_one_string_literal() {
        assert!(error("q.block_state() == 'north'").contains("takes one argument"));
        assert!(
            error("q.block_state('facing', 'level') == 'north'").contains("takes one argument")
        );
        assert!(
            error("q.block_state(1) == 'north'").contains("name of a property as a string literal")
        );
    }
}
