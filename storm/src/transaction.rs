use crate::CtxTransaction;

pub trait Transaction {
    #[must_use]
    fn transaction(&self) -> CtxTransaction<'_>;
}
