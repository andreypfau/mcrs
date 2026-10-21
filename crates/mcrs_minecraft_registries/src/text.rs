use mcrs_minecraft_text::{Style, Text, TextMap, TextTypes};

use crate::{Namer, Resolve, ResolveError, Resolver};

struct Resolving<'a, C>(&'a C);

struct Naming<'a, C>(&'a C);

impl<I, C> TextMap<I::Named, I> for Resolving<'_, C>
where
    I: TextTypes + Resolve<Named: Clone>,
    I::Named: TextTypes<
            EntityKey = <I::EntityKey as Resolve>::Named,
            Dialog = <I::Dialog as Resolve>::Named,
        >,
    I::EntityKey: Resolve<Named: Clone>,
    I::Dialog: Resolve<Named: Clone>,
    C: Resolver,
{
    type Error = ResolveError;

    fn item(&self, item: &I::Named) -> Result<I, ResolveError> {
        I::resolve(item.clone(), self.0)
    }

    fn entity(
        &self,
        entity: &<I::Named as TextTypes>::EntityKey,
    ) -> Result<I::EntityKey, ResolveError> {
        I::EntityKey::resolve(entity.clone(), self.0)
    }

    fn dialog(&self, dialog: &<I::Named as TextTypes>::Dialog) -> Result<I::Dialog, ResolveError> {
        I::Dialog::resolve(dialog.clone(), self.0)
    }

    fn at(error: ResolveError, field: &'static str) -> ResolveError {
        error.at(field)
    }

    fn at_index(error: ResolveError, index: usize) -> ResolveError {
        error.at_index(index)
    }
}

impl<I, C> TextMap<I, I::Named> for Naming<'_, C>
where
    I: TextTypes + Resolve,
    I::Named: TextTypes<
            EntityKey = <I::EntityKey as Resolve>::Named,
            Dialog = <I::Dialog as Resolve>::Named,
        >,
    I::EntityKey: Resolve,
    I::Dialog: Resolve,
    C: Namer,
{
    type Error = ResolveError;

    fn item(&self, item: &I) -> Result<I::Named, ResolveError> {
        item.name(self.0)
    }

    fn entity(
        &self,
        entity: &I::EntityKey,
    ) -> Result<<I::Named as TextTypes>::EntityKey, ResolveError> {
        entity.name(self.0)
    }

    fn dialog(&self, dialog: &I::Dialog) -> Result<<I::Named as TextTypes>::Dialog, ResolveError> {
        dialog.name(self.0)
    }

    fn at(error: ResolveError, field: &'static str) -> ResolveError {
        error.at(field)
    }

    fn at_index(error: ResolveError, index: usize) -> ResolveError {
        error.at_index(index)
    }
}

impl<I> Resolve for Text<I>
where
    I: TextTypes + Resolve<Named: Clone>,
    I::Named: TextTypes<
            EntityKey = <I::EntityKey as Resolve>::Named,
            Dialog = <I::Dialog as Resolve>::Named,
        >,
    I::EntityKey: Resolve<Named: Clone>,
    I::Dialog: Resolve<Named: Clone>,
{
    type Named = Text<I::Named>;

    fn resolve<C: Resolver>(named: Text<I::Named>, ctx: &C) -> Result<Self, ResolveError> {
        named.map(&Resolving(ctx))
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Text<I::Named>, ResolveError> {
        self.map(&Naming(names))
    }
}

impl<I> Resolve for Style<I>
where
    I: TextTypes + Resolve<Named: Clone>,
    I::Named: TextTypes<
            EntityKey = <I::EntityKey as Resolve>::Named,
            Dialog = <I::Dialog as Resolve>::Named,
        >,
    I::EntityKey: Resolve<Named: Clone>,
    I::Dialog: Resolve<Named: Clone>,
{
    type Named = Style<I::Named>;

    fn resolve<C: Resolver>(named: Style<I::Named>, ctx: &C) -> Result<Self, ResolveError> {
        named.map(&Resolving(ctx))
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Style<I::Named>, ResolveError> {
        self.map(&Naming(names))
    }
}
