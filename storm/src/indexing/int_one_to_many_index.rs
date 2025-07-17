use super::{
    set::{empty, Set},
    IndexLog,
};
use crate::{
    indexing::{Index, IndexTrx},
    Entity,
};
use roaring::RoaringBitmap;
use std::{
    any::Any,
    collections::hash_map::Entry,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
};

#[derive(Default)]
struct IntOneToManyLog {
    map: nohash::IntMap<u32, RoaringBitmap>,
    none: Option<RoaringBitmap>,
}

impl IndexLog for IntOneToManyLog {}

pub struct IntOneToManyIndex<K, V, A> {
    map: nohash::IntMap<u32, RoaringBitmap>,
    none: RoaringBitmap,
    _kva: PhantomData<(K, V, A)>,
}

impl<K, V, A> IntOneToManyIndex<K, V, A> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
            none: RoaringBitmap::default(),
            _kva: PhantomData,
        }
    }

    pub fn contains(&self, k: K, v: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.contains_impl(usize::from(k) as u32, usize::from(v) as u32)
    }

    pub fn contains_opt(&self, k: Option<K>, v: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        match k {
            Some(k) => self.contains(k, v),
            None => self.none.contains(usize::from(v) as u32),
        }
    }

    fn contains_impl(&self, k: u32, v: u32) -> bool {
        match self.map.get(&k) {
            Some(b) => b.contains(v),
            None => false,
        }
    }

    pub fn get(&self, k: K) -> &Set<V>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.get_impl(usize::from(k) as u32))
    }

    pub fn get_opt(&self, k: Option<K>) -> &Set<V>
    where
        usize: From<K>,
    {
        match k {
            Some(k) => self.get(k),
            None => Set::from_roaring_bitmap_ref(&self.none),
        }
    }

    fn get_impl(&self, k: u32) -> &RoaringBitmap {
        self.map.get(&k).unwrap_or_else(empty)
    }

    fn insert_impl2(&self, log: &mut IntOneToManyLog, k: Option<u32>, value: u32) {
        match k {
            Some(k) => match log.map.entry(k) {
                Entry::Occupied(mut o) => {
                    o.get_mut().insert(value);
                }
                Entry::Vacant(v) => {
                    let mut set = if let Some(set) = self.map.get(&k) {
                        if set.contains(value) {
                            return;
                        }

                        set.clone()
                    } else {
                        RoaringBitmap::new()
                    };

                    set.insert(value);
                    v.insert(set);
                }
            },
            None => {
                if log.none.is_none() {
                    log.none = Some(self.none.clone());
                }

                unsafe { log.none.as_mut().unwrap_unchecked() }.insert(value);
            }
        }
    }

    pub fn none(&self) -> &Set<V> {
        Set::from_roaring_bitmap_ref(&self.none)
    }

    fn remove_impl(&self, log: &mut dyn IndexLog, old: Option<(Option<u32>, u32)>) {
        if let Some((k, v)) = old {
            let log = index_map_mut(log);
            self.remove_impl2(log, k, v);
        }
    }

    fn remove_impl2(&self, log: &mut IntOneToManyLog, k: Option<u32>, value: u32) {
        match k {
            Some(k) => match log.map.entry(k) {
                Entry::Occupied(mut o) => {
                    o.get_mut().remove(value);
                }
                Entry::Vacant(v) => {
                    if let Some(set) = self.map.get(&k) {
                        if set.contains(value) {
                            let mut set = set.clone();

                            set.remove(value);
                            v.insert(set);
                        }
                    }
                }
            },
            None => {
                if log.none.is_none() {
                    log.none = Some(self.none.clone());
                }

                unsafe { log.none.as_mut().unwrap_unchecked() }.remove(value);
            }
        }
    }

    fn upsert_impl(
        &self,
        log: &mut dyn IndexLog,
        old: Option<(Option<u32>, u32)>,
        new: Option<(Option<u32>, u32)>,
    ) {
        if old != new {
            let log = index_map_mut(log);

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
    fn default() -> Self {
        Self::new()
    }
}

fn index_map_mut(log: &mut dyn IndexLog) -> &mut IntOneToManyLog {
    <dyn Any>::downcast_mut(&mut *log).expect("IndexMap")
}

impl<K, V, A> Clone for IntOneToManyIndex<K, V, A> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            none: self.none.clone(),
            _kva: PhantomData,
        }
    }
}

impl<K, V, A> Debug for IntOneToManyIndex<K, V, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntOneToManyIndex")
            .field("map", &self.map)
            .field("none", &self.none)
            .finish()
    }
}

impl<K, V, A> PartialEq for IntOneToManyIndex<K, V, A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map && self.none == other.none
    }
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
        let log = *Box::<dyn Any + Send + Sync>::downcast::<IntOneToManyLog>(log)
            .expect("IntOneToManyLog");

        for (k, set) in log.map {
            match self.map.entry(k) {
                Entry::Occupied(mut o) => {
                    if set.is_empty() {
                        o.remove();
                    } else {
                        o.insert(set);
                    }
                }
                Entry::Vacant(v) => {
                    if !set.is_empty() {
                        v.insert(set);
                    }
                }
            }
        }

        if let Some(none) = log.none {
            self.none = none;
        }
    }

    fn create_log(&self) -> Box<dyn IndexLog> {
        Box::new(IntOneToManyLog::default())
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

    fn trx<'a>(&'a self, log: &'a mut dyn IndexLog) -> Self::Trx<'a> {
        IntOneToManyTrx {
            _kv: PhantomData,
            changes: index_map_mut(log),
            map: &self.map,
            none: &self.none,
        }
    }
}

pub struct IntOneToManyTrx<'a, K, V> {
    changes: &'a mut IntOneToManyLog,
    map: &'a nohash::IntMap<u32, RoaringBitmap>,
    none: &'a RoaringBitmap,
    _kv: PhantomData<(K, V)>,
}

impl<K, V> IntOneToManyTrx<'_, K, V> {
    pub fn contains(&self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.contains_impl(usize::from(key) as u32, usize::from(value) as u32)
    }

    fn contains_impl(&self, key: u32, value: u32) -> bool {
        match self.changes.map.get(&key) {
            Some(set) => set.contains(value),
            None => match self.map.get(&key) {
                Some(set) => set.contains(value),
                None => false,
            },
        }
    }

    pub fn get(&self, key: K) -> &Set<V>
    where
        usize: From<K> + From<V>,
    {
        Set::from_roaring_bitmap_ref(self.get_impl(usize::from(key) as u32))
    }

    pub fn get_opt(&self, key: Option<K>) -> &Set<V>
    where
        usize: From<K> + From<V>,
    {
        Set::from_roaring_bitmap_ref(self.get_opt_impl(key.map(|key| usize::from(key) as u32)))
    }

    fn get_impl(&self, key: u32) -> &RoaringBitmap {
        match self.changes.map.get(&key) {
            Some(set) => set,
            None => self.map.get(&key).unwrap_or_else(empty),
        }
    }

    fn get_opt_impl(&self, key: Option<u32>) -> &RoaringBitmap {
        match key {
            Some(key) => self.get_impl(key),
            None => self.none_impl(),
        }
    }

    pub fn none(&self) -> &Set<V> {
        Set::from_roaring_bitmap_ref(self.none_impl())
    }

    fn none_impl(&self) -> &RoaringBitmap {
        self.changes.none.as_ref().unwrap_or(self.none)
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
