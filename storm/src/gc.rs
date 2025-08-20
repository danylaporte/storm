use crate::{Ctx, GcEvent};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash},
    ops::DerefMut,
    rc::Rc,
    sync::Arc,
};
use vec_map::VecMap;

static GC_EVENT: GcEvent = GcEvent::new();

impl Ctx {
    pub fn gc(&mut self) {
        #[cfg(feature = "telemetry")]
        crate::telemetry::inc_storm_gc();

        self.provider.gc();
        GC_EVENT.call(self);
    }

    /// Private. for macro use only.
    #[doc(hidden)]
    #[inline]
    pub fn on_gc_collect(f: fn(ctx: &mut Ctx)) {
        GC_EVENT.on(f);
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

impl<K, V> Gc for fast_set::flat_set_index::FlatSetIndex<K, V> {}

impl<T> Gc for fast_set::IntSet<T> {}

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

impl<T> Gc for std::sync::OnceLock<T>
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
    V: Gc,
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
