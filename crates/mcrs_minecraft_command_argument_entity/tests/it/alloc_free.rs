use crate::entity::{Mob, world};
use crate::{OP, read};
use mcrs_command::{Suggestion, SuggestionBuilder};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_test_alloc::{CountingAllocator, allocations};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn a_warm_read_search_and_suggestion_of_a_selector_allocates_nothing() {
    let selectors = [
        "@e[type=#minecraft:skeletons,nbt={Tags:[\"a\"],Health:20f},distance=..5,sort=nearest]",
        "@a[gamemode=!creative,limit=2]",
        "@e[type=!cow,type=pig]",
    ];
    let inputs = ["@e[type=!cow,type=armadi", "@a[gamemode=", "@e["];
    assert!(read(&EntityArgument::ENTITIES, selectors[0], &OP).0.is_ok());
    assert!(read(&EntityArgument::ENTITIES, selectors[1], &OP).0.is_ok());
    assert_eq!(
        read(&EntityArgument::ENTITIES, selectors[2], &OP)
            .0
            .map(drop)
            .map_err(|e| e.position),
        Err(18)
    );

    let world = world();
    let mut found: Vec<&Mob> = Vec::new();
    let mut out: Vec<Suggestion<'_, &'static str>> = Vec::new();
    let mut run = |trace: &mut [usize; 12]| {
        for (index, input) in selectors.into_iter().enumerate() {
            let (selector, cursor) = read(&EntityArgument::ENTITIES, input, &OP);
            found.clear();
            let ok = selector.is_ok();
            if let Ok(selector) = selector {
                let result =
                    selector.select(&OP, world.first(), world.iter(), &mut |_| 0, &mut found);
                assert_eq!(std::hint::black_box(result), Ok(()));
            }
            trace[index * 3..][..3].copy_from_slice(&[usize::from(ok), cursor, found.len()]);
        }
        for (index, input) in inputs.into_iter().enumerate() {
            out.clear();
            EntityArgument::ENTITIES.suggest(&OP, &mut SuggestionBuilder::new(input, 0, &mut out));
            trace[9 + index] = out.len();
        }
    };
    let mut warm = [0; 12];
    let mut measured = [0; 12];
    run(&mut warm);
    let before = allocations();
    run(&mut measured);
    assert_eq!(allocations() - before, 0);
    assert_eq!(
        measured, warm,
        "the measured run does the work of the warm run"
    );
    let selected: usize = [warm[2], warm[5], warm[8]].iter().sum();
    assert!(selected > 0 && warm[9..].iter().any(|&count| count > 0));
}
