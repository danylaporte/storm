use crate::{
    BoxFuture, Clearable, Ctx, CtxTypeInfo, CtxVar, EV_CREATED, Gc, OBJ_INDEX, Result, Touchable,
    debug_index_get_or_init_elapsed,
};
use std::{future::Future, time::Instant};

pub trait RebuildIndex:
    Clearable + CtxTypeInfo + Gc + Send + Sized + Sync + Touchable + 'static
{
    fn var() -> CtxVar<Self>;

    fn handle_cleared_or_touched(ctx: &mut Ctx) {
        if ctx.ctx_ext_obj.get_mut(Self::var()).take().is_some() {
            Self::cleared().call(ctx);
        }
    }

    fn index_gc(ctx: &mut Ctx) {
        if let Some(idx) = ctx.ctx_ext_obj.get_mut(Self::var()).get_mut() {
            idx.gc();
        }
    }

    /// Must be called in the registration phase.
    fn register() {
        // allocates the var before any `Ctx` is created.
        Self::var();

        if Self::SUPPORT_GC {
            Ctx::on_gc_collect(Self::index_gc);
        }
    }

    fn register_clear_or_touchable<T: Clearable + Touchable>() {
        T::cleared().on(Self::handle_cleared_or_touched);
        T::touched().on(Self::handle_cleared_or_touched);
    }
}

/// Private: used by the `indexing` macro.
///
/// Gets the index, or builds it from its dependencies, without holding any lock.
#[doc(hidden)]
pub fn __get_or_init_sync<I, D>(
    ctx: &Ctx,
    deps: impl FnOnce() -> D,
    build: impl FnOnce(D) -> I,
) -> &I
where
    I: RebuildIndex,
{
    let slot = ctx.ctx_ext_obj.get(I::var());

    if let Some(v) = slot.get() {
        return v;
    }

    let deps = deps();

    if let Some(v) = slot.get() {
        return v;
    }

    let instant = Instant::now();
    let r = slot.get_or_init(|| traced_build(I::NAME, || build(deps)));
    debug_index_get_or_init_elapsed(instant, I::NAME);
    r
}

/// Private: used by the `indexing` macro.
///
/// Gets the index, or loads its dependencies and builds it under the provider gate.
#[doc(hidden)]
pub fn __get_or_init_async<'a, I, D>(
    ctx: &'a Ctx,
    deps: impl Future<Output = Result<D>> + Send + 'a,
    build: impl FnOnce(D) -> I + Send + 'a,
) -> BoxFuture<'a, Result<&'a I>>
where
    I: RebuildIndex,
    D: Send + 'a,
{
    Box::pin(async move {
        let slot = ctx.ctx_ext_obj.get(I::var());

        if let Some(v) = slot.get() {
            return Ok(v);
        }

        let deps = deps.await?;

        if let Some(v) = slot.get() {
            return Ok(v);
        }

        let _gate = ctx.provider.gate(I::NAME).await;

        Ok(slot.get_or_init(|| traced_build(I::NAME, || build(deps))))
    })
}

#[inline]
fn traced_build<I>(name: &'static str, build: impl FnOnce() -> I) -> I {
    let _span = index_span(name);

    #[cfg(feature = "telemetry")]
    let instant = Instant::now();

    let idx = build();

    #[cfg(feature = "telemetry")]
    metrics::histogram!("index_build_dur_sec", "name" => std::any::type_name::<I>())
        .record(instant.elapsed().as_secs_f64());

    idx
}

/// Not generic, so the tracing callsite is emitted once instead of once per index.
fn index_span(name: &'static str) -> tracing::span::EnteredSpan {
    tracing::debug_span!("index", name, obj = OBJ_INDEX, ev = EV_CREATED).entered()
}
