use crate::{log::LogToken, Gc, ObjTrxBase, Tag, Trx};
use fxhash::FxHashMap;
use std::{
    borrow::Cow,
    collections::hash_map,
    hash::Hash,
    mem::{take, transmute},
};
use vec_map::{Entry, VecMap};
use version_tag::VersionTag;

type Log<K, V> = FxHashMap<K, Vec<V>>;

pub struct VecOneMany<K, V> {
    map: VecMap<K, Box<[V]>>,
    tag: VersionTag,
}

impl<K, V> VecOneMany<K, V>
where
    K: Copy + Eq + Hash,
    usize: From<K>,
{
    pub fn contains_key(&self, key: &K) -> bool {
        !self.get(key).is_empty()
    }

    pub fn contains_key_value(&self, key: &K, value: &V) -> bool
    where
        V: Ord,
    {
        self.get(key).binary_search(value).is_ok()
    }

    pub fn get<'b>(&'b self, key: &K) -> &'b [V] {
        match self.map.get(key) {
            Some(set) => set,
            None => &[],
        }
    }
}

impl<K, V> Tag for VecOneMany<K, V> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<K, V> From<VecMap<K, Vec<V>>> for VecOneMany<K, V>
where
    K: Copy,
    V: Ord,
    usize: From<K>,
{
    fn from(mut map: VecMap<K, Vec<V>>) -> Self {
        map.values_mut().for_each(|v| v.sort_unstable());

        Self {
            map: map
                .into_iter()
                .map(|(k, v)| (k, v.into_boxed_slice()))
                .collect(),
            tag: VersionTag::new(),
        }
    }
}

impl<K, V> FromIterator<(K, V)> for VecOneMany<K, V>
where
    K: Copy + Eq + Hash,
    V: Ord,
    usize: From<K>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            map: create_vec_map(iter),
            tag: VersionTag::new(),
        }
    }
}

impl<K, V> Gc for VecOneMany<K, V>
where
    V: Gc,
{
    const SUPPORT_GC: bool = V::SUPPORT_GC;

    fn gc(&mut self) {
        self.map.values_mut().for_each(|v| v.gc());
    }
}

impl<K, V> ObjTrxBase for VecOneMany<K, V>
where
    K: Copy + Eq + Hash + Send + Sync + 'static,
    V: Clone + Gc + PartialEq + Send + Sync + 'static,
    usize: From<K>,
{
    type Log = Log<K, V>;
    type Trx<'a> = VecOneManyTrx<'a, K, V>;

    fn apply_log(&mut self, log: Self::Log) -> bool {
        let mut changed = false;

        for (key, vec) in log {
            match self.map.entry(key) {
                Entry::Occupied(mut o) => {
                    if vec.is_empty() {
                        o.remove();
                        changed = true;
                    } else if **o.get() != vec[..] {
                        o.insert(vec.into_boxed_slice());
                        changed = true;
                    }
                }
                Entry::Vacant(v) => {
                    if !vec.is_empty() {
                        v.insert(vec.into_boxed_slice());
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
        VecOneManyTrx {
            log_token,
            map: self,
            trx,
            upd: Default::default(),
        }
    }
}

pub struct VecOneManyTrx<'a, K: Clone + Sync, V: Clone + Sync> {
    map: &'a VecOneMany<K, V>,
    trx: &'a mut Trx<'a>,
    upd: FxHashMap<(Cow<'a, K>, Cow<'a, V>), Rec>,
    log_token: LogToken<Log<K, V>>,
}

impl<'a, K, V> VecOneManyTrx<'a, K, V>
where
    K: Copy + Eq + Hash + Sync + 'static,
    V: Clone + Sync,
    usize: From<K>,
{
    pub fn contains_key(&self, key: &K) -> bool {
        !self.get(key).is_empty()
    }

    pub fn contains_key_value(&self, key: &K, value: &V) -> bool
    where
        V: Ord,
    {
        self.get(key).binary_search(value).is_ok()
    }

    fn entry(&mut self, key: K) -> hash_map::Entry<K, Vec<V>> {
        self.log_mut().entry(key)
    }

    pub fn get<'c>(&'c self, key: &K) -> &'c [V] {
        self.trx
            .log
            .get(&self.log_token)
            .and_then(|map| map.get(key))
            .map_or_else(|| self.map.get(key), |vec| &vec[..])
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        V: Clone + Ord,
    {
        let map = self.map;

        match self.entry(key) {
            hash_map::Entry::Occupied(mut o) => {
                let vec = o.get_mut();

                if let Err(index) = vec.binary_search(&value) {
                    vec.insert(index, value);
                }
            }
            hash_map::Entry::Vacant(v) => {
                let vec = map.get(v.key());

                if let Err(index) = vec.binary_search(&value) {
                    let mut vec = vec.to_vec();
                    vec.insert(index, value);
                    v.insert(vec);
                }
            }
        }
    }

    pub fn insert_key(&mut self, key: K, mut vec: Vec<V>)
    where
        V: Ord,
    {
        vec.sort();
        vec.dedup();

        self.log_mut().insert(key, vec);
    }

    fn log_mut(&mut self) -> &mut Log<K, V> {
        self.trx.log.get_or_init_mut(&self.log_token)
    }

    pub fn remove_key(&mut self, key: K) {
        self.log_mut().insert(key, Vec::new());
    }

    pub fn remove_key_value(&mut self, key: K, value: &V)
    where
        V: Clone + Ord,
    {
        let map = self.map;

        match self.entry(key) {
            hash_map::Entry::Occupied(mut o) => {
                let vec = o.get_mut();

                if let Ok(index) = vec.binary_search(value) {
                    vec.remove(index);
                }
            }
            hash_map::Entry::Vacant(v) => {
                let slice = map.get(v.key());

                if let Ok(index) = slice.binary_search(value) {
                    let mut vec = slice.to_vec();
                    vec.remove(index);
                    v.insert(vec);
                }
            }
        }
    }
}

pub struct VecOneManyUpdater<'a, K, V>
where
    K: Copy + Eq + Hash + Sync + 'static,
    V: Clone + Eq + Hash + Ord + Sync + 'static,
    usize: From<K>,
{
    trx: &'a mut VecOneManyTrx<'a, K, V>,
    upd: FxHashMap<(Cow<'a, K>, Cow<'a, V>), Rec>,
}

#[derive(Clone, Copy, Default)]
struct Rec {
    add: bool,
    rem: bool,
}

impl<'a, K, V> VecOneManyUpdater<'a, K, V>
where
    K: Copy + Eq + Hash + Eq + Hash + Sync + 'static,
    V: Clone + Eq + Hash + Ord + Sync + 'static,
    usize: From<K>,
{
    pub fn insert(&mut self, one: Cow<'a, K>, many: Cow<'a, V>) {
        self.upd.entry((one, many)).or_default().add = true;
    }

    pub fn insert_iter<I, IK, IV>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (IK, IV)>,
        IK: Into<Cow<'a, K>>,
        IV: Into<Cow<'a, V>>,
    {
        for (one, many) in iter {
            self.insert(one.into(), many.into());
        }
    }

    pub fn remove(&mut self, one: Cow<'a, K>, many: Cow<'a, V>) {
        self.upd.entry((one, many)).or_default().rem = true;
    }

    pub fn remove_iter<I, IK, IV>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (IK, IV)>,
        IK: Into<Cow<'a, K>>,
        IV: Into<Cow<'a, V>>,
    {
        for (one, many) in iter {
            self.remove(one.into(), many.into());
        }
    }
}

impl<'a, K, V> Drop for VecOneManyUpdater<'a, K, V>
where
    K: Copy + Eq + Hash + Sync + 'static,
    V: Clone + Eq + Hash + Ord + Sync + 'static,
    usize: From<K>,
{
    fn drop(&mut self) {
        for ((one, many), rec) in self.upd.drain() {
            if rec.rem && !rec.add {
                self.trx.remove_key_value(*one, &many);
            }

            if rec.add && !rec.rem {
                self.trx.insert(*one, many.into_owned());
            }
        }

        self.trx.upd = coerce_hash_map_lifetime(take(&mut self.upd));
    }
}

fn create_vec_map<K, V, I>(iter: I) -> VecMap<K, Box<[V]>>
where
    K: Copy + Eq + Hash,
    I: IntoIterator<Item = (K, V)>,
    V: Ord,
    usize: From<K>,
{
    let mut map = Log::<K, V>::default();

    for (k, v) in iter {
        let vec = map.entry(k).or_default();

        if let Err(index) = vec.binary_search(&v) {
            vec.insert(index, v);
        }
    }

    map.into_iter()
        .map(|(k, v)| (k, v.into_boxed_slice()))
        .collect()
}

fn coerce_hash_map_lifetime<'a, 'b, K, V>(
    map: FxHashMap<(Cow<'a, K>, Cow<'a, V>), Rec>,
) -> FxHashMap<(Cow<'b, K>, Cow<'b, V>), Rec>
where
    K: Clone,
    V: Clone,
{
    unsafe { transmute(map) }
}
