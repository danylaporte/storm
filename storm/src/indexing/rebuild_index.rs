use crate::{Clearable, Ctx, CtxVar, Gc, Touchable};

pub trait RebuildIndex: Clearable + Gc + Sized + Touchable {
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

    fn register_clear_or_touchable<T: Clearable + Touchable>() {
        T::cleared().on(Self::handle_cleared_or_touched);
        T::touched().on(Self::handle_cleared_or_touched);
    }
}
