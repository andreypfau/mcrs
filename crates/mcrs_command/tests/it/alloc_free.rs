use crate::{Builder, OPEN, executable_literal};
use mcrs_command::{
    BoolArgument, IntegerArgument, NodeId, ParseScratch, StringArgument, build_list,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations() -> usize {
    ALLOCATIONS.with(Cell::get)
}

#[test]
fn a_warm_parse_and_suggestion_pass_allocate_nothing() {
    let mut b = Builder::new();
    let root = executable_literal(&mut b, NodeId::ROOT, "cmd");
    let n = b.argument(root, "n", IntegerArgument::ANY, OPEN).unwrap();
    b.executes(n.node(), 0).unwrap();
    let flag = b.argument(n.node(), "flag", BoolArgument, OPEN).unwrap();
    b.executes(flag.node(), 0).unwrap();
    let word = b
        .argument(root, "word", StringArgument::Phrase, OPEN)
        .unwrap();
    b.executes(word.node(), 0).unwrap();
    let alias = b.literal(NodeId::ROOT, "c", OPEN).unwrap();
    b.redirect(alias, root).unwrap();
    let graph = b.freeze().unwrap();

    let inputs = ["c 12 t", "cmd 12 true", "cmd \"a b\"", "cmd x y", "c c"];
    let mut scratch = ParseScratch::default();
    let mut out = Vec::new();
    let run = |scratch: &mut ParseScratch<_>, out: &mut Vec<_>| {
        for input in inputs {
            let parse = graph.parse(input, 0, &0u8, scratch);
            let _ = parse.check(&graph);
            out.clear();
            graph.suggest(&parse, &0u8, out);
            let list = build_list(input, out);
            std::hint::black_box(list);
        }
    };
    run(&mut scratch, &mut out);
    let before = allocations();
    run(&mut scratch, &mut out);
    assert_eq!(allocations() - before, 0);
}
