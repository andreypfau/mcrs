use crate::world::block::Block;
use bevy_app::{App, Plugin};
use mcrs_protocol::{BlockStateId, Ident};
use mcrs_registry::Registry;

macro_rules! declare_blocks {
    (
        $(
            $module:ident => $const_name:ident
            $([$($plugin:ident),* $(,)?])?
            $(: [$($state:ident),* $(,)?])?
        ),* $(,)?
    ) => {
        // Declare modules
        $(
            pub mod $module;
        )*

        // Re-export block constants
        $(
            pub use $module::BLOCK as $const_name;
        )*

        pub struct MinecraftBlockPlugin;

        impl Plugin for MinecraftBlockPlugin {
            fn build(&self, app: &mut App) {
                // Add block-specific plugins
                $(
                    $($(
                        app.add_plugins($module::$plugin);
                    )*)?
                )*

                // Register blocks in registry
                let mut registry = Registry::<&'static Block>::default();
                $(
                    registry.insert($const_name.identifier, &$const_name);
                )*

                app.insert_resource(registry);
            }
        }

        const STATE_TABLE_LEN: usize = 1 << 16;

        static STATE_TO_BLOCK: [Option<&'static Block>; STATE_TABLE_LEN] = {
            let mut t: [Option<&'static Block>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
            $(
                // If explicit states provided, use them
                $($(
                    t[$module::$state.id.0 as usize] = Some(&$const_name);
                )*)?

                // Otherwise use DEFAULT_STATE (this gets overridden if states were specified)
                #[allow(unreachable_code)]
                {
                    $($(let _ = $module::$state;)*)?  // Consume the pattern if it exists

                    // Only register DEFAULT_STATE if no explicit states
                    if false $(|| { $(let _ = $module::$state;)* true })? {
                        // Skip - explicit states handled above
                    } else {
                        t[$module::DEFAULT_STATE.id.0 as usize] = Some(&$const_name);
                    }
                }
            )*
            t
        };
    };
}

declare_blocks! {
    air => AIR,
    stone => STONE,
    granite => GRANITE,
    polished_granite => POLISHED_GRANITE,
    diorite => DIORITE,
    polished_diorite => POLISHED_DIORITE,

    grass_block => GRASS_BLOCK,
    dirt => DIRT,
    bedrock => BEDROCK,
    note_block => NOTE_BLOCK,
    tnt => TNT [TntBlockPlugin]: [UNSTABLE_STATE, DEFAULT_STATE],
}

impl TryFrom<BlockStateId> for &'static Block {
    type Error = ();

    #[inline]
    fn try_from(v: BlockStateId) -> Result<Self, Self::Error> {
        STATE_TO_BLOCK.get(v.0 as usize).and_then(|x| *x).ok_or(())
    }
}

impl TryFrom<Ident<String>> for &'static Block {
    type Error = ();

    fn try_from(value: Ident<String>) -> Result<Self, Self::Error> {
        STATE_TO_BLOCK
            .iter()
            .find(|block_opt| {
                if let Some(block) = block_opt {
                    block.identifier.as_str() == value.as_str()
                } else {
                    false
                }
            })
            .and_then(|block_opt| *block_opt)
            .ok_or(())
    }
}

impl AsRef<Block> for BlockStateId {
    #[inline]
    fn as_ref(&self) -> &Block {
        STATE_TO_BLOCK[self.0 as usize].expect(&format!("Invalid block state id: {}", self.0))
    }
}
