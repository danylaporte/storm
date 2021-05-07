use crate::Ctx;
use tracing::instrument;

#[derive(Default)]
pub struct GcCtx {
    island: Option<u64>,
}

impl Ctx {
    #[instrument(level = "debug", skip(self))]
    pub fn gc(&mut self) {
        self.provider.gc();
        collectables::collect(self);
        self.gc.island = Some(cache::current_cache_island_age());
    }
}

pub trait Gc {
    fn gc(&mut self, ctx: &GcCtx) -> bool;
}

#[cfg(feature = "cache")]
impl<E> Gc for cache::CacheIsland<E> {
    fn gc(&mut self, ctx: &GcCtx) -> bool {
        if let Some(age) = ctx.island {
            self.clear_if_untouched_since(age)
        } else {
            false
        }
    }
}

#[allow(clippy::type_complexity)]
pub mod collectables {
    use crate::Ctx;
    use parking_lot::RwLock;

    #[static_init::dynamic]
    static FUNCS: RwLock<Vec<Box<dyn Fn(&mut Ctx) + Send + Sync>>> = Default::default();

    #[tracing::instrument(level = "debug", skip(ctx))]
    pub fn collect(ctx: &mut Ctx) {
        FUNCS.read().iter().for_each(|f| f(ctx));
    }

    pub fn register<F>(f: F)
    where
        F: Fn(&mut Ctx) + Send + Sync + 'static,
    {
        FUNCS.write().push(Box::new(f));
    }
}
