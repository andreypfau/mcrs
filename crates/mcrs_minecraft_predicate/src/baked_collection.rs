use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::component::predicate::CollectionPredicate;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedCollection<P> {
    contains: Box<[P]>,
    count: Box<[BakedCount<P>]>,
    size: MinMaxBounds<i32>,
}

#[derive(Clone, Debug, PartialEq)]
struct BakedCount<P> {
    test: P,
    count: MinMaxBounds<i32>,
}

impl<P> BakedCollection<P> {
    pub(crate) fn bake<T, E>(
        collection: &CollectionPredicate<T>,
        mut bake: impl FnMut(&T) -> Result<P, E>,
    ) -> Result<Self, E> {
        let contains: Box<[P]> = collection
            .contains
            .iter()
            .flatten()
            .map(&mut bake)
            .collect::<Result<_, E>>()?;
        let count: Box<[BakedCount<P>]> = collection
            .count
            .iter()
            .flatten()
            .map(|entry| {
                Ok(BakedCount {
                    test: bake(&entry.test)?,
                    count: entry.count,
                })
            })
            .collect::<Result<_, E>>()?;
        Ok(Self {
            contains,
            count,
            size: collection.size.unwrap_or(MinMaxBounds::ANY),
        })
    }

    pub(crate) fn matches<V>(&self, elements: &[V], test: impl Fn(&P, &V) -> bool) -> bool {
        self.size.matches(saturating_count(elements.len()))
            && self
                .contains
                .iter()
                .all(|predicate| elements.iter().any(|element| test(predicate, element)))
            && self.count.iter().all(|entry| {
                let passing = elements
                    .iter()
                    .filter(|element| test(&entry.test, element))
                    .count();
                entry.count.matches(saturating_count(passing))
            })
    }
}

fn saturating_count(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}
