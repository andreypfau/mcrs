use alloc::string::String;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Serialize;
use serde::de::{Error as _, MapAccess};

use crate::instance_modifier::InstanceModifier;
use crate::modifier_kind::ModifierKind;
use crate::modifier_operation::ModifierOperation;
use crate::tolerant::{Lenient, Tolerant};

/// A permanent modifier in the save (`persistence.md`, the attribute compound): each field is
/// required, and a load leaves out a modifier with a missing or bad field (ATR-SAV-002).
#[derive(Clone, Debug, Serialize)]
pub struct SavedModifier {
    /// The identifier of the modifier.
    pub id: ResourceLocationBuf,
    /// The amount, as the instance holds it.
    pub amount: f64,
    /// The operation, by its name of ATR-INS-003.
    pub operation: ModifierOperation,
}

impl PartialEq for SavedModifier {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.amount.to_bits() == other.amount.to_bits()
            && self.operation == other.operation
    }
}

impl SavedModifier {
    /// The saved form of `modifier`, or `None` when it is transient (ATR-SAV-001).
    #[must_use]
    pub(crate) fn from_modifier(modifier: &InstanceModifier) -> Option<Self> {
        (modifier.kind() == ModifierKind::Permanent).then(|| Self {
            id: modifier.id().to_buf(),
            amount: modifier.amount(),
            operation: modifier.operation(),
        })
    }

    /// The permanent modifier that a load sets (ATR-SAV-002, step 2).
    #[must_use]
    pub(crate) fn into_modifier(self) -> InstanceModifier {
        InstanceModifier::new(
            self.id,
            self.amount,
            self.operation,
            ModifierKind::Permanent,
        )
    }
}

const FIELDS: &[&str] = &["id", "amount", "operation"];

impl Tolerant for SavedModifier {
    fn from_compound<'de, M: MapAccess<'de>>(mut map: M) -> Result<Option<Self>, M::Error> {
        let mut id = None;
        let mut amount = None;
        let mut operation = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "id" => id = map.next_value::<Lenient<ResourceLocationBuf>>()?.0,
                "amount" => amount = map.next_value::<Lenient<f64>>()?.0,
                "operation" => operation = map.next_value::<Lenient<ModifierOperation>>()?.0,
                _ => return Err(M::Error::unknown_field(&key, FIELDS)),
            }
        }
        let (Some(id), Some(amount), Some(operation)) = (id, amount, operation) else {
            return Ok(None);
        };
        Ok(Some(Self {
            id,
            amount,
            operation,
        }))
    }
}
