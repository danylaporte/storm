use crate::Gc;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::Index,
};
use vec_map::{Entry, VecMap};

pub struct OneToMany<ONE, MANY>(VecMap<ONE, Box<[MANY]>>);

impl<ONE, MANY> OneToMany<ONE, MANY> {
    pub fn get(&self, index: &ONE) -> &[MANY]
    where
        ONE: Copy + Into<usize>,
    {
        self.0.get(index).map_or(&[], |v| &**v)
    }

    pub fn iter(&self) -> vec_map::Iter<ONE, Box<[MANY]>> {
        self.0.iter()
    }
}

impl<ONE, MANY> Gc for OneToMany<ONE, MANY>
where
    ONE: Copy + Send,
    MANY: Gc + Send + Sync,
{
    const SUPPORT_GC: bool = MANY::SUPPORT_GC;

    fn gc(&mut self, ctx: &crate::GcCtx) {
        self.0.iter_mut().for_each(|(_, item)| item.gc(ctx));
    }
}

impl<'a, ONE, MANY> IntoIterator for &'a OneToMany<ONE, MANY> {
    type Item = (&'a ONE, &'a Box<[MANY]>);
    type IntoIter = vec_map::Iter<'a, ONE, Box<[MANY]>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<ONE, MANY> Index<&ONE> for OneToMany<ONE, MANY>
where
    ONE: Copy + Into<usize>,
{
    type Output = [MANY];

    #[inline]
    fn index(&self, index: &ONE) -> &Self::Output {
        self.get(index)
    }
}

impl<ONE, MANY, S> From<HashMap<ONE, Box<[MANY]>, S>> for OneToMany<ONE, MANY>
where
    ONE: Copy,
    usize: From<ONE>,
{
    fn from(m: HashMap<ONE, Box<[MANY]>, S>) -> Self {
        let mut vec = VecMap::with_capacity(m.len());

        for (k, v) in m {
            vec.insert(k, v);
        }

        Self(vec)
    }
}

impl<ONE, MANY, S, S2> From<HashMap<ONE, HashSet<MANY, S2>, S>> for OneToMany<ONE, MANY>
where
    ONE: Copy,
    usize: From<ONE>,
{
    fn from(m: HashMap<ONE, HashSet<MANY, S2>, S>) -> Self {
        let mut vec = VecMap::with_capacity(m.len());

        for (k, v) in m {
            vec.insert(k, v.into_iter().collect::<Vec<_>>().into_boxed_slice());
        }

        Self(vec)
    }
}

impl<ONE, MANY, S> From<HashMap<ONE, Vec<MANY>, S>> for OneToMany<ONE, MANY>
where
    ONE: Copy,
    usize: From<ONE>,
{
    fn from(m: HashMap<ONE, Vec<MANY>, S>) -> Self {
        let mut vec = VecMap::with_capacity(m.len());

        for (k, v) in m {
            vec.insert(k, v.into_boxed_slice());
        }

        Self(vec)
    }
}

impl<ONE, MANY> From<VecMap<ONE, Box<[MANY]>>> for OneToMany<ONE, MANY>
where
    ONE: Copy,
    usize: From<ONE>,
{
    fn from(m: VecMap<ONE, Box<[MANY]>>) -> Self {
        Self(m)
    }
}

impl<ONE, MANY> From<VecMap<ONE, Vec<MANY>>> for OneToMany<ONE, MANY>
where
    ONE: Copy,
    usize: From<ONE>,
{
    fn from(m: VecMap<ONE, Vec<MANY>>) -> Self {
        let mut vec = VecMap::with_capacity(m.len());

        for (k, v) in m {
            vec.insert(k, v.into_boxed_slice());
        }

        Self(vec)
    }
}

impl<ONE, MANY> FromIterator<(ONE, MANY)> for OneToMany<ONE, MANY>
where
    ONE: Copy + Into<usize>,
    MANY: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (ONE, MANY)>>(iter: T) -> Self {
        Self(
            collect_vec_map(iter.into_iter())
                .into_iter()
                .map(|(one, many)| (one, many.into_boxed_slice()))
                .collect(),
        )
    }
}

pub trait OneToManyFromIter<ONE, MANY>: IntoIterator<Item = (ONE, MANY)> + Sized
where
    ONE: Copy + Into<usize>,
{
    /// Collect the items and sort them.
    fn collect_sort<F>(self) -> OneToMany<ONE, MANY>
    where
        MANY: Ord,
    {
        self.collect_sort_by(|a, b| a.cmp(b))
    }

    /// Collect the items and sort them by the cmp function specified.
    fn collect_sort_by<F>(self, cmp: F) -> OneToMany<ONE, MANY>
    where
        F: Fn(&MANY, &MANY) -> Ordering,
    {
        OneToMany(
            collect_vec_map(self.into_iter())
                .into_iter()
                .map(|(one, mut many)| {
                    many.sort_unstable_by(&cmp);
                    (one, many.into_boxed_slice())
                })
                .collect(),
        )
    }

    /// Collect the items and sort them using a key_cmp function.
    fn collect_sort_by_key<F, K>(self, key_cmp: F) -> OneToMany<ONE, MANY>
    where
        F: Fn(&MANY) -> K,
        K: Ord,
    {
        self.collect_sort_by(|a, b| key_cmp(a).cmp(&key_cmp(b)))
    }

    fn collect_sort_dedup(self) -> OneToMany<ONE, MANY>
    where
        MANY: Ord,
    {
        self.collect_sort_dedup_by(|a, b| a.cmp(b))
    }

    /// Collect the items and sort them by the cmp function specified.
    /// Also removes duplicate MANY items using the same cmp function.
    fn collect_sort_dedup_by<F>(self, cmp: F) -> OneToMany<ONE, MANY>
    where
        F: Fn(&MANY, &MANY) -> Ordering,
    {
        OneToMany(
            collect_vec_map(self.into_iter())
                .into_iter()
                .map(|(one, mut many)| {
                    many.sort_unstable_by(&cmp);
                    many.dedup_by(|a, b| cmp(a, b).is_eq());
                    (one, many.into_boxed_slice())
                })
                .collect(),
        )
    }

    /// Collect the items and sort them by the key_cmp function specified.
    /// Also removes duplicate MANY items using the same key_cmp function.
    fn collect_sort_dedup_by_key<F, K>(self, key_cmp: F) -> OneToMany<ONE, MANY>
    where
        F: Fn(&MANY) -> K,
        K: Ord,
    {
        self.collect_sort_dedup_by(|a, b| key_cmp(a).cmp(&key_cmp(b)))
    }
}

impl<ONE, MANY, T> OneToManyFromIter<ONE, MANY> for T
where
    T: IntoIterator<Item = (ONE, MANY)> + Sized,
    ONE: Copy + Into<usize>,
{
}

fn collect_vec_map<ONE, MANY, I>(iter: I) -> VecMap<ONE, Vec<MANY>>
where
    I: Iterator<Item = (ONE, MANY)>,
    ONE: Copy + Into<usize>,
{
    iter.fold(VecMap::<ONE, Vec<MANY>>::new(), |mut map, (one, many)| {
        match map.entry(one) {
            Entry::Occupied(mut o) => {
                o.get_mut().push(many);
            }
            Entry::Vacant(v) => {
                v.insert(vec![many]);
            }
        }

        map
    })
}
