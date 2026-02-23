/// Construct a [`BlockStateId`] from a block's default state with property overrides.
///
/// Two syntaxes are supported:
///
/// ## Declarative (literal values)
///
/// Uses the same property names and value tokens as `define_block!`'s `default:` block.
/// Starts from the block's default state and overrides only the specified properties.
///
/// ```rust,ignore
/// use mcrs_vanilla::block_state;
/// use mcrs_vanilla::block::minecraft::NOTE_BLOCK;
///
/// let id = block_state!(NOTE_BLOCK, { note: 12, powered: true });
/// let id = block_state!(NOTE_BLOCK, { instrument: harp, note: 24 });
/// ```
///
/// ## Typed (expressions + Property handles)
///
/// Uses typed [`Property<T>`](mcrs_core::block_state::Property) handles for type-safe,
/// dynamic construction.
///
/// ```rust,ignore
/// use mcrs_vanilla::block_state;
/// use mcrs_vanilla::block::minecraft::NOTE_BLOCK;
/// use mcrs_vanilla::block::state_properties::*;
///
/// let note: u8 = 12;
/// let id = block_state!(NOTE_BLOCK, NOTE_PROP => note, POWERED_PROP => true);
/// ```
#[macro_export]
macro_rules! block_state {
    // Declarative syntax: block_state!(BLOCK, { field: value, ... })
    ($block:expr, { $($field:ident : $val:tt),* $(,)? }) => {{
        #[allow(unused_mut)]
        let mut _id = (&$block).default_state_id;
        $(
            _id = (&$block).with_property_str(_id, stringify!($field), stringify!($val))
                .expect(concat!("invalid property or value: ", stringify!($field), " = ", stringify!($val)));
        )*
        _id
    }};
    // Typed syntax: block_state!(BLOCK, PROP => val, ...)
    ($block:expr $(, $prop:expr => $val:expr)* $(,)?) => {
        (&$block).state()$(.set(&$prop, $val))*.id()
    };
}

/// Defines a block with optional property-based state layout.
///
/// # Single-state block (no properties)
/// ```rust,ignore
/// define_block! {
///     name: "stone",
///     protocol_id: 1,
///     base_state_id: 1,
///     block_properties: Properties::new().with_strength(1.5)
/// }
/// ```
///
/// # Multi-state block with properties
/// ```rust,ignore
/// define_block! {
///     name: "grass_block",
///     protocol_id: 8,
///     base_state_id: 8,
///     properties: [&state_properties::SNOWY],
///     default: { snowy: false },
///     block_properties: Properties::new().with_strength(0.6)
/// }
/// ```
///
/// # Complex block (note_block with 1150 states)
/// ```rust,ignore
/// define_block! {
///     name: "note_block",
///     protocol_id: 109,
///     base_state_id: 581,
///     properties: [&state_properties::INSTRUMENT, &state_properties::NOTE, &state_properties::POWERED],
///     default: { instrument: harp, note: 0, powered: false },
///     block_properties: Properties::new().with_strength(0.8)
/// }
/// ```
#[macro_export]
macro_rules! define_block {
    // ── With properties ──────────────────────────────────────────────
    (
        name: $name:expr,
        protocol_id: $protocol_id:expr,
        base_state_id: $base_state_id:expr,
        properties: [$($prop:expr),+ $(,)?],
        default: { $($default_prop:ident : $default_value:tt),+ $(,)? },
        block_properties: $block_props:expr $(,)?
    ) => {
        use mcrs_core::block_state::{PropertyDef, PropertyLayout, PropertyIndex};
        use mcrs_protocol::BlockStateId;

        pub const PROPERTIES: behaviour::Properties = $block_props;

        const PROP_COUNT: usize = define_block!(@count $($prop),+);

        static PROP_REFS: [&PropertyDef; PROP_COUNT] = [$($prop),+];

        static STRIDES: [u16; PROP_COUNT] = {
            let mut strides = [0u16; PROP_COUNT];
            let mut i = PROP_COUNT;
            let mut s = 1u16;
            while i > 0 {
                i -= 1;
                strides[i] = s;
                s *= PROP_REFS[i].count() as u16;
            }
            strides
        };

        const TOTAL_STATES: u16 = {
            let mut total = 1u16;
            let mut i = 0;
            while i < PROP_COUNT {
                total *= PROP_REFS[i].count() as u16;
                i += 1;
            }
            total
        };

        const DEFAULT_OFFSET: u16 = {
            let props: [&PropertyDef; PROP_COUNT] = [$($prop),+];
            let defaults: [&str; PROP_COUNT] = [$(stringify!($default_value)),+];

            let mut strides = [0u16; PROP_COUNT];
            let mut i = PROP_COUNT;
            let mut s = 1u16;
            while i > 0 {
                i -= 1;
                strides[i] = s;
                s *= props[i].count() as u16;
            }

            let mut offset = 0u16;
            let mut j = 0;
            while j < PROP_COUNT {
                let idx = match props[j].index_of(defaults[j]) {
                    Some(v) => v,
                    None => panic!("default value not found in property"),
                };
                offset += idx as u16 * strides[j];
                j += 1;
            }
            offset
        };

        static LAYOUT: PropertyLayout = PropertyLayout {
            base_state_id: $base_state_id,
            properties: &PROP_REFS,
            strides: &STRIDES,
            total_states: TOTAL_STATES,
        };

        pub const BLOCK: Block = Block {
            identifier: mcrs_core::rl!($name),
            protocol_id: $protocol_id,
            properties: &PROPERTIES,
            default_state_id: BlockStateId($base_state_id + DEFAULT_OFFSET),
            layout: Some(&LAYOUT),
            state_count: TOTAL_STATES,
        };

        // Property index constants for typed access
        define_block!(@prop_indices 0u8, $($default_prop),+);
    };

    // ── Without properties (single-state) ────────────────────────────
    (
        name: $name:expr,
        protocol_id: $protocol_id:expr,
        base_state_id: $base_state_id:expr,
        block_properties: $block_props:expr $(,)?
    ) => {
        use mcrs_protocol::BlockStateId;

        pub const PROPERTIES: behaviour::Properties = $block_props;

        pub const BLOCK: Block = Block {
            identifier: mcrs_core::rl!($name),
            protocol_id: $protocol_id,
            properties: &PROPERTIES,
            default_state_id: BlockStateId($base_state_id),
            layout: None,
            state_count: 1,
        };
    };

    // ── Helpers ──────────────────────────────────────────────────────

    // Count expressions
    (@count $first:expr $(, $rest:expr)*) => {
        1usize $(+ define_block!(@count_one $rest))*
    };
    (@count_one $x:expr) => { 1usize };

    // Generate property index constants from the default field names
    (@prop_indices $idx:expr, $name:ident) => {
        paste::paste! {
            #[allow(dead_code)]
            pub const [<$name:upper>]: PropertyIndex = PropertyIndex($idx);
        }
    };
    (@prop_indices $idx:expr, $name:ident, $($rest:ident),+) => {
        paste::paste! {
            #[allow(dead_code)]
            pub const [<$name:upper>]: PropertyIndex = PropertyIndex($idx);
        }
        define_block!(@prop_indices $idx + 1, $($rest),+);
    };
}
