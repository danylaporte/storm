use crate::{Asset, Assets};
use parking_lot::Mutex;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash},
    ops::DerefMut,
    ptr::addr_eq,
    rc::Rc,
    sync::Arc,
};
use vec_map::VecMap;

type AssetFnRef = &'static (dyn Fn(&mut Assets) + Sync);

#[derive(Default)]
pub(crate) struct AssetGc(Mutex<Vec<AssetFnRef>>);

impl AssetGc {
    /// Collect all garbage in assets.
    pub(crate) fn collect(&mut self, assets: &mut Assets) {
        for f in self.0.get_mut() {
            f(assets);
        }
    }

    /// Register a garbage collection for this asset.
    pub(crate) fn register<A: Asset>(&self) {
        if A::SUPPORT_GC {
            self.register_fn(&gc::<A>);
        }
    }

    fn register_fn(&self, f: &'static (dyn Fn(&mut Assets) + Sync)) {
        let mut guard = self.0.lock();

        if !guard.iter().any(|a| addr_eq(a, f)) {
            guard.push(f);
        }
    }
}

fn gc<A: Asset>(assets: &mut Assets) {
    if let Some(asset) = assets.get_mut(A::ctx_var()) {
        asset.gc();
    }
}

pub trait Gc {
    const SUPPORT_GC: bool = false;
    fn gc(&mut self) {}
}

impl<T> Gc for Arc<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        if let Some(v) = Arc::get_mut(self) {
            v.gc();
        }
    }
}

impl<T> Gc for Box<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        self.deref_mut().gc();
    }
}

#[cfg(feature = "cache")]
impl<E> Gc for cache::CacheIsland<E> {
    const SUPPORT_GC: bool = true;

    fn gc(&mut self) {
        let was_touched = self.untouch();

        if !was_touched && self.take().is_some() {
            #[cfg(feature = "telemetry")]
            crate::telemetry::inc_storm_cache_island_gc();
        }
    }
}

impl<T> Gc for Cow<'_, T>
where
    T: Clone + Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        self.to_mut().gc();
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

    fn gc(&mut self) {
        self.iter_mut().for_each(|(_, v)| v.gc());
    }
}

impl<T, S> Gc for HashSet<T, S> where S: BuildHasher {}

impl<T> Gc for Option<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        if let Some(v) = self.as_mut() {
            v.gc();
        }
    }
}

impl<T> Gc for once_cell::sync::OnceCell<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        if let Some(v) = self.get_mut() {
            v.gc();
        }
    }
}

impl<T> Gc for Rc<T>
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        if let Some(v) = Rc::get_mut(self) {
            v.gc();
        }
    }
}

impl<T> Gc for Vec<T>
where
    T: Gc + Send,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        self.iter_mut().for_each(|v| v.gc());
    }
}

impl<K, V> Gc for VecMap<K, V>
where
    K: Copy + From<usize> + Send,
    V: Gc + Send,
{
    const SUPPORT_GC: bool = V::SUPPORT_GC;

    fn gc(&mut self) {
        self.iter_mut().for_each(|(_, v)| v.gc());
    }
}

impl<T> Gc for [T]
where
    T: Gc,
{
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    fn gc(&mut self) {
        self.iter_mut().for_each(|v| v.gc());
    }
}

#[cfg(feature = "str_utils")]
impl Gc for str_utils::str_ci::StringCi {
    const SUPPORT_GC: bool = false;

    fn gc(&mut self) {}
}

#[cfg(feature = "str_utils")]
impl<F> Gc for str_utils::form_str::FormStr<F> {
    const SUPPORT_GC: bool = false;

    fn gc(&mut self) {}
}

macro_rules! gc {
    (tuple $($t:ident:$n:tt),*) => {
        impl<$($t: Gc),*> Gc for ($($t,)*) {
            const SUPPORT_GC: bool = false $(|| $t::SUPPORT_GC)*;

            #[allow(unused_variables)]
            fn gc(&mut self) {
                $(self.$n.gc();)*
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

    static FUNCS: RwLock<Vec<Box<dyn Fn(&mut Ctx) + Send + Sync>>> = RwLock::new(Vec::new());

    pub fn collect(ctx: &mut Ctx) {
        FUNCS.read().iter().for_each(|f| f(ctx));
    }

    pub fn register<F>(f: F)
    where
        F: Fn(&mut Ctx) + Send + Sync + 'static,
    {
        register_impl(Box::new(f));
    }

    fn register_impl(f: Box<dyn Fn(&mut Ctx) + Send + Sync>) {
        FUNCS.write().push(f);
    }
}
