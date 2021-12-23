use crate::Ctx;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash},
    ops::DerefMut,
    rc::Rc,
    sync::Arc,
};
use tracing::instrument;
use vec_map::VecMap;

#[derive(Default)]
pub struct GcCtx {
    #[cfg(feature = "cache")]
    island: Option<u64>,
}

impl Ctx {
    #[instrument(level = "debug", skip(self))]
    pub fn gc(&mut self) {
        self.provider.gc();
        collectables::collect(self);

        #[cfg(feature = "cache")]
        {
            self.gc.island = Some(cache::current_cache_island_age());
        }
    }
}

pub trait Gc {
    const SUPPORT_GC: bool = false;
    fn gc(&mut self, _ctx: &GcCtx) {}
}

impl<T> Gc for Arc<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        if let Some(v) = Arc::get_mut(self) {
            v.gc(ctx);
        }
    }
}

impl<T> Gc for Box<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.deref_mut().gc(ctx);
    }
}

#[cfg(feature = "cache")]
impl<E> Gc for cache::CacheIsland<E> {
    const SUPPORT_GC: bool = true;

    fn gc(&mut self, ctx: &GcCtx) {
        if let Some(age) = ctx.island {
            self.clear_if_untouched_since(age);
        }
    }
}

impl<'a, T> Gc for Cow<'a, T>
where
    T: Clone + Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.to_mut().gc(ctx);
    }
}

impl<K, V, S> Gc for HashMap<K, V, S>
where
    K: Eq + Hash + Sync,
    V: Send,
    S: BuildHasher,
    V: Gc,
{
    const SUPPORT_GC: bool = V::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.par_iter_mut().for_each(|(_, v)| v.gc(ctx));
    }
}

impl<T, S> Gc for HashSet<T, S> where S: BuildHasher {}

impl<T> Gc for Option<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        if let Some(v) = self.as_mut() {
            v.gc(ctx);
        }
    }
}

impl<T> Gc for Rc<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        if let Some(v) = Rc::get_mut(self) {
            v.gc(ctx);
        }
    }
}

impl<T> Gc for Vec<T>
where
    T: Gc + Send,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.par_iter_mut().for_each(|v| v.gc(ctx));
    }
}

impl<K, V> Gc for VecMap<K, V>
where
    K: From<usize> + Send,
    V: Gc + Send,
{
    const SUPPORT_GC: bool = V::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.par_iter_mut().for_each(|(_, v)| v.gc(ctx));
    }
}

impl<T> Gc for [T]
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self, ctx: &GcCtx) {
        self.iter_mut().for_each(|v| v.gc(ctx));
    }
}

macro_rules! gc {
    (tuple $($t:ident:$n:tt),*) => {
        impl<$($t: Gc),*> Gc for ($($t,)*) {
            const SUPPORT_GC: bool = false $(|| $t::SUPPORT_GC)*;

            #[allow(unused_variables)]
            fn gc(&mut self, ctx: &GcCtx) {
                $(self.$n.gc(ctx);)*
            }
        }
    };
    ($t:ty) => {
        impl Gc for $t {}
    };
}

gc!(Arc<[u8]>);
gc!(Arc<str>);
gc!(Box<[u8]>);
gc!(Box<str>);
gc!(Rc<[u8]>);
gc!(Rc<str>);
gc!(String);
gc!(bool);
gc!(f32);
gc!(f64);
gc!(i128);
gc!(i16);
gc!(i32);
gc!(i64);
gc!(i8);
gc!(isize);
gc!(u128);
gc!(u16);
gc!(u32);
gc!(u64);
gc!(u8);
gc!(usize);

#[cfg(feature = "chrono")]
gc!(chrono::Date<chrono::FixedOffset>);

#[cfg(feature = "chrono")]
gc!(chrono::Date<chrono::Utc>);

#[cfg(feature = "chrono")]
gc!(chrono::DateTime<chrono::FixedOffset>);

#[cfg(feature = "chrono")]
gc!(chrono::DateTime<chrono::Utc>);

#[cfg(feature = "chrono")]
gc!(chrono::NaiveDate);

#[cfg(feature = "chrono")]
gc!(chrono::NaiveDateTime);

#[cfg(feature = "chrono")]
gc!(chrono::NaiveTime);

#[cfg(feature = "dec19x5")]
gc!(dec19x5::Decimal);

#[cfg(feature = "uuid")]
gc!(uuid::Uuid);

gc!(tuple);
gc!(tuple A:0);
gc!(tuple A:0,B:1);
gc!(tuple A:0,B:1,C:2);
gc!(tuple A:0,B:1,C:2,D:3);
gc!(tuple A:0,B:1,C:2,D:3,E:4);
gc!(tuple A:0,B:1,C:2,D:3,E:4,F:5);
gc!(tuple A:0,B:1,C:2,D:3,E:4,F:5,G:6);
gc!(tuple A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7);
gc!(tuple A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8);
gc!(tuple A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7,I:8,J:9);

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
