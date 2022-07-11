use crate::{async_cell_lock::QueueRwLockQueueGuard, ApplyLog, Ctx, LogsVar, Result};

pub struct Logs(pub(crate) LogsVar);

impl Logs {
    pub async fn apply_log(self, ctx: QueueRwLockQueueGuard<'_, Ctx>) -> Result<bool> {
        Ok(ctx.write().await?.apply_log(self))
    }
}
