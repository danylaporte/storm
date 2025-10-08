use crate::CtxTransaction;
use uuid::Uuid;

pub trait Transaction {
    #[must_use]
    fn transaction<U>(&self, user_id: U) -> CtxTransaction<'_>
    where
        U: Into<Uuid>;
}
