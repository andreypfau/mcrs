use mcrs_minecraft_light_engine::epoch::{PublishedLight, SectionLight};
use mcrs_voxel::ColumnPos;

pub(crate) enum LightOutput {
    Relit(Vec<PublishedLight>),
    Settled(Vec<ColumnPos>),
    Lit(Vec<LitColumn>),
}

pub(crate) struct LitColumn {
    pub(crate) column: ColumnPos,
    pub(crate) sections: Vec<SectionLight>,
}
