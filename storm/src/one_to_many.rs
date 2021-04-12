use fxhash::FxHashSet;
use std::{hash::Hash, iter::FromIterator, ops::Index};
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
            map.entry(one)
                .or_insert_with(FxHashSet::default)
                .insert(many);
        }

        Self(
            map.into_iter()
                .map(|(one, many)| {
                    let many = many.into_iter().collect::<Vec<_>>();
                    (one, many.into_boxed_slice())
                })
                .collect(),
        )
    }
}
