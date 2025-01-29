use crate::{BoxFuture, CtxTransaction, Result};
use async_cell_lock::sync::mutex::Mutex;
use std::sync::Arc;

pub trait CommitHandler {
    fn handle_commit<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<()>>;
}

impl<T> CommitHandler for T
where
    T: for<'a> Fn(&'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<()>>,
{
    fn handle_commit<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<()>> {
        (self)(trx)
    }
}

type ArcCommitHandler = &'static (dyn for<'a> Fn(&'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<()>>
              + Send
              + Sync);

static HANDLERS: Mutex<Option<Arc<[ArcCommitHandler]>>> = Mutex::new(None, "COMMIT_HANDLERS");

pub(crate) fn call_on_commit<'b>(trx: &'b mut CtxTransaction<'_>) -> BoxFuture<'b, Result<()>> {
    Box::pin(async move {
        let handlers = HANDLERS.lock()?.as_ref().map(Arc::clone);

        if let Some(handlers) = handlers {
            for handler in handlers.iter() {
                handler.handle_commit(trx).await?;
            }
        }

        Ok(())
    })
}

pub fn register_on_commit_handler(handler: ArcCommitHandler) -> Result<()> {
    let mut guard = HANDLERS.lock()?;

    let mut v = match guard.as_ref() {
        Some(v) => v.iter().copied().collect::<Vec<_>>(),
        None => Vec::new(),
    };

    v.push(handler);
    *guard = Some(v.into());

    Ok(())
}
