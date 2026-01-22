use crate::world::block::Block;
use bevy_app::{App, Plugin};
use mcrs_protocol::{BlockStateId, Ident};
use mcrs_registry::Registry;

macro_rules! declare_blocks {
    (
        $(
            $module:ident => $const_name:ident
            $([$($plugin:ident),* $(,)?])?
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
                {
                    let block = &$const_name;
                    let states = block.states;
                    let mut i = 0;
                    while i < states.len() {
                        t[states[i].id.0 as usize] = Some(block);
                        i += 1;
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
    andesite => ANDESITE,
    polished_andesite => POLISHED_ANDESITE,
    grass_block => GRASS_BLOCK,
    dirt => DIRT,
    coarse_dirt => COARSE_DIRT,
    podzol => PODZOL,
    cobblestone => COBBLESTONE,
    oak_planks => OAK_PLANKS,
    spruce_planks => SPRUCE_PLANKS,
    birch_planks => BIRCH_PLANKS,
    jungle_planks => JUNGLE_PLANKS,
    acacia_planks => ACACIA_PLANKS,
    cherry_planks => CHERRY_PLANKS,
    dark_oak_planks => DARK_OAK_PLANKS,
    pale_oak_wood => PALE_OAK_WOOD,
    pale_oak_planks => PALE_OAK_PLANKS,
    mangrove_planks => MANGROVE_PLANKS,
    bamboo_planks => BAMBOO_PLANKS,
    bamboo_mosaic => BAMBOO_MOSAIC,
    oak_sapling => OAK_SAPLING,
    spruce_sapling => SPRUCE_SAPLING,
    birch_sapling => BIRCH_SAPLING,
    jungle_sapling => JUNGLE_SAPLING,
    acacia_sapling => ACACIA_SAPLING,
    cherry_sapling => CHERRY_SAPLING,
    dark_oak_sapling => DARK_OAK_SAPLING,
    pale_oak_sapling => PALE_OAK_SAPLING,
    mangrove_propagule => MANGROVE_PROPAGULE,
    bedrock => BEDROCK,
    note_block => NOTE_BLOCK,
    tnt => TNT [TntBlockPlugin],
}

fn foo() {}

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
