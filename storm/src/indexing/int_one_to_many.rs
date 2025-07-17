use crate::{
    indexing::{set::empty, Set},
    Gc,
};
use nohash::IntMap;
use roaring::RoaringBitmap;
use std::marker::PhantomData;

pub struct IntOneToMany<K, V> {
    _kv: PhantomData<(K, V)>,
    map: IntMap<u32, RoaringBitmap>,
    none: RoaringBitmap,
}

impl<K, V> IntOneToMany<K, V> {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            _kv: PhantomData,
            map: IntMap::with_capacity_and_hasher(capacity, Default::default()),
            none: RoaringBitmap::default(),
        }
    }

    #[inline]
    pub fn contains(&self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.contains_impl(usize::from(key), usize::from(value))
    }

    fn contains_impl(&self, key: usize, value: usize) -> bool {
        self.map
            .get(&(key as u32))
            .is_some_and(|set| set.contains(value as u32))
    }

    #[inline]
    pub fn get(&self, key: K) -> &Set<V>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.get_impl(usize::from(key)))
    }

    fn get_impl(&self, key: usize) -> &RoaringBitmap {
        self.map.get(&(key as u32)).unwrap_or_else(empty)
    }

    #[inline]
    pub fn get_opt(&self, key: Option<K>) -> &Set<V>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.get_opt_impl(key.map(usize::from)))
    }

    fn get_opt_impl(&self, key: Option<usize>) -> &RoaringBitmap {
        match key {
            Some(key) => self.map.get(&(key as u32)).unwrap_or_else(empty),
            None => &self.none,
        }
    }

    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.insert_impl(usize::from(key), usize::from(value))
    }

    #[inline]
    pub fn insert_opt(&mut self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.insert_impl(usize::from(key), usize::from(value))
    }

    fn insert_impl(&mut self, key: usize, value: usize) -> bool {
        self.map.entry(key as u32).or_default().insert(value as u32)
    }

    pub fn insert_none(&mut self, value: V) -> bool
    where
        usize: From<V>,
    {
        self.none.insert(usize::from(value) as u32)
    }

    #[inline]
    pub fn none(&self) -> &Set<V> {
        Set::from_roaring_bitmap_ref(&self.none)
    }

    #[inline]
    pub fn union(&mut self, key: K, set: &Set<V>)
    where
        usize: From<K>,
    {
        self.union_impl(usize::from(key), &set.0);
    }

    fn union_impl(&mut self, key: usize, set: &RoaringBitmap) {
        if !set.is_empty() {
            *self.map.entry(key as u32).or_default() |= set;
        }
    }

    #[inline]
    pub fn union_none(&mut self, set: &Set<V>) {
        self.none |= &set.0;
    }
}

impl<K, V> FromIterator<(K, V)> for IntOneToMany<K, V>
where
    usize: From<K> + From<V>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::default();

        for (key, value) in iter {
            map.insert(key, value);
        }

        map
    }
}

impl<K, V> FromIterator<(Option<K>, V)> for IntOneToMany<K, V>
where
    usize: From<K> + From<V>,
{
    fn from_iter<I: IntoIterator<Item = (Option<K>, V)>>(iter: I) -> Self {
        let mut map = Self::default();

        for (key, value) in iter {
            match key {
                Some(key) => map.insert(key, value),
                None => map.insert_none(value),
            };
        }

        map
    }
}

impl<K, V> Default for IntOneToMany<K, V> {
    #[inline]
    fn default() -> Self {
        Self {
            _kv: PhantomData,
            map: IntMap::default(),
            none: RoaringBitmap::new(),
        }
    }
}

impl<K, V> Gc for IntOneToMany<K, V> {}
