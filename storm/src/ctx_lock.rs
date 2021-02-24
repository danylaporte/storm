use crate::{mem, provider, ApplyLog, Result};
use async_cell_lock::{QueueRwLock, QueueRwLockReadGuard, QueueWriteGuard};

pub struct CtxLock<CTX, PROVIDER>(QueueRwLock<(CTX, PROVIDER)>);

impl<CTX, PROVIDER> CtxLock<CTX, PROVIDER> {
    pub async fn read(&self) -> CtxRead<'_, CTX, PROVIDER> {
        CtxRead(self.0.read().await)
    }
}

pub struct CtxRead<'a, CTX, PROVIDER>(QueueRwLockReadGuard<'a, (CTX, PROVIDER)>);

impl<'a, CTX, PROVIDER> CtxRead<'a, CTX, PROVIDER> {
    pub async fn queue(self) -> Result<CtxQueue<'a, CTX, PROVIDER>>
    where
        CTX: mem::Transaction<'a>,
        PROVIDER: provider::Transaction,
    {
        Ok(CtxQueue(self.0.queue().await))
    }
}

pub struct CtxQueue<'a, CTX, PROVIDER>(QueueWriteGuard<'a, (CTX, PROVIDER)>);

impl<'a, CTX, PROVIDER> CtxQueue<'a, CTX, PROVIDER> {
    pub async fn commmit(self, log: CTX::Log) -> Result<()>
    where
        CTX: ApplyLog,
        PROVIDER: provider::Commit,
    {
        provider::Commit::commit(&self.0 .1).await?;
        self.0.write().await.0.apply_log(log);

        Ok(())
    }

    pub async fn transaction<'b>(&'b self) -> Result<<CTX as mem::Transaction<'b>>::Transaction>
    where
        CTX: mem::Transaction<'b>,
        PROVIDER: provider::Transaction,
    {
        provider::Transaction::transaction(&self.0 .1).await?;
        Ok(mem::Transaction::transaction(&self.0 .0))
    }
}
