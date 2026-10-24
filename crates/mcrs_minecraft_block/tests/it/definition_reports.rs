use mcrs_minecraft_core::BlockStateId;

use crate::common::{assert_no_mismatches, corpus};

#[test]
fn touching_state_ranges_keep_their_owners() {
    let definitions = corpus();
    let mut by_state: Vec<_> = definitions.blocks().iter().collect();
    by_state.sort_by_key(|block| block.base_state_id);
    assert_eq!(by_state[0].base_state_id, BlockStateId::AIR);
    assert_eq!(by_state[0].identifier.as_str(), "minecraft:air");
    let mut mismatches = Vec::new();
    for pair in by_state.windows(2) {
        let (before, after) = (pair[0], pair[1]);
        let last_before = before.state_at(before.state_count - 1);
        if before.state_at(before.state_count).is_some()
            || last_before
                .and_then(|state| definitions.owner(state))
                .map(|owner| &owner.identifier)
                != Some(&before.identifier)
            || definitions
                .owner(after.base_state_id)
                .map(|owner| &owner.identifier)
                != Some(&after.identifier)
            || last_before.map(BlockStateId::number).map(|n| n + 1)
                != Some(after.base_state_id.number())
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
    let mut by_state: Vec<_> = definitions.blocks().iter().collect();
    by_state.sort_by_key(|block| block.base_state_id);
    let mut seen = 0;
    let mut mismatches = Vec::new();
    for (at, block) in by_state.iter().enumerate() {
        if block.state_count != 1 {
            continue;
        }
        seen += 1;
        let state = block.base_state_id;
        let before = at
            .checked_sub(1)
            .and_then(|at| by_state[at].state_at(by_state[at].state_count - 1));
        let after = by_state.get(at + 1).map(|next| next.base_state_id);
        let alone = block.default_state_id == state
            && block.owns(state)
            && block.state_at(1).is_none()
            && !after.is_some_and(|after| block.owns(after))
            && !before.is_some_and(|before| block.owns(before))
            && definitions.owner(state).map(|owner| &owner.identifier) == Some(&block.identifier);
        if !alone {
            mismatches.push(format!("{} does not own only its state", block.identifier));
        }
    }
    assert!(seen > 0, "the corpus has no single-state block");
    assert_no_mismatches("single-state blocks", mismatches);
}
