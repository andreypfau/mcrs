use mcrs_core::block_state::{Property, PropertyDef, PropertyLayout, PropertyValue};
use mcrs_core::resource_location::ResourceLocation;
use mcrs_core::tag::key::TagRegistryType;
use mcrs_protocol::BlockStateId;
use std::hash::{Hash, Hasher};

pub mod behaviour;
#[macro_use]
mod macros;
pub mod minecraft;
pub mod state_properties;
pub mod tags;

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct BlockUpdateFlags: u32 {
        const NEIGHBORS = 1;
        const CLIENTS   = 2;
        const INVISIBLE = 4;
        const IMMEDIATE = 8;
        const KNOWN_SHAPE = 16;
        const SUPPRESS_DROPS = 32;
        const MOVE_BY_PISTON = 64;
        const SKIP_SHAPE_UPDATE_ON_WIRE = 128;
        const SKIP_BLOCK_ENTITY_SIDEEFFECTS = 256;
        const SKIP_ON_PLACE = 512;
        const NONE = BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDEEFFECTS.bits() | BlockUpdateFlags::INVISIBLE.bits();
        const ALL = BlockUpdateFlags::NEIGHBORS.bits() | BlockUpdateFlags::CLIENTS.bits();
        const ALL_IMMEDIATE = BlockUpdateFlags::ALL.bits() | BlockUpdateFlags::IMMEDIATE.bits();
    }
}

#[derive(Debug)]
pub struct Block {
    pub identifier: ResourceLocation<&'static str>,
    /// Vanilla `minecraft:block` registry index (protocol ID).
    /// Must match the client's built-in registry ordering.
    pub protocol_id: u16,
    pub properties: &'static behaviour::Properties,
    pub default_state_id: BlockStateId,
    pub layout: Option<&'static PropertyLayout>,
    pub state_count: u16,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}

impl Block {
    #[inline]
    pub fn hardness(&self) -> f32 {
        self.properties.hardness
    }

    pub fn explosion_resistance(&self) -> f32 {
        self.properties.explosion_resistance
    }

    pub fn requires_correct_tool_for_drops(&self) -> bool {
        self.properties.requires_correct_tool_for_drops
    }

    pub fn xp_range(&self) -> Option<(u32, u32)> {
        self.properties.xp_range
    }

    /// The base (lowest) state ID for this block.
    pub fn base_state_id(&self) -> BlockStateId {
        match self.layout {
            Some(l) => BlockStateId(l.base_state_id),
            None => self.default_state_id,
        }
    }

    /// Whether the given state ID belongs to this block.
    pub fn owns_state(&self, state_id: BlockStateId) -> bool {
        let base = self.base_state_id().0;
        state_id.0 >= base && state_id.0 < base + self.state_count
    }

    // ── Typed property API ──────────────────────────────────────────

    /// Get a typed property value from a state ID.
    pub fn get<T: PropertyValue>(&self, state_id: BlockStateId, prop: &Property<T>) -> Option<T> {
        self.layout?.get_typed(state_id.0, prop)
    }

    /// Return a new state ID with a typed property set to a new value.
    pub fn set<T: PropertyValue>(
        &self,
        state_id: BlockStateId,
        prop: &Property<T>,
        value: T,
    ) -> Option<BlockStateId> {
        self.layout?
            .set_typed(state_id.0, prop, value)
            .map(BlockStateId)
    }

    // ── Raw PropertyDef API ─────────────────────────────────────────

    /// Get the raw value index of a property by its [`PropertyDef`].
    pub fn get_value_index(&self, state_id: BlockStateId, def: &PropertyDef) -> Option<u8> {
        self.layout?.get_by_def(state_id.0, def)
    }

    /// Return a new state ID with a property (by [`PropertyDef`]) set to a raw value index.
    pub fn with_value_index(
        &self,
        state_id: BlockStateId,
        def: &PropertyDef,
        value_idx: u8,
    ) -> Option<BlockStateId> {
        self.layout?
            .set_by_def(state_id.0, def, value_idx)
            .map(BlockStateId)
    }

    // ── String-based API (serialization/deserialization) ────────────

    /// Get a property value as a string for a given state ID.
    pub fn get_property_str(
        &self,
        state_id: BlockStateId,
        prop_name: &str,
    ) -> Option<&'static str> {
        let layout = self.layout?;
        let prop_idx = layout.property_by_name(prop_name)?;
        Some(layout.get_value_str(state_id.0, prop_idx))
    }

    /// Return a new state ID with a property set by string name and value.
    pub fn with_property_str(
        &self,
        state_id: BlockStateId,
        prop_name: &str,
        value: &str,
    ) -> Option<BlockStateId> {
        let layout = self.layout?;
        layout
            .with_value_str(state_id.0, prop_name, value)
            .map(BlockStateId)
    }

    // ── StateBuilder ────────────────────────────────────────────────

    /// Start building a state from this block's default state.
    pub fn state(&'static self) -> StateBuilder {
        StateBuilder {
            block: self,
            state_id: self.default_state_id,
        }
    }
}

/// Fluent builder for constructing a [`BlockStateId`] with multiple property values.
///
/// ```rust,ignore
/// use mcrs_vanilla::block::state_properties::*;
/// use mcrs_vanilla::block::minecraft::NOTE_BLOCK;
///
/// let state_id = NOTE_BLOCK
///     .state()
///     .set(&NOTE_PROP, 12u8)
///     .set(&POWERED_PROP, true)
///     .id();
/// ```
pub struct StateBuilder {
    block: &'static Block,
    state_id: BlockStateId,
}

impl StateBuilder {
    /// Set a typed property value. Panics if the property doesn't belong to this block
    /// or the value is out of range.
    pub fn set<T: PropertyValue>(mut self, prop: &Property<T>, value: T) -> Self {
        self.state_id = self
            .block
            .set(self.state_id, prop, value)
            .expect("property not found in block layout or value out of range");
        self
    }

    /// Try to set a typed property value. Returns `None` if the property doesn't
    /// belong to this block or the value is out of range.
    pub fn try_set<T: PropertyValue>(mut self, prop: &Property<T>, value: T) -> Option<Self> {
        self.state_id = self.block.set(self.state_id, prop, value)?;
        Some(self)
    }

    /// Get the resulting state ID.
    pub fn id(self) -> BlockStateId {
        self.state_id
    }
}

impl From<StateBuilder> for BlockStateId {
    fn from(builder: StateBuilder) -> Self {
        builder.state_id
    }
}

impl TagRegistryType for Block {
    const REGISTRY_PATH: &'static str = "block";
}

impl From<&'static Block> for BlockStateId {
    fn from(block: &'static Block) -> Self {
        block.default_state_id
    }
}

#[cfg(test)]
mod tests {
    use super::minecraft::*;
    use super::state_properties::*;
    use crate::block_state;
    use mcrs_protocol::BlockStateId;

    #[test]
    fn declarative_macro_default_state() {
        // No overrides → default state
        let id = block_state!(GRASS_BLOCK, {});
        assert_eq!(id, GRASS_BLOCK.default_state_id);
    }

    #[test]
    fn declarative_macro_single_prop() {
        let id = block_state!(GRASS_BLOCK, { snowy: true });
        // snowy: true = index 0, base_state_id = 8 → id = 8
        assert_eq!(id, BlockStateId(8));
        // Verify via typed API
        assert_eq!(GRASS_BLOCK.get(id, &SNOWY_PROP), Some(true));
    }

    #[test]
    fn declarative_macro_multi_prop() {
        let id = block_state!(NOTE_BLOCK, { note: 12, powered: true });
        assert_eq!(NOTE_BLOCK.get(id, &NOTE_PROP), Some(12u8));
        assert_eq!(NOTE_BLOCK.get(id, &POWERED_PROP), Some(true));
    }

    #[test]
    fn typed_macro_single_prop() {
        let id = block_state!(GRASS_BLOCK, SNOWY_PROP => true);
        assert_eq!(GRASS_BLOCK.get(id, &SNOWY_PROP), Some(true));
    }

    #[test]
    fn typed_macro_multi_prop() {
        let id = block_state!(NOTE_BLOCK, NOTE_PROP => 12u8, POWERED_PROP => true);
        assert_eq!(NOTE_BLOCK.get(id, &NOTE_PROP), Some(12u8));
        assert_eq!(NOTE_BLOCK.get(id, &POWERED_PROP), Some(true));
    }

    #[test]
    fn builder_round_trip() {
        let id = NOTE_BLOCK
            .state()
            .set(&NOTE_PROP, 24u8)
            .set(&POWERED_PROP, true)
            .id();
        assert_eq!(NOTE_BLOCK.get(id, &NOTE_PROP), Some(24u8));
        assert_eq!(NOTE_BLOCK.get(id, &POWERED_PROP), Some(true));
        // instrument should still be default (harp = index 0)
        assert_eq!(NOTE_BLOCK.get_property_str(id, "instrument"), Some("harp"));
    }

    #[test]
    fn axis_typed_round_trip() {
        let id = block_state!(PALE_OAK_WOOD, AXIS_PROP => Axis::Z);
        assert_eq!(PALE_OAK_WOOD.get(id, &AXIS_PROP), Some(Axis::Z));
        assert_eq!(PALE_OAK_WOOD.get_property_str(id, "axis"), Some("z"));
    }

    #[test]
    fn declarative_and_typed_agree() {
        let decl = block_state!(NOTE_BLOCK, { note: 7, powered: true });
        let typed = block_state!(NOTE_BLOCK, NOTE_PROP => 7u8, POWERED_PROP => true);
        assert_eq!(decl, typed);
    }
}
