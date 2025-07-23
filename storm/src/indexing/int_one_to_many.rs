use crate::Gc;
use fast_set::{IntSet, OneU32ManyU32, OneU32ManyU32Log};
use std::{borrow::Cow, marker::PhantomData};

pub struct IntOneToMany<K, V> {
    _kv: PhantomData<(K, V)>,
    map: OneU32ManyU32,
}

impl<K, V> IntOneToMany<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            _kv: PhantomData,
            map: OneU32ManyU32::new(),
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            _kv: PhantomData,
            map: OneU32ManyU32::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn contains(&self, key: K, value: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.map
            .contains(usize::from(key) as u32, usize::from(value) as u32)
    }

    #[inline]
    pub fn get(&self, key: K) -> &IntSet<V>
    where
        usize: From<K> + From<V>,
    {
        let b = self.map.get(usize::from(key) as u32);
        unsafe { IntSet::from_bitmap_ref(b) }
    }

    pub fn get_opt(&self, key: Option<K>) -> &IntSet<V>
    where
        usize: From<K> + From<V>,
    {
        match key {
            Some(key) => self.get(key),
            None => self.none(),
        }
    }

    #[inline]
    pub fn none(&self) -> &IntSet<V>
    where
        usize: From<V>,
    {
        let b = self.map.none();
        unsafe { IntSet::from_bitmap_ref(b) }
    }
}

impl<K, V> FromIterator<(K, V)> for IntOneToMany<K, V>
where
    usize: From<K> + From<V>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut builder = IntOneToManyBuilder::new();

        for (key, value) in iter {
            builder.insert(key, value);
        }

        builder.build()
    }
}

impl<K, V> FromIterator<(Option<K>, V)> for IntOneToMany<K, V>
where
    usize: From<K> + From<V>,
{
    fn from_iter<I: IntoIterator<Item = (Option<K>, V)>>(iter: I) -> Self {
        let mut builder = IntOneToManyBuilder::new();

        for (key, value) in iter {
            match key {
                Some(key) => builder.insert(key, value),
                None => builder.insert_none(value),
            };
        }

        builder.build()
    }
}

impl<K, V> Default for IntOneToMany<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Gc for IntOneToMany<K, V> {}

pub struct IntOneToManyBuilder<K, V> {
    commit: OneU32ManyU32,
    log: OneU32ManyU32Log,
    _kv: PhantomData<(K, V)>,
}

impl<K, V> IntOneToManyBuilder<K, V> {
    pub fn new() -> Self {
        Self {
            commit: Default::default(),
            log: Default::default(),
            _kv: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            commit: OneU32ManyU32::with_capacity(capacity),
            log: OneU32ManyU32Log::with_capacity(capacity),
            _kv: PhantomData,
        }
    }

    pub fn build(mut self) -> IntOneToMany<K, V> {
        self.commit.apply(self.log);

        IntOneToMany {
            _kv: PhantomData,
            map: self.commit,
        }
    }

    #[inline]
    pub fn difference<B>(&mut self, key: K, set: &IntSet<V>)
    where
        usize: From<K> + From<V>,
    {
        self.log
            .difference(&self.commit, usize::from(key) as u32, set.as_bitmap());
    }

    #[inline]
    pub fn difference_none<B>(&mut self, set: &IntSet<V>)
    where
        usize: From<V>,
    {
        self.log.difference_none(&self.commit, set.as_bitmap());
    }

    #[inline]
    pub fn insert(&mut self, key: K, val: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.log.insert(
            &self.commit,
            usize::from(key) as u32,
            usize::from(val) as u32,
        )
    }

    #[inline]
    pub fn insert_none(&mut self, val: V) -> bool
    where
        usize: From<V>,
    {
        self.log.insert_none(&self.commit, usize::from(val) as u32)
    }

    #[inline]
    pub fn intersection<B>(&mut self, key: K, set: &IntSet<V>)
    where
        for<'a> Cow<'a, IntSet<V>>: From<B>,
        usize: From<K> + From<V>,
    {
        self.log
            .intersection(&self.commit, usize::from(key) as u32, set.as_bitmap());
    }

    #[inline]
    pub fn intersection_none<B>(&mut self, set: &IntSet<V>)
    where
        for<'a> Cow<'a, IntSet<V>>: From<B>,
        usize: From<V>,
    {
        self.log.intersection_none(&self.commit, set.as_bitmap());
    }

    #[inline]
    pub fn remove(&mut self, key: K, val: V) -> bool
    where
        usize: From<K> + From<V>,
    {
        self.log.remove(
            &self.commit,
            usize::from(key) as u32,
            usize::from(val) as u32,
        )
    }

    #[inline]
    pub fn remove_none(&mut self, val: V) -> bool
    where
        usize: From<V>,
    {
        self.log.remove_none(&self.commit, usize::from(val) as u32)
    }

    #[inline]
    pub fn union(&mut self, key: K, set: &IntSet<V>)
    where
        usize: From<K> + From<V>,
    {
        self.log
            .union(&self.commit, usize::from(key) as u32, set.as_bitmap());
    }

    #[inline]
    pub fn union_none<B>(&mut self, set: &IntSet<V>)
    where
        usize: From<V>,
    {
        self.log.union_none(&self.commit, set.as_bitmap());
    }
}

impl<K, V> Default for IntOneToManyBuilder<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
