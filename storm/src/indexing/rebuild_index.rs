use crate::{Ctx, CtxVar, Gc, Touchable};

pub trait RebuildIndex: Gc + Sized + Touchable {
    fn var() -> CtxVar<Self>;

    fn index_gc(ctx: &mut Ctx) {
        if let Some(idx) = ctx.ctx_ext_obj.get_mut(Self::var()).get_mut() {
            idx.gc();
        }
    }

    fn register_touchable<T: Touchable>() {
        T::touched().on(|ctx: &mut Ctx| {
            if ctx.ctx_ext_obj.get_mut(Self::var()).take().is_some() {
                Self::touched().call(ctx);
            }
        });
    }
}
