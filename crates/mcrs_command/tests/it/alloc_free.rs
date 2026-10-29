use crate::{Builder, OPEN, executable_literal};
use mcrs_command::{
    BoolArgument, Chain, Executor, Failure, ForkMode, IntegerArgument, Limits, NodeId,
    ParseScratch, SourceLists, StringArgument, SuggestionList, execute,
};
use mcrs_test_alloc::{CountingAllocator, allocations};

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[derive(Default)]
struct Count {
    modified: usize,
    ran: usize,
}

impl Executor<u32, u32> for Count {
    type Error = ();

    fn modify(&mut self, _: usize, _: &u32, source: &u32, out: &mut Vec<u32>) -> Result<(), ()> {
        self.modified += 1;
        out.extend([source * 2, source * 2 + 1]);
        Ok(())
    }

    fn run(&mut self, _: usize, _: &u32, source: &u32) -> Result<i32, ()> {
        self.ran += 1;
        Ok(*source as i32)
    }

    fn report(&mut self, _: Failure<()>) {}
}

#[test]
fn a_warm_parse_run_and_suggestion_pass_allocate_nothing() {
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
    let fork = b.literal(NodeId::ROOT, "f", OPEN).unwrap();
    b.redirect(fork, NodeId::ROOT).unwrap();
    b.modifies(fork, 1, ForkMode::Fork).unwrap();
    let graph = b.freeze().unwrap();

    let inputs = [
        "c 12 t",
        "cmd 12 true",
        "cmd \"a b\"",
        "cmd x y",
        "c c",
        "f f c 7",
        "cm",
        "cmd 12 ",
    ];
    let mut scratch = ParseScratch::default();
    let mut out = Vec::new();
    let mut chain = Chain::default();
    let mut lists = SourceLists::default();
    let mut count = Count::default();
    let mut values = 0;
    let mut run = |scratch: &mut ParseScratch<_>,
                   out: &mut Vec<_>,
                   chain: &mut Chain<u32>,
                   lists: &mut SourceLists<u32>,
                   suggested: &mut usize| {
        for input in inputs {
            let parse = graph.parse(input, 0, &0u8, scratch);
            let checked = std::hint::black_box(parse.check(&graph));
            if checked.is_ok() {
                chain.copy_from(&graph, &parse);
                let stages = chain.stages();
                std::hint::black_box(execute(stages, 1, Limits::default(), &mut count, lists));
                if let Some(span) = chain.span(stages.len() - 1, n.node()) {
                    values += 1;
                    std::hint::black_box(graph.value(input, span, n, &0u8));
                }
            }
            out.clear();
            graph.suggest(&parse, &0u8, out);
            *suggested += out.len();
            let list = SuggestionList::build(input, out);
            std::hint::black_box(list);
        }
    };
    let mut warm_suggested = 0;
    let mut measured_suggested = 0;
    run(
        &mut scratch,
        &mut out,
        &mut chain,
        &mut lists,
        &mut warm_suggested,
    );
    let before = allocations();
    run(
        &mut scratch,
        &mut out,
        &mut chain,
        &mut lists,
        &mut measured_suggested,
    );
    assert_eq!(allocations() - before, 0);
    assert!(warm_suggested > 0, "the passes build suggestions");
    assert_eq!(measured_suggested, warm_suggested);
    assert_eq!((count.modified, count.ran, values), (2 * 3, 2 * 7, 2 * 2));
}
