use crate::{mem, provider, ApplyLog, Commit, Entity, Get, Insert, Remove, Result};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

pub struct Connected<T, P> {
    pub ctx: T,
    pub provider: P,
}

impl<T, P> ApplyLog for Connected<T, P>
where
    T: ApplyLog + Send + Sync,
{
    type Log = T::Log;

    fn apply_log(&mut self, log: Self::Log) {
        self.ctx.apply_log(log)
    }
}

#[async_trait]
impl<T, P> Commit for Connected<T, P>
where
    T: mem::Commit + Send + Sync,
    P: provider::Commit + Send + Sync,
{
    type Log = T::Log;

    async fn commit(self) -> Result<Self::Log> {
        self.provider.commit().await?;
        Ok(self.ctx.commit())
    }
}

impl<T, P> Deref for Connected<T, P> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl<T, P> DerefMut for Connected<T, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl<E, T, P> Get<E> for Connected<T, P>
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.ctx.get(k)
    }
}

#[async_trait]
impl<E, T, P> Insert<E> for Connected<T, P>
where
    E: Entity + Send + Sync + 'static,
    E::Key: Send + Sync,
    P: provider::Upsert<E> + Send + Sync,
    T: mem::Insert<E> + Send,
{
    async fn insert(&mut self, k: E::Key, v: E) -> Result<()> {
        self.provider.upsert(&k, &v).await?;
        self.ctx.insert(k, v);
        Ok(())
    }
}

#[async_trait]
impl<E, T, P> Remove<E> for Connected<T, P>
where
    E: Entity + Send + Sync + 'static,
    E::Key: Send + Sync,
    P: provider::Delete<E> + Send + Sync,
    T: mem::Remove<E> + Send,
{
    async fn remove(&mut self, k: E::Key) -> Result<()> {
        self.provider.delete(&k).await?;
        self.ctx.remove(k);
        Ok(())
    }
}

impl<'a, T, P> mem::Transaction<'a> for Connected<T, P>
where
    T: mem::Transaction<'a>,
{
    type Transaction = T::Transaction;

    fn transaction(&'a self) -> Self::Transaction {
        self.ctx.transaction()
    }
}

#[async_trait]
impl<'a, T, P> provider::Transaction<'a> for Connected<T, P>
where
    T: Send + Sync,
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
    type Transaction = Connected<
        <T as mem::Transaction<'a>>::Transaction,
        <T as provider::Transaction<'a>>::Transaction,
    >;

    async fn transaction(&'a self) -> Result<Self::Transaction> {
        Ok(Connected {
            provider: provider::Transaction::transaction(&**self).await?,
            ctx: mem::Transaction::transaction(&**self),
        })
    }
}