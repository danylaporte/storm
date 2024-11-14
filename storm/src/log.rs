use crate::{async_cell_lock::QueueRwLockQueueGuard, Asset, Ctx, Result};
use attached::{container, Container};

container!(pub LogVars);

type Logs = Container<LogVars>;

#[derive(Default)]
pub struct Log {
    apply_list: Vec<&'static (dyn Fn(&mut Ctx, &mut Logs) -> bool + Sync)>,
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

    pub(crate) fn get<A: Asset>(&self) -> Option<&A::Log> {
        self.logs.get(A::log_var())
    }

    pub(crate) fn get_or_init_mut<A: Asset>(&mut self) -> &mut A::Log {
        self.logs.get_or_init_mut(A::log_var(), || {
            self.apply_list.push(&apply::<A>);
            A::Log::default()
        })
    }
}

fn apply<A: Asset>(ctx: &mut Ctx, logs: &mut Logs) -> bool {
    if let Some(log) = logs.replace(A::log_var(), None) {
        if let Some(asset) = ctx.assets.get_mut(A::ctx_var()) {
            return asset.apply_log(log);
        }
    }

    false
}
