use crate::{Entity, EntityAccessor, Get, TblTransaction};
use fxhash::FxHashMap;
use std::hash::Hash;

impl<'c, E> IntoIterator for &'c TblTransaction<'_, '_, E>
where
    E: EntityAccessor,
    E::Key: Eq + Hash,
    &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)> + Get<E>,
{
    type Item = (&'c E::Key, &'c E);
    type IntoIter = TrxIter<'c, E, <&'c E::Tbl as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        match self.ctx.logs.get(E::tbl_var()) {
            Some(log) => TrxIter::Log {
                log,
                log_iter: log.iter(),
                tbl_iter: self.tbl().into_iter(),
            },
            None => TrxIter::Tbl(self.tbl().into_iter()),
        }
    }
}

pub enum TrxIter<'a, E: Entity, I> {
    Log {
        log: &'a FxHashMap<E::Key, Option<E>>,
        log_iter: std::collections::hash_map::Iter<'a, E::Key, Option<E>>,
        tbl_iter: I,
    },
    Tbl(I),
}

impl<'a, E, I> Iterator for TrxIter<'a, E, I>
where
    E: Entity,
    E::Key: Eq + Hash,
    I: Iterator<Item = (&'a E::Key, &'a E)>,
{
    type Item = (&'a E::Key, &'a E);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Log {
                log,
                log_iter,
                tbl_iter,
            } => {
                for item in log_iter.by_ref() {
                    if let Some(val) = item.1 {
                        return Some((item.0, val));
                    }
                }

                for item in tbl_iter.by_ref() {
                    if !log.contains_key(item.0) {
                        return Some(item);
                    }
                }

                None
            }

            Self::Tbl(iter) => iter.next(),
        }
    }
}

pub struct TblChangedIter<'a, E: EntityAccessor> {
    pub(crate) log_iter: Option<std::collections::hash_map::Iter<'a, E::Key, Option<E>>>,
    pub(crate) tbl: Option<&'a E::Tbl>,
}

impl<'a, E: EntityAccessor> Iterator for TblChangedIter<'a, E> {
    type Item = (&'a E::Key, Option<&'a E>, Option<&'a E>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (k, new) = self.log_iter.as_mut()?.next()?;
        let old = self.tbl.and_then(|t| t.get(k));

        Some((k, old, new.as_ref()))
    }
}
