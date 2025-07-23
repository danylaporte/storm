use super::IndexLog;
use crate::{
    indexing::{Index, IndexTrx},
    Entity,
};
use fast_set::{AdaptiveBitmap, AdaptiveBitmapLog, AdaptiveBitmapTrx, IntSet, IntSetTrx};
use std::{
    any::Any,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    ops::Deref,
};

pub struct SingleSetIndex<K, A>(AdaptiveBitmap, PhantomData<(K, A)>);

impl IndexLog for Option<AdaptiveBitmap> {}

impl<K, A> SingleSetIndex<K, A> {
    pub fn new() -> Self {
        Self(AdaptiveBitmap::new(), PhantomData)
    }

    #[inline]
    pub fn contains(&self, k: K) -> bool
    where
        usize: From<K>,
    {
        self.0.contains(usize::from(k) as u32)
    }

    fn remove_impl(&self, log: &mut dyn IndexLog, old: Option<u32>) {
        if let Some(old) = old {
            log_mut(log).remove(&self.0, old);
        }
    }

    fn upsert_impl(&self, log: &mut dyn IndexLog, old: Option<u32>, new: Option<u32>) {
        if old == new {
            return;
        }

        self.remove_impl(log, old);

        if let Some(new) = new {
            log_mut(log).insert(&self.0, new);
        }
    }
}

impl<K, A> Default for SingleSetIndex<K, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, A> Deref for SingleSetIndex<K, A>
where
    usize: From<K>,
{
    type Target = IntSet<K>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { IntSet::from_bitmap_ref(&self.0) }
    }
}

fn log_mut(log: &mut dyn IndexLog) -> &mut AdaptiveBitmapLog {
    <dyn Any>::downcast_mut(&mut *log).expect("AdaptiveBitmapLog")
}

fn log_ref(log: &dyn IndexLog) -> &AdaptiveBitmapLog {
    <dyn Any>::downcast_ref(log).expect("AdaptiveBitmapLog")
}

impl<K, A> Clone for SingleSetIndex<K, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<K, A> Debug for SingleSetIndex<K, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SingleSetIndex")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl<K, A> PartialEq for SingleSetIndex<K, A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K, E, A> Index<E> for SingleSetIndex<K, A>
where
    A: SingleSetAdapt<E> + Send + Sync + 'static,
    E: Entity<Key = K>,
    K: Copy + Send + Sync + 'static,
    usize: From<K>,
{
    fn apply_log(&mut self, log: Box<dyn IndexLog>) {
        let log = *Box::<dyn Any>::downcast::<AdaptiveBitmapLog>(log).expect("AdaptiveBitmapLog");
        self.0.apply(log);
    }

    fn create_log(&self) -> Box<dyn IndexLog> {
        Box::new(AdaptiveBitmapLog::new())
    }

    fn remove(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E)
    where
        E: Entity,
    {
        let old = adapt_u32::<A, E>(k, entity);
        self.remove_impl(log, old);
    }

    fn upsert(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity,
    {
        let old = old.and_then(|old| adapt_u32::<A, E>(k, old));
        let new = adapt_u32::<A, E>(k, entity);

        self.upsert_impl(log, old, new);
    }
}

impl<K, A> IndexTrx for SingleSetIndex<K, A>
where
    A: 'static,
    K: 'static,
{
    type Trx<'a> = IntSetTrx<'a, K>;

    #[inline]
    fn trx<'a>(&'a self, log: &'a dyn IndexLog) -> Self::Trx<'a> {
        let trx = AdaptiveBitmapTrx::new(&self.0, log_ref(log));
        unsafe { IntSetTrx::from_adaptive_bitmap_trx(trx) }
    }
}

pub struct SingleSetTrx<'a, K> {
    changes: &'a mut Option<AdaptiveBitmap>,
    map: &'a AdaptiveBitmap,
    _k: PhantomData<K>,
}

impl<K> SingleSetTrx<'_, K> {
    #[inline]
    pub fn contains(&self, key: K) -> bool
    where
        usize: From<K>,
    {
        self.contains_impl(usize::from(key))
    }

    fn contains_impl(&self, key: usize) -> bool {
        self.get_impl().contains(key as u32)
    }

    #[inline]
    fn get_impl(&self) -> &AdaptiveBitmap {
        self.changes.as_ref().unwrap_or(self.map)
    }
}

impl<K> Deref for SingleSetTrx<'_, K>
where
    usize: From<K>,
{
    type Target = IntSet<K>;

    fn deref(&self) -> &Self::Target {
        unsafe { IntSet::from_bitmap_ref(self.get_impl()) }
    }
}

pub trait SingleSetAdapt<E: Entity> {
    fn adapt(k: &E::Key, v: &E) -> bool;
}

fn adapt_u32<A, E>(k: &E::Key, e: &E) -> Option<u32>
where
    A: SingleSetAdapt<E>,
    E: Entity,
    E::Key: Copy,
    usize: From<E::Key>,
{
    A::adapt(k, e).then(|| usize::from(*k) as u32)
}

#[macro_export]
macro_rules! single_set_adapt {
    ($adapt:ident, $alias:ident, $init:ident, $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> bool {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::SingleSetAdapt<$entity_ty> for $adapt {
            #[allow(unused_variables)]
            fn adapt($id: &$entity_key, $entity: &$entity_ty) -> bool {
                $($t)*
            }
        }

        $vis type $alias = storm::indexing::SingleSetIndex<$entity_key, $adapt>;

        #[$crate::linkme::distributed_slice($crate::STORM_INITS)]
        #[linkme(crate = $crate::linkme)]
        fn $init() {
            <$entity_ty as $crate::EntityAccessor>::entity_inits().register(|tbl| {
                tbl.register_index($alias::new());
            });
        }
    };
}
