use std::fs;
use std::path::Path;

use anyhow::Context;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::build_block_definitions;
use mcrs_voxel::BlockStateId;

/// The names of the block state ids that the play packets carry, from the block corpus.
pub(crate) struct Names {
    blocks: BlockDefinitions,
}

impl Names {
    pub(crate) fn load(root: &Path) -> anyhow::Result<Self> {
        let directory = root.join("assets/mcrs/block_definition");
        let mut files = Vec::new();
        for entry in fs::read_dir(&directory).with_context(|| directory.display().to_string())? {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            files.push((path.display().to_string(), fs::read(&path)?));
        }
        let names: Vec<&str> = Block::ALL
            .iter()
            .map(|block| block.as_static_str())
            .collect();
        let (blocks, _) = build_block_definitions(&names, files)?;
        Ok(Self { blocks })
    }

    /// The state as `name[property=value,...]`, or `#<id>` for an id the table lacks.
    pub(crate) fn block_state(&self, number: u16) -> String {
        let Some(id) = BlockStateId::checked(number, self.blocks.state_count()) else {
            return format!("#{number}");
        };
        let Some(block) = self.blocks.owner(id) else {
            return format!("#{number}");
        };
        let values: Vec<String> = block
            .properties
            .0
            .iter()
            .filter_map(|property| {
                let value = block.value_of(id, &property.name)?;
                Some(format!("{}={}", property.name, value.to_text()))
            })
            .collect();
        if values.is_empty() {
            block.identifier.to_string()
        } else {
            format!("{}[{}]", block.identifier, values.join(","))
        }
    }
}
