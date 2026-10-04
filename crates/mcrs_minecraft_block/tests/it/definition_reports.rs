use mcrs_minecraft_registry::BlockStateId;

use crate::common::{assert_no_mismatches, corpus};

#[test]
fn touching_state_ranges_keep_their_owners() {
    let definitions = corpus();
    let mut by_state: Vec<_> = definitions.blocks().iter().collect();
    by_state.sort_by_key(|block| block.base_state_id);
    assert_eq!(by_state[0].base_state_id, BlockStateId(0));
    let mut mismatches = Vec::new();
    for pair in by_state.windows(2) {
        let (before, after) = (pair[0], pair[1]);
        let last_before = BlockStateId(after.base_state_id.0 - 1);
        if before.base_state_id.0 + before.state_count != after.base_state_id.0
            || definitions.owner(last_before).identifier != before.identifier
            || definitions.owner(after.base_state_id).identifier != after.identifier
        {
            mismatches.push(format!(
                "{} and {} do not touch cleanly",
                before.identifier, after.identifier
            ));
        }
    }
    assert_no_mismatches("neighbouring blocks", mismatches);
}

#[test]
fn a_single_state_block_owns_only_its_state() {
    let definitions = corpus();
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for block in definitions.blocks().iter().filter(|b| b.state_count == 1) {
        seen += 1;
        let state = block.base_state_id;
        let alone = block.default_state_id == state
            && block.owns(state)
            && !block.owns(BlockStateId(state.0.wrapping_add(1)))
            && state
                .0
                .checked_sub(1)
                .is_none_or(|before| !block.owns(BlockStateId(before)))
            && definitions.owner(state).identifier == block.identifier;
        if !alone {
            mismatches.push(format!("{} does not own only its state", block.identifier));
        }
    }
    assert!(seen > 0, "the corpus has no single-state block");
    assert_no_mismatches("single-state blocks", mismatches);
}
