use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Experiment};

/// The packs that the client applies, in the order of each list.
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x07)]
pub struct ResourcePackStack<'a> {
    /// The client must accept the packs to join.
    pub packs_required: bool,
    /// The behaviour packs.
    pub addon_packs: Vec<ResourcePackStackEntry<'a>>,
    /// The resource packs.
    pub resource_packs: Vec<ResourcePackStackEntry<'a>>,
    /// The game version of the vanilla packs, or `*` for the version of the client.
    pub base_game_version: &'a str,
    /// The experiments of the world.
    #[bedrock(count = u32)]
    pub experiments: Vec<Experiment<'a>>,
    /// The world ever had an experiment on.
    pub experiments_previously_toggled: bool,
    /// True only for an Editor world; a server writes false.
    pub include_editor_packs: bool,
}

/// One pack of [`ResourcePackStack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ResourcePackStackEntry<'a> {
    /// The UUID of the pack in text form.
    pub pack_id: &'a str,
    /// The pack version.
    pub version: &'a str,
    /// The sub-pack name.
    pub sub_pack_name: &'a str,
}
