use std::fmt;
use std::marker::PhantomData;

/// A validated property identifier string. Must match `^[a-z0-9_]+$`
/// (the same rule Java's `StateDefinition.NAME_PATTERN` uses for both
/// property names and property value names).
///
/// Wraps `&'static str` for zero-cost static usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyStr(&'static str);

impl PropertyStr {
    /// Create at compile time. Panics in debug builds if invalid.
    pub const fn new(s: &'static str) -> Self {
        debug_assert!(Self::is_valid(s), "must match ^[a-z0-9_]+$");
        Self(s)
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }

    const fn is_valid(s: &str) -> bool {
        let b = s.as_bytes();
        if b.is_empty() {
            return false;
        }
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if !((c >= b'a' && c <= b'z') || (c >= b'0' && c <= b'9') || c == b'_') {
                return false;
            }
            i += 1;
        }
        true
    }
}

impl fmt::Display for PropertyStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Index of a property within a block's [`PropertyLayout`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyIndex(pub u8);

/// Type-erased property definition. All data is `&'static`.
///
/// Instances are compared by **pointer identity** (not by name) when used
/// as keys in [`PropertyLayout`] lookups — each static `PropertyDef` is
/// a unique property.
#[derive(Debug)]
pub struct PropertyDef {
    pub name: PropertyStr,
    /// Ordered list of allowed values; the array index is the value index.
    pub values: &'static [PropertyStr],
}

impl PropertyDef {
    /// Number of possible values.
    pub const fn count(&self) -> u8 {
        self.values.len() as u8
    }

    /// Look up the index of a value by string (const-compatible).
    pub const fn index_of(&self, value: &str) -> Option<u8> {
        let mut i = 0;
        while i < self.values.len() {
            if const_str_eq(self.values[i].as_str(), value) {
                return Some(i as u8);
            }
            i += 1;
        }
        None
    }

    /// Look up the string for a given value index.
    pub const fn value_str(&self, index: u8) -> Option<&'static str> {
        if (index as usize) < self.values.len() {
            Some(self.values[index as usize].as_str())
        } else {
            None
        }
    }
}

/// Describes the property layout for a block type. One instance per block (static).
///
/// Block state IDs within a block are assigned sequentially by iterating
/// properties in declaration order (last property varies fastest):
///
/// ```text
/// offset = v0 * stride[0] + v1 * stride[1] + ... + vn * stride[n]
/// stride[i] = |P(i+1)| * |P(i+2)| * ... * |Pn|,  stride[last] = 1
/// ```
#[derive(Debug)]
pub struct PropertyLayout {
    pub base_state_id: u16,
    pub properties: &'static [&'static PropertyDef],
    pub strides: &'static [u16],
    pub total_states: u16,
}

impl PropertyLayout {
    // ── Pointer-based lookup (primary API) ────────────────────────────

    /// Find a property's index by pointer identity of its [`PropertyDef`].
    ///
    /// This is O(n) in the number of properties but n ≤ 8 in practice,
    /// and uses a single pointer comparison per slot — no string work.
    pub fn index_of_def(&self, def: &PropertyDef) -> Option<PropertyIndex> {
        self.properties
            .iter()
            .position(|p| std::ptr::eq(*p, def))
            .map(|i| PropertyIndex(i as u8))
    }

    /// Get the raw value index of a property identified by its [`PropertyDef`].
    pub fn get_by_def(&self, state_id: u16, def: &PropertyDef) -> Option<u8> {
        let idx = self.index_of_def(def)?;
        Some(self.get_value_index(state_id, idx))
    }

    /// Return a new state ID with a property (identified by [`PropertyDef`])
    /// set to the given value index. Returns `None` if the property is not
    /// part of this layout or the value index is out of range.
    pub fn set_by_def(&self, state_id: u16, def: &PropertyDef, value_idx: u8) -> Option<u16> {
        let idx = self.index_of_def(def)?;
        if value_idx >= self.properties[idx.0 as usize].count() {
            return None;
        }
        Some(self.with_value(state_id, idx, value_idx))
    }

    // ── Typed API (using Property<T>) ────────────────────────────────

    /// Get a typed property value.
    pub fn get_typed<T: PropertyValue>(&self, state_id: u16, prop: &Property<T>) -> Option<T> {
        let idx = self.index_of_def(prop.def)?;
        let val_idx = self.get_value_index(state_id, idx);
        T::from_index(val_idx)
    }

    /// Set a typed property value. Returns `None` if the property is not
    /// part of this layout.
    pub fn set_typed<T: PropertyValue>(
        &self,
        state_id: u16,
        prop: &Property<T>,
        value: T,
    ) -> Option<u16> {
        let idx = self.index_of_def(prop.def)?;
        let value_idx = value.to_index();
        if value_idx >= self.properties[idx.0 as usize].count() {
            return None;
        }
        Some(self.with_value(state_id, idx, value_idx))
    }

    // ── Index-based API ──────────────────────────────────────────────

    /// Get the value index of a property for a given state ID.
    pub const fn get_value_index(&self, state_id: u16, prop_idx: PropertyIndex) -> u8 {
        let offset = state_id - self.base_state_id;
        let i = prop_idx.0 as usize;
        ((offset / self.strides[i]) % self.properties[i].count() as u16) as u8
    }

    /// Get the string representation of a property value for a given state ID.
    pub const fn get_value_str(&self, state_id: u16, prop_idx: PropertyIndex) -> &'static str {
        let idx = self.get_value_index(state_id, prop_idx);
        self.properties[prop_idx.0 as usize].values[idx as usize].as_str()
    }

    /// Return a new state ID with the given property set to a new value index.
    pub const fn with_value(&self, state_id: u16, prop_idx: PropertyIndex, value_idx: u8) -> u16 {
        let old_idx = self.get_value_index(state_id, prop_idx);
        let stride = self.strides[prop_idx.0 as usize];
        state_id - old_idx as u16 * stride + value_idx as u16 * stride
    }

    // ── String-based API (for serialization/deserialization) ─────────

    /// Return a new state ID with the given property set by string value.
    /// Returns `None` if the property name or value is unknown.
    pub const fn with_value_str(&self, state_id: u16, prop_name: &str, value: &str) -> Option<u16> {
        let Some(prop_idx) = self.property_by_name(prop_name) else {
            return None;
        };
        let Some(val_idx) = self.properties[prop_idx.0 as usize].index_of(value) else {
            return None;
        };
        Some(self.with_value(state_id, prop_idx, val_idx))
    }

    /// Find a property by name string.
    pub const fn property_by_name(&self, name: &str) -> Option<PropertyIndex> {
        let mut i = 0;
        while i < self.properties.len() {
            if const_str_eq(self.properties[i].name.as_str(), name) {
                return Some(PropertyIndex(i as u8));
            }
            i += 1;
        }
        None
    }
}

// ── PropertyValue trait ──────────────────────────────────────────────────

/// Trait for typed property values (bool, enum variants, integer indices, etc.).
///
/// Maps between a Rust type and the raw value index stored in the
/// block state layout.
pub trait PropertyValue: Copy + 'static {
    fn from_index(index: u8) -> Option<Self>;
    fn to_index(self) -> u8;
}

/// Matches vanilla ordering: `true` = index 0, `false` = index 1.
impl PropertyValue for bool {
    fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(true),
            1 => Some(false),
            _ => None,
        }
    }
    fn to_index(self) -> u8 {
        if self {
            0
        } else {
            1
        }
    }
}

/// Identity mapping — the value index IS the u8 value.
/// Used for integer properties like `note` (0..=24), `stage` (0..=1), etc.
/// Out-of-range values are rejected by [`PropertyLayout::set_typed`].
impl PropertyValue for u8 {
    fn from_index(index: u8) -> Option<Self> {
        Some(index)
    }
    fn to_index(self) -> u8 {
        self
    }
}

// ── Typed property handle ────────────────────────────────────────────────

/// Typed property handle that pairs a [`PropertyDef`] with a Rust type `T`.
///
/// Use with [`PropertyLayout::get_typed`] / [`PropertyLayout::set_typed`]
/// for type-safe property access.
///
/// ```rust,ignore
/// use mcrs_vanilla::block::state_properties::{SNOWY_PROP, POWERED_PROP};
///
/// let snowy: bool = block.get(state_id, &SNOWY_PROP).unwrap();
/// let new_state = block.set(state_id, &POWERED_PROP, true).unwrap();
/// ```
pub struct Property<T: PropertyValue> {
    pub def: &'static PropertyDef,
    _marker: PhantomData<T>,
}

impl<T: PropertyValue> Property<T> {
    pub const fn new(def: &'static PropertyDef) -> Self {
        Self {
            def,
            _marker: PhantomData,
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Const-compatible string equality check.
pub const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
