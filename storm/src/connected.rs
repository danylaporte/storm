use crate::{
    mem,
    provider::{self, Delete, ProviderContainer, TransactionProvider, Upsert},
    ApplyLog, Entity, Get, Insert, Remove, Result,
};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

pub struct Connected<T> {
    pub ctx: T,
    pub provider: ProviderContainer,
}

impl<T> ApplyLog for Connected<T>
where
    T: ApplyLog,
{
    type Log = T::Log;

    fn apply_log(&mut self, log: Self::Log) {
        self.ctx.apply_log(log);
    }
}

impl<T> Deref for Connected<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl<T> DerefMut for Connected<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl<E, T> Get<E> for Connected<T>
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.ctx.get(k)
    }
}

pub struct ConnectedTrx<'a, T> {
    pub trx: T,
    pub provider: TransactionProvider<'a>,
}

#[async_trait]
impl<'a, T> crate::Commit for ConnectedTrx<'a, T>
where
    T: mem::Commit + Send + Sync,
{
    type Log = T::Log;

    async fn commit(self) -> Result<Self::Log> {
        self.provider.commit().await?;
        Ok(self.trx.commit())
    }
}

impl<'a, E, T> Get<E> for ConnectedTrx<'a, T>
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.trx.get(k)
    }
}

pub struct ConnectedTrxRef<'a, T> {
    pub trx: T,
    pub provider: &'a TransactionProvider<'a>,
}

impl<'a, T> ConnectedTrxRef<'a, T> {
    pub fn new<'b>(trx: T, provider: &'a TransactionProvider<'b>) -> Self
    where
        'b: 'a,
    {
        Self { trx, provider }
    }
}

impl<'a, E, T> Get<E> for ConnectedTrxRef<'a, T>
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.trx.get(k)
    }
}

#[async_trait]
impl<'a, E, T> Insert<E> for ConnectedTrxRef<'a, T>
where
    E: Entity + Send + Sync + 'a,
    E::Key: Send + Sync,
    T: mem::Insert<E> + Send + Sync,
    TransactionProvider<'a>: provider::Upsert<E>,
{
    async fn insert(&mut self, k: E::Key, v: E) -> Result<()> {
        self.provider.upsert(&k, &v).await?;
        self.trx.insert(k, v);
        Ok(())
    }
}

#[async_trait]
impl<'a, E, T> Remove<E> for ConnectedTrxRef<'a, T>
where
    E: Entity + 'a,
    E::Key: Send + Sync,
    T: mem::Remove<E> + Send + Sync,
    TransactionProvider<'a>: Delete<E>,
{
    async fn remove(&mut self, k: E::Key) -> Result<()> {
        self.provider.delete(&k).await?;
        self.trx.remove(k);
        Ok(())
    }
}

impl<'a, 'b, T> crate::Transaction<'b> for async_cell_lock::QueueRwLockQueueGuard<'a, Connected<T>>
where
    T: mem::Transaction<'b> + Send + Sync,
{
    type Transaction = ConnectedTrx<'b, <T as mem::Transaction<'b>>::Transaction>;

    fn transaction(&'b self) -> Self::Transaction {
        ConnectedTrx {
            provider: self.provider.transaction(),
            trx: mem::Transaction::transaction(&self.ctx),
        }
    }
}
