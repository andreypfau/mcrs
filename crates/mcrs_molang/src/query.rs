use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::diagnostic::ErrorKind;
use crate::value::{Milestone, Rate, ValueType};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UseContext {
    Block,
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryScope {
    Block,
    Entity,
    Any,
}

impl QueryScope {
    #[must_use]
    pub const fn includes(self, context: UseContext) -> bool {
        matches!(
            (self, context),
            (Self::Any, _) | (Self::Block, UseContext::Block) | (Self::Entity, UseContext::Entity)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Literal<'a> {
    Number(f32),
    Bool(bool),
    String(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Argument<'a> {
    Literal(Literal<'a>),
    Dynamic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuerySlot(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueryBinding {
    pub slot: QuerySlot,
    pub value_type: ValueType,
    pub rate: Rate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Bound(QueryBinding),
    Unknown,
    OutOfContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    pub kind: ErrorKind,
    pub message: String,
}

impl QueryError {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub trait QueryHost {
    fn use_context(&self) -> UseContext;

    fn resolve_query(
        &mut self,
        name: &str,
        arguments: Option<&[Argument<'_>]>,
        milestone: Milestone,
    ) -> Result<Resolution, QueryError>;

    fn check_comparison(
        &self,
        binding: QueryBinding,
        literal: Literal<'_>,
    ) -> Result<(), QueryError> {
        let _ = (binding, literal);
        Ok(())
    }

    fn resolve_context(&mut self, name: &str) -> Option<QueryBinding> {
        let _ = name;
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arguments {
    None,
    Literals(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryDeclaration {
    pub name: Box<str>,
    pub arguments: Arguments,
    pub result: ValueType,
    pub rate: Rate,
    pub scope: QueryScope,
    pub milestone: Milestone,
}

impl QueryDeclaration {
    #[must_use]
    pub fn entity(name: &str, result: ValueType, arguments: Arguments, rate: Rate) -> Self {
        Self {
            name: name.into(),
            arguments,
            result,
            rate,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LiteralKey {
    Number(u32),
    String(Box<str>),
}

impl From<Literal<'_>> for LiteralKey {
    fn from(literal: Literal<'_>) -> Self {
        match literal {
            Literal::Number(value) => Self::Number(value.to_bits()),
            Literal::Bool(value) => Self::Number(if value { 1.0f32 } else { 0.0 }.to_bits()),
            Literal::String(text) => Self::String(text.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotKey {
    pub declaration: usize,
    pub arguments: Vec<LiteralKey>,
}

#[derive(Clone, Debug)]
pub struct QueryRegistry {
    context: UseContext,
    declarations: Vec<QueryDeclaration>,
    by_name: HashMap<Box<str>, usize>,
    slots: Vec<SlotKey>,
    slot_index: HashMap<SlotKey, QuerySlot>,
    contexts: Vec<(Box<str>, ValueType)>,
}

impl QueryRegistry {
    #[must_use]
    pub fn new(context: UseContext) -> Self {
        Self {
            context,
            declarations: Vec::new(),
            by_name: HashMap::new(),
            slots: Vec::new(),
            slot_index: HashMap::new(),
            contexts: Vec::new(),
        }
    }

    pub fn declare(&mut self, declaration: QueryDeclaration) {
        let name = declaration.name.to_ascii_lowercase().into_boxed_str();
        self.by_name.insert(name, self.declarations.len());
        self.declarations.push(declaration);
    }

    pub fn declare_context(&mut self, name: &str, value_type: ValueType) {
        self.contexts
            .push((name.to_ascii_lowercase().into_boxed_str(), value_type));
    }

    #[must_use]
    pub fn declaration(&self, index: usize) -> Option<&QueryDeclaration> {
        self.declarations.get(index)
    }

    pub fn slots(&self) -> impl Iterator<Item = (QuerySlot, &SlotKey)> {
        (0u16..).map(QuerySlot).zip(&self.slots)
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    fn slot(&mut self, key: SlotKey) -> Result<QuerySlot, QueryError> {
        if let Some(slot) = self.slot_index.get(&key) {
            return Ok(*slot);
        }
        let index = u16::try_from(self.slots.len())
            .map_err(|_| QueryError::new(ErrorKind::Unsupported, "too many query slots"))?;
        let slot = QuerySlot(index);
        self.slots.push(key.clone());
        self.slot_index.insert(key, slot);
        Ok(slot)
    }
}

impl QueryHost for QueryRegistry {
    fn use_context(&self) -> UseContext {
        self.context
    }

    fn resolve_query(
        &mut self,
        name: &str,
        arguments: Option<&[Argument<'_>]>,
        milestone: Milestone,
    ) -> Result<Resolution, QueryError> {
        let Some(&index) = self.by_name.get(name) else {
            return Ok(Resolution::Unknown);
        };
        let Some(declaration) = self.declarations.get(index) else {
            return Ok(Resolution::Unknown);
        };
        if declaration.milestone > milestone {
            return Err(QueryError::new(
                ErrorKind::Unsupported,
                alloc::format!("`query.{name}` belongs to a later milestone"),
            ));
        }
        if !declaration.scope.includes(self.context) {
            return Ok(Resolution::OutOfContext);
        }
        let given = arguments.unwrap_or_default();
        let expected = match declaration.arguments {
            Arguments::None => 0,
            Arguments::Literals(count) => usize::from(count),
        };
        if given.len() != expected {
            return Err(QueryError::new(
                ErrorKind::Arity,
                alloc::format!(
                    "`query.{name}` takes {expected} arguments, not {}",
                    given.len()
                ),
            ));
        }
        let mut literals = Vec::with_capacity(given.len());
        for argument in given {
            match argument {
                Argument::Literal(literal) => literals.push(LiteralKey::from(*literal)),
                Argument::Dynamic => {
                    return Err(QueryError::new(
                        ErrorKind::StaticType,
                        alloc::format!("`query.{name}` needs literal arguments"),
                    ));
                }
            }
        }
        let binding = QueryBinding {
            slot: QuerySlot(0),
            value_type: declaration.result,
            rate: declaration.rate,
        };
        let slot = self.slot(SlotKey {
            declaration: index,
            arguments: literals,
        })?;
        Ok(Resolution::Bound(QueryBinding { slot, ..binding }))
    }

    fn resolve_context(&mut self, name: &str) -> Option<QueryBinding> {
        let value_type = self
            .contexts
            .iter()
            .find(|(candidate, _)| &**candidate == name)?
            .1;
        let slot = self
            .slot(SlotKey {
                declaration: usize::MAX,
                arguments: alloc::vec![LiteralKey::String(name.into())],
            })
            .ok()?;
        Some(QueryBinding {
            slot,
            value_type,
            rate: Rate::EntityTick,
        })
    }
}
