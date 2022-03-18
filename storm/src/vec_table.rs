use crate::{
    provider::LoadAll, Accessor, ApplyLog, BoxFuture, CtxTypeInfo, Deps, Entity, EntityAccessor,
    EntityOf, Gc, GcCtx, Get, GetMut, Init, Log, LogState, NotifyTag, Result, Tag, TblVar,
};
use rayon::iter::IntoParallelIterator;
use std::ops::Deref;
use tracing::instrument;
use vec_map::{Iter, Keys, ParIter, Values, VecMap};
use version_tag::VersionTag;

pub struct VecTable<E: Entity> {
    map: VecMap<E::Key, E>,
    tag: VersionTag,
}

impl<E: Entity> VecTable<E> {
    pub fn new() -> Self {
        Self {
            map: VecMap::new(),
            tag: VersionTag::new(),
        }
    }

    #[inline]
    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
    }

    #[inline]
    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    fn update_metrics(&self)
    where
        E: CtxTypeInfo,
    {
        #[cfg(feature = "telemetry")]
        {
            use conv::prelude::ValueFrom;
            metrics::gauge!("storm.table.rows", f64::value_from(self.len()).unwrap_or(0.0), "type" => E::NAME);
        }
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
    fn var() -> &'static TblVar<Self> {
        E::entity_var()
    }

    #[inline]
    fn deps() -> &'static Deps {
        E::entity_deps()
    }
}

impl<E> ApplyLog<Log<E>> for VecTable<E>
where
    E: CtxTypeInfo + Entity,
    E::Key: Clone + Into<usize>,
{
    fn apply_log(&mut self, log: Log<E>) -> bool {
        if log.is_empty() {
            return false;
        }

        for (k, state) in log {
            match state {
                LogState::Inserted(v) => {
                    self.map.insert(k, v);
                }
                LogState::Removed => {
                    self.map.remove(&k);
                }
            }
        }

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
    E::Key: Into<usize>,
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
    E::Key: From<usize>,
{
    const SUPPORT_GC: bool = E::SUPPORT_GC;

    #[instrument(level = "debug", fields(name = <E as CtxTypeInfo>::NAME, obj = crate::OBJ_TABLE), skip_all)]
    #[inline]
    fn gc(&mut self, ctx: &GcCtx) {
        self.map.gc(ctx);
    }
}

impl<E: Entity> Get<E> for VecTable<E>
where
    E::Key: Clone + Into<usize>,
{
    #[inline]
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.map.get(k)
    }
}

impl<E: Entity> GetMut<E> for VecTable<E>
where
    E::Key: Clone + Into<usize>,
{
    #[inline]
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<'a, P, E> Init<'a, P> for VecTable<E>
where
    E: CtxTypeInfo + Entity + Send,
    E::Key: Into<usize> + Send,
    P: Sync + LoadAll<E, (), Self>,
{
    #[inline]
    fn init(provider: &'a P) -> BoxFuture<'a, Result<Self>> {
        provider.load_all(&())
    }
}

impl<'a, E: Entity> IntoIterator for &'a VecTable<E>
where
    E::Key: From<usize>,
{
    type Item = (E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E: Entity> IntoParallelIterator for &'a VecTable<E>
where
    E::Key: From<usize>,
{
    type Item = (E::Key, &'a E);
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
