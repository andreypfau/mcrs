use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::{Items, test_corpus};

pub fn corpus() -> &'static (Blocks, Items) {
    test_corpus()
}

pub fn items() -> &'static Items {
    &corpus().1
}
