use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_testing::{assets_dir, corpus_set, parse_all, reencode};

#[test]
fn every_shipped_processor_list_round_trips() {
    corpus_set().scope(|| {
        let lists = parse_all::<StructureProcessorList>("minecraft/worldgen/processor_list");
        assert_eq!(lists.len(), 40);
        for (path, list) in &lists {
            let text = std::fs::read_to_string(path).unwrap();
            let shipped: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                reencode(list),
                shipped,
                "{} does not re-encode to its own text",
                path.strip_prefix(assets_dir()).unwrap_or(path).display()
            );
        }
    });
}
