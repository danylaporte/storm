use crate::{mem, provider, ApplyLog, Commit, Result};
use async_trait::async_trait;

pub struct CtxProvider<C, P> {
    pub ctx: C,
    pub provider: P,
}

impl<C, P> ApplyLog for CtxProvider<C, P>
where
    C: ApplyLog + Send + Sync,
{
    type Log = C::Log;

    fn apply_log(&mut self, log: Self::Log) {
        self.ctx.apply_log(log)
    }
}

impl<C, P, U> AsRef<U> for CtxProvider<C, P>
where
    C: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.ctx.as_ref()
    }
}

impl<'a, C, P> mem::Transaction<'a> for CtxProvider<C, P>
where
    C: mem::Transaction<'a>,
{
    type Transaction = C::Transaction;

    fn transaction(&'a self) -> Self::Transaction {
        self.ctx.transaction()
    }
}

#[async_trait]
impl<'a, C, P> provider::Transaction<'a> for CtxProvider<C, P>
where
    C: Send + Sync,
    P: provider::Transaction<'a> + Send + Sync,
{
    type Transaction = P::Transaction;

    async fn transaction(&'a self) -> Result<Self::Transaction> {
        self.provider.transaction().await
    }
}

#[cfg(feature = "async-cell-lock")]
#[async_trait]
impl<'a, T> crate::Transaction<'a> for async_cell_lock::QueueRwLockQueueGuard<'a, T>
where
    T: mem::Transaction<'a> + provider::Transaction<'a> + Send + Sync,
{
    type Transaction = TrxProvider<
        <T as mem::Transaction<'a>>::Transaction,
        <T as provider::Transaction<'a>>::Transaction,
    >;

    async fn transaction(&'a self) -> Result<Self::Transaction> {
        Ok(TrxProvider {
            provider: provider::Transaction::transaction(&**self).await?,
            trx: mem::Transaction::transaction(&**self),
        })
    }
}

#[must_use]
pub struct TrxProvider<T, P> {
    pub provider: P,
    pub trx: T,
}

impl<T, P, U> AsMut<U> for TrxProvider<T, P>
where
    T: AsMut<U>,
{
    fn as_mut(&mut self) -> &mut U {
        self.trx.as_mut()
    }
}

impl<T, P, U> AsRef<U> for TrxProvider<T, P>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.trx.as_ref()
    }
}

#[async_trait]
impl<T, P> Commit for TrxProvider<T, P>
where
    T: mem::Commit + Send + Sync,
    P: provider::Commit + Send + Sync,
{
    type Log = T::Log;

    async fn commit(self) -> Result<Self::Log> {
        self.provider.commit().await?;
        Ok(self.trx.commit())
    }
}
