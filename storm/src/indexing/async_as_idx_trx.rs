use crate::{BoxFuture, CtxTransaction, Result};

pub trait AsyncAsIdxTrx {
    type Trx<'a>;

    fn async_as_idx_trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>>;
}
