use mcrs_minecraft_predicate::BlockHost;

pub trait ScoreView<E> {
    fn score(&self, holder: ScoreHolder<'_, E>, objective: &str) -> Option<i32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreHolder<'a, E> {
    Entity(&'a E),
    Name(&'a str),
}

impl<F, E> ScoreView<E> for BlockHost<F, E> {
    fn score(&self, _: ScoreHolder<'_, E>, _: &str) -> Option<i32> {
        None
    }
}
