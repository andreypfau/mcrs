use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::diagnostic::ErrorKind;
use crate::value::{Milestone, Rate, ValueType};

/// The consumers in which an expression runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UseContext {
    /// A permutation condition of a block definition. An unknown query is a load error.
    Block,
    /// A script, animation, controller or render controller of a client entity.
    Entity,
}

/// The consumers in which a query is valid.
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

/// A literal in the source text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Literal<'a> {
    Number(f32),
    Bool(bool),
    String(&'a str),
}

/// An argument of a query call, as the compiler sees it at load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Argument<'a> {
    Literal(Literal<'a>),
    /// An argument whose value is known only when the expression runs.
    Dynamic,
}

/// An index into the query values of a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QuerySlot(pub u16);

/// A query reference that the host resolved at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueryBinding {
    pub slot: QuerySlot,
    pub value_type: ValueType,
    pub rate: Rate,
}

/// The answer of a host for a query name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Bound(QueryBinding),
    /// No declaration has the name.
    Unknown,
    /// The declaration does not include the context of use.
    OutOfContext,
}

/// A load error that a host reports for a query reference.
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

/// The part of a consumer that declares queries and context values, and supplies their values
/// in the slots that it gives at load.
pub trait QueryHost {
    fn use_context(&self) -> UseContext;

    /// Resolves `query.<name>(<arguments>)`. A host can give each set of literal arguments its
    /// own slot.
    fn resolve_query(
        &mut self,
        name: &str,
        arguments: Option<&[Argument<'_>]>,
        milestone: Milestone,
    ) -> Result<Resolution, QueryError>;

    /// Checks a comparison of a query result with a literal, for example that the literal is a
    /// value of a block property.
    fn check_comparison(
        &self,
        binding: QueryBinding,
        literal: Literal<'_>,
    ) -> Result<(), QueryError> {
        let _ = (binding, literal);
        Ok(())
    }

    /// Resolves `context.<name>`. `None` means that the consumer does not supply it.
    fn resolve_context(&mut self, name: &str) -> Option<QueryBinding> {
        let _ = name;
        None
    }
}

/// The arguments that a query declaration takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arguments {
    /// No argument list, or an empty one.
    None,
    /// A fixed number of literal arguments; each set of values gets its own slot.
    Literals(u8),
}

/// One query that a host supplies.
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
    /// A query of the entity scope that the first milestone supports.
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

/// An owned literal argument, the key of a query slot.
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

/// A query slot of a [`QueryRegistry`]: a declaration with one set of literal arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotKey {
    pub declaration: usize,
    pub arguments: Vec<LiteralKey>,
}

/// A table of query declarations that gives one slot to each query reference with distinct
/// literal arguments. The host samples each slot of [`QueryRegistry::slots`] for each subject.
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

    /// Adds a declaration. A later declaration with the same name replaces the earlier one.
    pub fn declare(&mut self, declaration: QueryDeclaration) {
        let name = declaration.name.to_ascii_lowercase().into_boxed_str();
        self.by_name.insert(name, self.declarations.len());
        self.declarations.push(declaration);
    }

    /// Declares a context value that the consumer supplies for each run.
    pub fn declare_context(&mut self, name: &str, value_type: ValueType) {
        self.contexts
            .push((name.to_ascii_lowercase().into_boxed_str(), value_type));
    }

    #[must_use]
    pub fn declaration(&self, index: usize) -> Option<&QueryDeclaration> {
        self.declarations.get(index)
    }

    /// The slots in slot order. A context value has the declaration index `usize::MAX`.
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
