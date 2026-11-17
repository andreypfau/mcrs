use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Experiment};

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x07)]
pub struct ResourcePackStack<'a> {
    pub packs_required: bool,
    pub addon_packs: Vec<ResourcePackStackEntry<'a>>,
    pub resource_packs: Vec<ResourcePackStackEntry<'a>>,
    pub base_game_version: &'a str,
    #[bedrock(count = u32)]
    pub experiments: Vec<Experiment<'a>>,
    pub experiments_previously_toggled: bool,
    pub include_editor_packs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ResourcePackStackEntry<'a> {
    pub pack_id: &'a str,
    pub version: &'a str,
    pub sub_pack_name: &'a str,
}
