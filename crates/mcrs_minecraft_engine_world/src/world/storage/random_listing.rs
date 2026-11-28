use mcrs_minecraft_simulation::RandomHit;
use mcrs_voxel::ColumnPos;

#[derive(Clone, Debug, Default)]
pub struct RandomListing {
    pub hits: Vec<RandomHit>,
    pub columns: Vec<ColumnPos>,
}
