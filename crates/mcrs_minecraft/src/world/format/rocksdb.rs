use bevy_ecs::prelude::Resource;
use rocksdb::{ColumnFamilyDescriptor, DB, Options};
use std::path::Path;

#[derive(Resource)]
pub struct RocksDB {
    db: rocksdb::DB,
}

impl RocksDB {
    fn new<P: AsRef<Path>>(path: P) -> Self {
        let chunks_cf = ColumnFamilyDescriptor::new("chunks", rocksdb::Options::default());
        let columns_cf = ColumnFamilyDescriptor::new("columns", rocksdb::Options::default());
        let block_entities_cf =
            ColumnFamilyDescriptor::new("block_entities", rocksdb::Options::default());
        let entities_cf = ColumnFamilyDescriptor::new("entities", rocksdb::Options::default());

        let mut db_opts = Options::default();
        db_opts.create_missing_column_families(true);
        db_opts.create_if_missing(true);

        let db = DB::open_cf_descriptors(
            &db_opts,
            path,
            vec![chunks_cf, columns_cf, block_entities_cf, entities_cf],
        )
        .unwrap();

        Self { db }
    }
}
