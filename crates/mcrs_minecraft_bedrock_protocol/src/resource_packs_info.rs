use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x06)]
pub struct ResourcePacksInfo<'a> {
    pub packs_required: bool,
    pub has_addon_packs: bool,
    pub has_scripts: bool,
    #[bedrock(count = u16)]
    pub resource_packs: Vec<ResourcePackInfoEntry<'a>>,
    pub cdn_urls: Vec<PackCdnUrl<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ResourcePackInfoEntry<'a> {
    pub pack_id: &'a str,
    pub version: &'a str,
    pub size: u64,
    pub content_key: &'a str,
    pub sub_pack_name: &'a str,
    pub content_identity: &'a str,
    pub has_scripts: bool,
    pub is_addon_pack: bool,
    pub ray_tracing_capable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct PackCdnUrl<'a> {
    pub pack_reference: &'a str,
    pub url: &'a str,
}
