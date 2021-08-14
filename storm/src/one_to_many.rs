use fxhash::FxHashSet;
use std::{cmp::Ordering, hash::Hash, iter::FromIterator, ops::Index};
use vec_map::VecMap;

pub struct OneToMany<ONE, MANY>(VecMap<ONE, Box<[MANY]>>);

impl<ONE, MANY> OneToMany<ONE, MANY> {
    pub fn iter(&self) -> vec_map::Iter<ONE, Box<[MANY]>> {
        self.0.iter()
    }
}

impl<'a, ONE, MANY> IntoIterator for &'a OneToMany<ONE, MANY>
where
    ONE: From<usize>,
{
    type Item = (ONE, &'a Box<[MANY]>);
    type IntoIter = vec_map::Iter<'a, ONE, Box<[MANY]>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<ONE, MANY> Index<&ONE> for OneToMany<ONE, MANY>
where
    ONE: Clone + Into<usize>,
{
    type Output = [MANY];

    #[inline]
    fn index(&self, index: &ONE) -> &Self::Output {
        self.0.get(index).map(|v| &**v).unwrap_or(&[])
    }
}

impl<ONE, MANY> FromIterator<(ONE, MANY)> for OneToMany<ONE, MANY>
where
    ONE: Clone + From<usize> + Into<usize>,
    MANY: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (ONE, MANY)>>(iter: T) -> Self {
        let mut map = VecMap::new();

        for (one, many) in iter {
            map.entry(one).or_insert_with(Vec::default).push(many);
        }

        Self(
            map.into_iter()
                .map(|(one, many)| (one, many.into_boxed_slice()))
                .collect(),
        )
    }
}

pub trait OneToManyFromIter<ONE, MANY>: IntoIterator<Item = (ONE, MANY)> + Sized
where
    ONE: Clone + From<usize> + Into<usize>,
{
    /// Collect and dedup items using an FxHashSet internally.
    fn collect_dedup(self) -> OneToMany<ONE, MANY>
    where
        MANY: Eq + Hash,
    {
        let mut map = VecMap::new();

        for (one, many) in self {
            map.entry(one)
                .or_insert_with(FxHashSet::default)
                .insert(many);
        }

        OneToMany(
            map.into_iter()
                .map(|(one, many)| {
                    let many = many.into_iter().collect::<Vec<_>>();
                    (one, many.into_boxed_slice())
                })
                .collect(),
        )
    }

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
        let mut map = VecMap::new();

        for (one, many) in self {
            map.entry(one).or_insert_with(Vec::default).push(many);
        }

        OneToMany(
            map.into_iter()
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
        let mut map = VecMap::new();

        for (one, many) in self {
            map.entry(one).or_insert_with(Vec::default).push(many);
        }

        OneToMany(
            map.into_iter()
                .map(|(one, mut many)| {
                    many.sort_unstable_by(&cmp);
                    many.dedup_by(|a, b| cmp(a, b) == Ordering::Equal);
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
    ONE: Clone + From<usize> + Into<usize>,
{
}
