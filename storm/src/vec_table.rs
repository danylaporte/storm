use crate::{
    indexing::{Index, IndexList},
    on_changed::Changed,
    provider::LoadAll,
    Accessor, ApplyLog, BoxFuture, CtxTypeInfo, Deps, Entity, EntityAccessor, EntityOf, Gc, GcCtx,
    Get, GetMut, Init, Log, LogState, NotifyTag, Result, Tag, TblVar,
};
use rayon::iter::IntoParallelIterator;
use std::ops::Deref;
use vec_map::{Entry, Iter, Keys, ParIter, Values, VecMap};
use version_tag::VersionTag;

pub struct VecTable<E: Entity> {
    indexes: IndexList<E>,
    map: VecMap<E::Key, E>,
    tag: VersionTag,
}

impl<E: Entity> VecTable<E> {
    pub fn new() -> Self {
        Self {
            indexes: IndexList::new(),
            map: VecMap::new(),
            tag: VersionTag::new(),
        }
    }

    #[track_caller]
    #[inline]
    pub fn index<I>(&self) -> &I
    where
        I: Index<E> + 'static,
    {
        self.indexes.get::<I>().0
    }

    #[inline]
    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
    }

    #[inline]
    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    pub fn register_index<I>(&mut self, mut index: I)
    where
        I: Index<E> + 'static,
    {
        let mut log = index.create_log();

        for (k, v) in &self.map {
            index.upsert(&mut *log, k, v, None);
        }

        index.apply_log(log);
        self.indexes.register(index);
    }

    fn update_metrics(&self)
    where
        E: CtxTypeInfo,
    {
        #[cfg(feature = "telemetry")]
        crate::telemetry::update_storm_table_rows(self.len(), E::NAME);
    }

    #[inline]
    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E> Accessor for VecTable<E>
where
    E: Entity + EntityAccessor<Tbl = VecTable<E>>,
{
    #[inline]
    fn var() -> TblVar<Self> {
        E::entity_var()
    }

    #[inline]
    fn deps() -> &'static Deps {
        E::entity_deps()
    }
}

impl<E> ApplyLog<Log<E>> for VecTable<E>
where
    E: CtxTypeInfo + Entity + EntityAccessor,
    E::Key: Copy + Into<usize>,
{
    fn apply_log(&mut self, log: Log<E>) -> bool {
        if log.changes.is_empty() {
            return false;
        }

        for (k, state) in log.changes {
            match state {
                LogState::Inserted(new) => match self.map.entry(k) {
                    Entry::Occupied(mut o) => {
                        let entity = Changed::Inserted {
                            old: Some(o.get()),
                            new: &new,
                        };
                        E::on_changed().__call(o.key(), entity);
                        o.insert(new);
                    }
                    Entry::Vacant(v) => {
                        let entity = Changed::Inserted {
                            old: None,
                            new: &new,
                        };
                        E::on_changed().__call(v.key(), entity);
                        v.insert(new);
                    }
                },
                LogState::Removed => {
                    if let Some(old) = self.map.remove(&k) {
                        E::on_changed().__call(&k, Changed::Removed { old: &old });
                    }
                }
            }
        }

        self.indexes.apply_changes(log.indexes);
        self.update_metrics();
        self.tag.notify();
        true
    }
}

impl<E: Entity> AsRef<Self> for VecTable<E> {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<E: Entity> AsRef<IndexList<E>> for VecTable<E> {
    #[inline]
    fn as_ref(&self) -> &IndexList<E> {
        &self.indexes
    }
}

impl<E: Entity> Default for VecTable<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity> Deref for VecTable<E> {
    type Target = VecMap<E::Key, E>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<E: Entity> EntityOf for VecTable<E> {
    type Entity = E;
}

impl<E> Extend<(E::Key, E)> for VecTable<E>
where
    E: CtxTypeInfo + Entity,
    E::Key: Copy + Into<usize>,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (E::Key, E)>,
    {
        for (k, v) in iter {
            self.map.insert(k, v);
        }

        self.update_metrics();
    }
}

impl<E> Gc for VecTable<E>
where
    E: Entity + CtxTypeInfo + Gc,
    E::Key: Copy,
{
    const SUPPORT_GC: bool = E::SUPPORT_GC;

    #[inline]
    fn gc(&mut self, ctx: &GcCtx) {
        self.map.gc(ctx);
    }
}

impl<E: Entity> Get<E> for VecTable<E>
where
    E::Key: Copy + Into<usize>,
{
    #[inline]
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.map.get(k)
    }
}

impl<E: Entity> GetMut<E> for VecTable<E>
where
    E::Key: Copy + Into<usize>,
{
    #[inline]
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<'a, P, E> Init<'a, P> for VecTable<E>
where
    E: CtxTypeInfo + Entity + Send,
    E::Key: Copy + Into<usize> + Send,
    P: Sync + LoadAll<E, (), Self>,
{
    #[inline]
    fn init(provider: &'a P) -> BoxFuture<'a, Result<Self>> {
        provider.load_all(&())
    }
}

impl<'a, E: Entity> IntoIterator for &'a VecTable<E> {
    type Item = (&'a E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E: Entity> IntoParallelIterator for &'a VecTable<E>
where
    E::Key: Copy,
{
    type Item = (&'a E::Key, &'a E);
    type Iter = ParIter<'a, E::Key, E>;

    #[inline]
    fn into_par_iter(self) -> Self::Iter {
        self.map.par_iter()
    }
}

impl<E: Entity> NotifyTag for VecTable<E> {
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<E: Entity> Tag for VecTable<E> {
    fn tag(&self) -> VersionTag {
        self.tag
    }
}
