/// # Usage Example 1: Mangrove Propagule
/// ```rust,ignore
/// generate_block_states! {
///     base_id: 45,
///     block_name: "mangrove_propagule",
///     state_properties: {
///         age: [0, 1, 2, 3, 4],
///         stage: [0, 1],
///         waterlogged: [true, false],
///         hanging: [true, false]
///     },
///     default: { age: 0, stage: 0, waterlogged: false, hanging: false }
/// }
/// ```
///
/// # Usage Example 2: Custom Door
/// ```rust,ignore
/// generate_block_states! {
///     base_id: 200,
///     block_name: "oak_door",
///     state_properties: {
///         facing: [north, south, east, west],
///         half: [upper, lower],
///         hinge: [left, right],
///         open: [true, false],
///         powered: [true, false]
///     },
///     default: { facing: north, half: lower, hinge: left, open: false, powered: false }
/// }
/// ```
///
/// # Usage Example 3: Single State Block (No Properties)
/// ```rust,ignore
/// generate_block_states! {
///     base_id: 100,
///     block_name: "bedrock",
///     block_properties: Properties::new()
///         .with_map_color(MapColor::STONE)
///         .with_strength(-1.0)
///         .is_air(false)
/// }
/// // Generates: STATE, DEFAULT_STATE, BLOCK, ALL_BLOCK_STATES, PROPERTIES
/// ```

#[macro_export]
macro_rules! generate_block_states {
    // Single state version (no properties)
    (
        base_id: $base_id:expr,
        block_name: $block_name:expr,
        protocol_id: $protocol_id:expr
        $(, block_properties: $properties:expr)?
    ) => {
        paste::paste! {
            // Generate single state constant
            pub const STATE: BlockState = BlockState {
                id: BlockStateId($base_id),
            };

            // Generate the states array with just one state
            pub const ALL_BLOCK_STATES: &[BlockState] = &[STATE];

            // Generate BLOCK constant
            pub const BLOCK: Block = Block {
                identifier: ident!($block_name),
                protocol_id: $protocol_id,
                properties: &PROPERTIES,
                default_state: &DEFAULT_STATE,
                states: ALL_BLOCK_STATES,
            };

            // DEFAULT_STATE is the only state
            pub const DEFAULT_STATE: &BlockState = &STATE;

            // Generate PROPERTIES
            pub const PROPERTIES: Properties = generate_block_states!(@get_props $($properties)?);
        }
    };

    // Main entry point with properties
    (
        base_id: $base_id:expr,
        block_name: $block_name:expr,
        protocol_id: $protocol_id:expr,
        state_properties: {
            $($prop_name:ident: [$($prop_value:tt),+ $(,)?]),+ $(,)?
        },
        default: {
            $($default_prop:ident: $default_value:tt),+ $(,)?
        }
        $(, block_properties: $properties:expr)?
    ) => {
        paste::paste! {
            // Generate all state constants with sequential IDs
            generate_block_states!(@gen_states
                base_id: $base_id,
                id: 0,
                props: [$($prop_name: [$($prop_value),+]),+]
            );

            // Generate ALL_BLOCK_STATES array with collected state names
            pub const ALL_BLOCK_STATES: &[BlockState] = &block_state_idents!(properties: {
                $($prop_name: [$($prop_value),+]),+
            });

            // Generate BLOCK constant
            pub const BLOCK: Block = Block {
                identifier: ident!($block_name),
                protocol_id: $protocol_id,
                properties: &PROPERTIES,
                default_state: &DEFAULT_STATE,
                states: ALL_BLOCK_STATES,
            };

            // Generate DEFAULT_STATE
            pub const DEFAULT_STATE: &BlockState = &[<
                $($default_prop:upper _ $default_value:upper _)+
                STATE
            >];

            // Generate PROPERTIES
            pub const PROPERTIES: Properties = generate_block_states!(@get_props $($properties)?);
        }

    };

    // Generate all state constants
    // Strategy: iterate through first property, then recursively handle rest
    (@gen_states
        base_id: $base_id:expr,
        id: $id:expr,
        props: [$first_prop:ident: [$($first_val:tt),+] $(, $rest_prop:ident: [$($rest_val:tt),+])*]
    ) => {
        generate_block_states!(@gen_for_first_prop
            base_id: $base_id,
            id: $id,
            first_prop: $first_prop,
            first_values: [$($first_val),+],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: generate_block_states!(@total_combinations [$($rest_prop: [$($rest_val),+]),*])
        );
    };

    // Iterate through each value of the first property
    (@gen_for_first_prop
        base_id: $base_id:expr,
        id: $id:expr,
        first_prop: $first_prop:ident,
        first_values: [$first_val:tt $(, $rest_vals:tt)*],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        // Generate states for this value combined with all rest combinations
        generate_block_states!(@gen_with_prefix
            base_id: $base_id,
            id: $id,
            prefix: [$first_prop: $first_val],
            props: [$($rest_prop: [$($rest_val),+]),*]
        );

        // Continue with remaining values (recursive call)
        generate_block_states!(@gen_for_first_prop_continue
            base_id: $base_id,
            id: $id + $rest_count,
            first_prop: $first_prop,
            first_values: [$($rest_vals),*],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: $rest_count
        );
    };

    // Helper to continue iteration or stop
    (@gen_for_first_prop_continue
        base_id: $base_id:expr,
        id: $id:expr,
        first_prop: $first_prop:ident,
        first_values: [$($vals:tt),+],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        generate_block_states!(@gen_for_first_prop
            base_id: $base_id,
            id: $id,
            first_prop: $first_prop,
            first_values: [$($vals),+],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: $rest_count
        );
    };

    // Base case: no more values to process
    (@gen_for_first_prop_continue
        base_id: $base_id:expr,
        id: $id:expr,
        first_prop: $first_prop:ident,
        first_values: [],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        // Done - no more values
    };

    // Generate states with a prefix (already assigned properties)
    (@gen_with_prefix
        base_id: $base_id:expr,
        id: $id:expr,
        prefix: [$($prefix_prop:ident: $prefix_val:tt),+],
        props: [$next_prop:ident: [$($next_val:tt),+] $(, $rest_prop:ident: [$($rest_val:tt),+])*]
    ) => {
        generate_block_states!(@gen_for_next_prop
            base_id: $base_id,
            id: $id,
            prefix: [$($prefix_prop: $prefix_val),+],
            next_prop: $next_prop,
            next_values: [$($next_val),+],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: generate_block_states!(@total_combinations [$($rest_prop: [$($rest_val),+]),*])
        );
    };

    // Base case: no more properties, generate the constant
    (@gen_with_prefix
        base_id: $base_id:expr,
        id: $id:expr,
        prefix: [$($prefix_prop:ident: $prefix_val:tt),+],
        props: []
    ) => {
        paste::paste! {
            pub const [<$($prefix_prop:upper _ $prefix_val:upper _)+STATE>]: BlockState = BlockState {
                id: BlockStateId($base_id + $id),
            };
        }
    };

    // Iterate through next property values
    (@gen_for_next_prop
        base_id: $base_id:expr,
        id: $id:expr,
        prefix: [$($prefix_prop:ident: $prefix_val:tt),+],
        next_prop: $next_prop:ident,
        next_values: [$next_val:tt $(, $rest_vals:tt)*],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        generate_block_states!(@gen_with_prefix
            base_id: $base_id,
            id: $id,
            prefix: [$($prefix_prop: $prefix_val,)+ $next_prop: $next_val],
            props: [$($rest_prop: [$($rest_val),+]),*]
        );

        // Continue with remaining values (recursive call)
        generate_block_states!(@gen_for_next_prop_continue
            base_id: $base_id,
            id: $id + $rest_count,
            prefix: [$($prefix_prop: $prefix_val),+],
            next_prop: $next_prop,
            next_values: [$($rest_vals),*],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: $rest_count
        );
    };

    // Helper to continue iteration or stop
    (@gen_for_next_prop_continue
        base_id: $base_id:expr,
        id: $id:expr,
        prefix: [$($prefix_prop:ident: $prefix_val:tt),+],
        next_prop: $next_prop:ident,
        next_values: [$($vals:tt),+],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        generate_block_states!(@gen_for_next_prop
            base_id: $base_id,
            id: $id,
            prefix: [$($prefix_prop: $prefix_val),+],
            next_prop: $next_prop,
            next_values: [$($vals),+],
            rest_props: [$($rest_prop: [$($rest_val),+]),*],
            rest_count: $rest_count
        );
    };

    // Base case: no more values to process
    (@gen_for_next_prop_continue
        base_id: $base_id:expr,
        id: $id:expr,
        prefix: [$($prefix_prop:ident: $prefix_val:tt),+],
        next_prop: $next_prop:ident,
        next_values: [],
        rest_props: [$($rest_prop:ident: [$($rest_val:tt),+]),*],
        rest_count: $rest_count:expr
    ) => {
        // Done - no more values
    };

        // finish: add _STATE exactly once
    (@props [ $($acc:ident),* ] ; ) => {
        paste! { [ $([<$acc _ STATE>]),* ] }
    };

    (@props [ $($acc:ident),* ] ;
        $p:ident : [$($v:tt),+ $(,)?]
        $(, $($rest:tt)*)?
    ) => {
        properties_array!(
            @cross
            [ $($acc),* ]
            $p
            [ $($v),+ ]
            [ ]
            ;
            $($($rest)*)?
        )
    };

    (@cross [ ] $p:ident [ $($v:tt),+ ] [ $($out:ident,)* ] ; $($rest:tt)*) => {
        properties_array!(@props [ $($out),* ] ; $($rest)*)
    };

    (@cross [ $head:ident $(, $tail:ident)* ] $p:ident [ $($v:tt),+ ] [ $($out:ident,)* ] ; $($rest:tt)*) => {
        paste::paste! {
            properties_array!(
                @cross
                [ $($tail),* ]
                $p
                [ $($v),+ ]
                [
                    $($out,)*
                    $([<$head _ $p:upper _ $v:upper>],)*
                ]
                ;
                $($rest)*
            )
        }
    };

    // Build the states array by recursively collecting all state names
    // Helper: Calculate total combinations for remaining properties
    (@total_combinations []) => { 1 };
    (@total_combinations [$_prop:ident: [$($vals:tt),+] $(, $rest_prop:ident: [$($rest_vals:tt),+])*]) => {
        generate_block_states!(@count_vals [$($vals),+]) *
        generate_block_states!(@total_combinations [$($rest_prop: [$($rest_vals),+]),*])
    };

    // Helper: Count values
    (@count_vals [$_:tt]) => { 1 };
    (@count_vals [$_:tt, $($rest:tt),+]) => {
        1 + generate_block_states!(@count_vals [$($rest),+])
    };

    // Helper: Get properties expression
    (@get_props) => { Properties::new() };
    (@get_props $props:expr) => { $props };
}

#[macro_export]
macro_rules! block_state_idents {
    (properties: {
        $first_p:ident : [$($first_v:tt),+ $(,)?]
        $(, $($rest:tt)*)?
    }) => {
        paste::paste! {
            block_state_idents!(
                @props
                [ $([<$first_p:upper _ $first_v:upper>]),+ ]
                ;
                $($($rest)*)?
            )
        }
    };

    // finish: add _STATE exactly once
    (@props [ $($acc:ident),* ] ; ) => {
        paste::paste! { [ $([<$acc _ STATE>]),* ] }
    };

    (@props [ $($acc:ident),* ] ;
        $p:ident : [$($v:tt),+ $(,)?]
        $(, $($rest:tt)*)?
    ) => {
        block_state_idents!(
            @cross
            [ $($acc),* ]
            $p
            [ $($v),+ ]
            [ ]
            ;
            $($($rest)*)?
        )
    };

    (@cross [ ] $p:ident [ $($v:tt),+ ] [ $($out:ident,)* ] ; $($rest:tt)*) => {
        block_state_idents!(@props [ $($out),* ] ; $($rest)*)
    };

    (@cross [ $head:ident $(, $tail:ident)* ] $p:ident [ $($v:tt),+ ] [ $($out:ident,)* ] ; $($rest:tt)*) => {
        paste::paste! {
            block_state_idents!(
                @cross
                [ $($tail),* ]
                $p
                [ $($v),+ ]
                [
                    $($out,)*
                    $([<$head _ $p:upper _ $v:upper>],)*
                ]
                ;
                $($rest)*
            )
        }
    };
}
