use crate::CtxTransaction;

pub trait Transaction {
    #[must_use]
    fn transaction<'a>(&'a self) -> CtxTransaction<'a>;
}
