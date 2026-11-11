use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

/// The packs that the server offers after the login.
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x06)]
pub struct ResourcePacksInfo<'a> {
    /// The client must accept the packs to join.
    pub packs_required: bool,
    /// The server has add-on packs.
    pub has_addon_packs: bool,
    /// The server has scripts.
    pub has_scripts: bool,
    /// The resource packs.
    #[bedrock(count = u16)]
    pub resource_packs: Vec<ResourcePackInfoEntry<'a>>,
    /// The addresses where the client can download packs.
    pub cdn_urls: Vec<PackCdnUrl<'a>>,
}

/// One resource pack of [`ResourcePacksInfo`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct ResourcePackInfoEntry<'a> {
    /// The UUID of the pack in text form.
    pub pack_id: &'a str,
    /// The pack version, for example `1.0.0`.
    pub version: &'a str,
    /// The size of the pack archive in bytes.
    pub size: u64,
    /// The key of an encrypted pack, or empty.
    pub content_key: &'a str,
    /// The sub-pack name.
    pub sub_pack_name: &'a str,
    /// The content identity.
    pub content_identity: &'a str,
    /// The pack has scripts.
    pub has_scripts: bool,
    /// The pack is part of an add-on.
    pub is_addon_pack: bool,
    /// The pack supports ray tracing.
    pub ray_tracing_capable: bool,
}

/// A download address of one pack in [`ResourcePacksInfo`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct PackCdnUrl<'a> {
    /// The pack reference: the pack id, `_` and the pack version.
    pub pack_reference: &'a str,
    /// The address where the client can download the pack.
    pub url: &'a str,
}
