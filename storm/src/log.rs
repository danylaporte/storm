use crate::{async_cell_lock::QueueRwLockQueueGuard, Ctx, Obj, Result};
use attached::{container, Container, Var};

container!(pub LogVars);

type ApplyFn = &'static (dyn Fn(&mut Ctx, &mut Logs) -> bool + Sync);
type Logs = Container<LogVars>;

#[derive(Default)]
pub struct Log {
    apply_list: Vec<ApplyFn>,
    logs: Logs,
}

impl Log {
    pub(crate) fn apply(mut self, ctx: &mut Ctx) -> bool {
        let mut changed = false;

        if !self.apply_list.is_empty() {
            for a in self.apply_list {
                changed = a(ctx, &mut self.logs) || changed;
            }
        }

        changed
    }

    pub async fn apply_log(self, ctx: QueueRwLockQueueGuard<'_, Ctx>) -> Result<bool> {
        let mut guard = ctx.write().await?;
        Ok(self.apply(&mut guard))
    }

    pub(crate) fn get<Log>(&self, token: &LogToken<Log>) -> Option<&Log> {
        self.logs.get(token.var)
    }

    pub(crate) fn get_or_init_mut<Log>(&mut self, token: &LogToken<Log>) -> &mut Log
    where
        Log: Default,
    {
        self.logs.get_or_init_mut(token.var, || {
            self.apply_list.push(token.apply_fn);
            Log::default()
        })
    }
}

fn apply<A: Obj>(ctx: &mut Ctx, logs: &mut Logs) -> bool {
    if let Some(log) = logs.replace(A::log_var(), None) {
        if let Some(obj) = ctx.objs.get_mut(A::ctx_var()) {
            return obj.apply_log(log);
        }
    }

    false
}

pub struct LogToken<Log> {
    apply_fn: ApplyFn,
    var: Var<Log, LogVars>,
}

impl<Log> LogToken<Log> {
    pub(crate) fn from_obj<A: Obj<Log = Log>>() -> Self {
        Self {
            apply_fn: &apply::<A>,
            var: A::log_var(),
        }
    }
}
