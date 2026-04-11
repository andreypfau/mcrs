use crate::world::block::Block;

pub type BlockTagSet = &'static [&'static BlockTag];

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub enum BlockTag {
    Tag(&'static Block),
    TagSet(BlockTagSet),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DynamicBlockTagSet {
    pub ident: mcrs_protocol::Ident<String>,
}

impl DynamicBlockTagSet {
    pub fn new(ident: mcrs_protocol::Ident<String>) -> Self {
        Self { ident }
    }

    pub fn from_static(ident: mcrs_protocol::Ident<&'static str>) -> Self {
        Self {
            ident: ident.to_string_ident(),
        }
    }

    pub fn ident(&self) -> &mcrs_protocol::Ident<String> {
        &self.ident
    }
}

pub trait BlockTagSetExt {
    fn contains_block(&self, block: &Block) -> bool;
}

impl BlockTagSetExt for BlockTag {
    fn contains_block(&self, block: &Block) -> bool {
        match self {
            BlockTag::Tag(b) => b == &block,
            BlockTag::TagSet(tag_set) => tag_set.contains_block(block),
        }
    }
}

impl BlockTagSetExt for BlockTagSet {
    fn contains_block(&self, block: &Block) -> bool {
        for tag in *self {
            if tag.contains_block(block) {
                return true;
            }
        }
        false
    }
}
