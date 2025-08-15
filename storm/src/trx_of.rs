use crate::{BoxFuture, CtxTransaction, LogOf, Result};

pub trait TrxOf: LogOf {
    type Trx<'a>
    where
        Self: 'a;

    fn trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>>;
}
