use super::IndexLog;
use crate::{
    indexing::{Index, IndexTrx},
    Entity,
};
use fast_set::{IntSet, IntSetTrx, OneU32ManyU32, OneU32ManyU32Log};
use std::{any::Any, marker::PhantomData};

impl IndexLog for OneU32ManyU32Log {}

pub struct IntOneToManyIndex<K, V, A> {
    _a: PhantomData<(K, V, A)>,
    map: OneU32ManyU32,
}

impl<K, V, A> IntOneToManyIndex<K, V, A> {
    pub fn new() -> Self {
        Self {
            _a: PhantomData,
            map: Default::default(),
        }
    }

    #[inline]
    pub fn contains(&self, key: K, val: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.map
            .contains(usize::from(key) as u32, usize::from(val) as u32)
    }

    #[inline]
    pub fn contains_none(&self, val: V) -> bool
    where
        usize: From<V>,
    {
        self.map.contains_none(usize::from(val) as u32)
    }

    #[inline]
    pub fn get(&self, key: K) -> &IntSet<V>
    where
        usize: From<K> + From<V>,
    {
        let b = self.map.get(usize::from(key) as u32);
        unsafe { IntSet::from_bitmap_ref(b) }
    }

    fn insert_impl2(&self, log: &mut OneU32ManyU32Log, k: Option<u32>, value: u32) {
        match k {
            Some(k) => log.insert(&self.map, k, value),
            None => log.insert_none(&self.map, value),
        };
    }

    #[inline]
    pub fn none(&self) -> &IntSet<V>
    where
        usize: From<V>,
    {
        let b = self.map.none();
        unsafe { IntSet::from_bitmap_ref(b) }
    }

    fn remove_impl(&self, log: &mut dyn IndexLog, old: Option<(Option<u32>, u32)>) {
        if let Some((k, v)) = old {
            let log = log_mut(log);
            self.remove_impl2(log, k, v);
        }
    }

    fn remove_impl2(&self, log: &mut OneU32ManyU32Log, k: Option<u32>, value: u32) {
        match k {
            Some(k) => log.remove(&self.map, k, value),
            None => log.remove_none(&self.map, value),
        };
    }

    fn upsert_impl(
        &self,
        log: &mut dyn IndexLog,
        old: Option<(Option<u32>, u32)>,
        new: Option<(Option<u32>, u32)>,
    ) {
        if old != new {
            let log = log_mut(log);

            if let Some((k, v)) = old {
                self.remove_impl2(log, k, v);
            }

            if let Some((k, v)) = new {
                self.insert_impl2(log, k, v);
            }
        }
    }
}

impl<K, V, A> Default for IntOneToManyIndex<K, V, A> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

fn log_mut(log: &mut dyn IndexLog) -> &mut OneU32ManyU32Log {
    <dyn Any>::downcast_mut(&mut *log).expect("OneU32ManyU32Log")
}

fn log_ref(log: &dyn IndexLog) -> &OneU32ManyU32Log {
    <dyn Any>::downcast_ref(log).expect("OneU32ManyU32Log")
}

impl<K, V, E, A> Index<E> for IntOneToManyIndex<K, V, A>
where
    A: IntOneToManyAdapt<E, K, V> + Send + Sync + 'static,
    E: Entity,
    K: Send + Sync + 'static,
    V: Send + Sync + 'static,
    usize: From<K> + From<V>,
{
    fn apply_log(&mut self, log: Box<dyn IndexLog>) {
        let log = *Box::<dyn Any + Send + Sync>::downcast::<OneU32ManyU32Log>(log)
            .expect("IntOneToManyLog");

        self.map.apply(log);
    }

    #[inline]
    fn create_log(&self) -> Box<dyn IndexLog> {
        Box::new(OneU32ManyU32Log::default())
    }

    fn remove(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E)
    where
        E: Entity,
    {
        let old = adapt_u32::<A, E, K, V>(k, entity);

        self.remove_impl(log, old);
    }

    fn upsert(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity,
    {
        let old = old.and_then(|old| adapt_u32::<A, E, K, V>(k, old));
        let new = adapt_u32::<A, E, K, V>(k, entity);

        self.upsert_impl(log, old, new);
    }
}

impl<K, V, A> IndexTrx for IntOneToManyIndex<K, V, A>
where
    A: 'static,
    K: 'static,
    V: 'static,
{
    type Trx<'a> = IntOneToManyTrx<'a, K, V>;

    fn trx<'a>(&'a self, log: &'a dyn IndexLog) -> Self::Trx<'a> {
        IntOneToManyTrx {
            _kv: PhantomData,
            commit: &self.map,
            log: log_ref(log),
        }
    }
}

pub struct IntOneToManyTrx<'a, K, V> {
    _kv: PhantomData<(K, V)>,
    commit: &'a OneU32ManyU32,
    log: &'a OneU32ManyU32Log,
}

impl<K, V> IntOneToManyTrx<'_, K, V> {
    #[inline]
    pub fn contains(&self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.log.contains(
            self.commit,
            usize::from(key) as u32,
            usize::from(value) as u32,
        )
    }

    #[inline]
    pub fn get(&self, key: K) -> IntSetTrx<V>
    where
        usize: From<K> + From<V>,
    {
        let b = self.log.get(self.commit, usize::from(key) as u32);
        unsafe { IntSetTrx::from_adaptive_bitmap_trx(b) }
    }

    pub fn get_opt(&self, key: Option<K>) -> IntSetTrx<V>
    where
        usize: From<K> + From<V>,
    {
        match key {
            Some(key) => self.get(key),
            None => self.none(),
        }
    }

    #[inline]
    pub fn none(&self) -> IntSetTrx<V>
    where
        usize: From<V>,
    {
        let b = self.log.none(self.commit);
        unsafe { IntSetTrx::from_adaptive_bitmap_trx(b) }
    }
}

pub trait IntOneToManyAdapt<E: Entity, K, V> {
    fn adapt(k: &E::Key, v: &E) -> Option<(Option<K>, V)>;
}

fn adapt_u32<A, E, K, V>(k: &E::Key, e: &E) -> Option<(Option<u32>, u32)>
where
    A: IntOneToManyAdapt<E, K, V>,
    E: Entity,
    usize: From<K> + From<V>,
{
    A::adapt(k, e).map(|(k, v)| (k.map(|k| usize::from(k) as u32), usize::from(v) as u32))
}

#[macro_export]
macro_rules! int_one_to_many_adapt {
    ($adapt:ident, $alias:ident, $init:ident, $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<(Option<$k:ty>, $v:ty)> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl $crate::indexing::IntOneToManyAdapt<$entity_ty, $k, $v> for $adapt {
            #[allow(unused_variables)]
            fn adapt($id: &$entity_key, $entity: &$entity_ty) -> Option<(Option<$k>, $v)> {
                $($t)*
            }
        }

        $vis type $alias = $crate::indexing::IntOneToManyIndex<$k, $v, $adapt>;

        #[$crate::linkme::distributed_slice($crate::STORM_INITS)]
        #[linkme(crate = $crate::linkme)]
        fn $init() {
            <$entity_ty as $crate::EntityAccessor>::entity_inits().register(|tbl| {
                tbl.register_index($alias::new());
            });
        }
    };
}
