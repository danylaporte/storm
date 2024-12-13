use crate::{log::LogToken, Gc, ObjTrxBase, Tag, Trx};
use fxhash::FxHashMap;
use std::{
    borrow::Borrow,
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};
use version_tag::VersionTag;

type Log<K, V> = FxHashMap<K, Option<V>>;

pub struct HashOneOne<K, V> {
    map: FxHashMap<K, V>,
    tag: VersionTag,
}

impl<K, V> HashOneOne<K, V>
where
    K: Eq + Hash,
{
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
    {
        self.map.contains_key(key)
    }

    pub fn contains_key_value<Q>(&self, key: &Q, value: &V) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
        V: PartialEq,
    {
        self.get(key).map_or(false, |q| q == value)
    }

    #[inline]
    pub fn get<'b, Q>(&'b self, key: &Q) -> Option<&'b V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.map.get(key)
    }
}

impl<K, V> Tag for HashOneOne<K, V> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<K, V> From<FxHashMap<K, V>> for HashOneOne<K, V> {
    fn from(map: FxHashMap<K, V>) -> Self {
        Self {
            map,
            tag: VersionTag::new(),
        }
    }
}

impl<K, V> FromIterator<(K, V)> for HashOneOne<K, V>
where
    K: Eq + Hash,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            map: HashMap::from_iter(iter),
            tag: VersionTag::new(),
        }
    }
}

impl<K, V> Gc for HashOneOne<K, V>
where
    V: Gc,
{
    const SUPPORT_GC: bool = V::SUPPORT_GC;

    fn gc(&mut self) {
        self.map.values_mut().for_each(|v| v.gc());
    }
}

impl<K, V> ObjTrxBase for HashOneOne<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: PartialEq + Send + Sync + 'static,
{
    type Log = Log<K, V>;
    type Trx<'a> = HashOneOneTrx<'a, K, V>;

    fn apply_log(&mut self, log: Self::Log) -> bool {
        let mut changed = false;

        for (key, val) in log {
            match self.map.entry(key) {
                Entry::Occupied(mut o) => match val {
                    Some(val) => {
                        if *o.get() != val {
                            o.insert(val);
                            changed = true;
                        }
                    }
                    None => {
                        o.remove();
                        changed = true;
                    }
                },
                Entry::Vacant(v) => {
                    if let Some(val) = val {
                        v.insert(val);
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.tag.notify()
        }

        changed
    }

    #[inline]
    fn trx<'a>(&'a self, trx: &'a mut Trx<'a>, log_token: LogToken<Log<K, V>>) -> Self::Trx<'a> {
        HashOneOneTrx {
            log_token,
            map: self,
            trx,
        }
    }
}

pub struct HashOneOneTrx<'a, K, V> {
    map: &'a HashOneOne<K, V>,
    trx: &'a mut Trx<'a>,
    log_token: LogToken<Log<K, V>>,
}

impl<'a, K, V> HashOneOneTrx<'a, K, V>
where
    K: Eq + Hash,
{
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.get(key).is_some()
    }

    pub fn contains_key_value<Q>(&self, key: &Q, value: &V) -> bool
    where
        K: Borrow<Q>,
        V: PartialEq,
        Q: Eq + Hash,
    {
        self.get(key).map_or(false, |v| v == value)
    }

    fn entry(&mut self, key: K) -> Entry<K, Option<V>> {
        self.log_mut().entry(key)
    }

    pub fn get<'c, Q>(&'c self, key: &Q) -> Option<&'c V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let trx = self
            .trx
            .log
            .get(&self.log_token)
            .and_then(|map| map.get(key));

        match trx {
            Some(v) => v.as_ref(),
            None => self.map.get(key),
        }
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        V: PartialEq,
    {
        let map = self.map;

        match self.entry(key) {
            Entry::Occupied(mut o) => {
                o.insert(Some(value));
            }
            Entry::Vacant(v) => {
                let old = map.get(v.key());

                if old.map_or(true, |old| *old != value) {
                    v.insert(Some(value));
                }
            }
        }
    }

    fn log_mut(&mut self) -> &mut Log<K, V> {
        self.trx.log.get_or_init_mut(&self.log_token)
    }

    pub fn remove_key(&mut self, key: K) {
        let map = self.map;

        match self.entry(key) {
            Entry::Occupied(mut o) => {
                o.insert(None);
            }
            Entry::Vacant(v) => {
                if map.get(v.key()).is_some() {
                    v.insert(None);
                }
            }
        }
    }

    pub fn remove_key_value(&mut self, key: K, value: &V)
    where
        V: PartialEq,
    {
        let map = self.map;

        match self.entry(key) {
            Entry::Occupied(o) => {
                if o.get().as_ref().map_or(false, |old| old == value) {
                    o.remove();
                }
            }
            Entry::Vacant(v) => {
                if map.get(v.key()).map_or(false, |old| old == value) {
                    v.insert(None);
                }
            }
        }
    }
}
