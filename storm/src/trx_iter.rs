use crate::{Entity, EntityAccessor, Get, LogAccessor, LogState, TblTransaction};
use fxhash::FxHashMap;
use std::hash::Hash;

impl<'c, E> IntoIterator for &'c TblTransaction<'_, '_, E>
where
    E: EntityAccessor + LogAccessor,
    E::Key: Eq + Hash,
    &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)> + Get<E>,
{
    type Item = (&'c E::Key, &'c E);
    type IntoIter = TrxIter<'c, E, <&'c E::Tbl as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        match self.log() {
            Some(log) => TrxIter::Log {
                log: &log.changes,
                log_iter: log.changes.iter(),
                tbl_iter: self.tbl().into_iter(),
            },
            None => TrxIter::Tbl(self.tbl().into_iter()),
        }
    }
}

pub enum TrxIter<'a, E: Entity, I> {
    Log {
        log: &'a FxHashMap<E::Key, LogState<E>>,
        log_iter: std::collections::hash_map::Iter<'a, E::Key, LogState<E>>,
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
                    if let LogState::Inserted(val) = item.1 {
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
