use super::IndexLog;
use crate::{
    indexing::{Index, IndexTrx},
    Entity,
};
pub use fast_set::u32_hierarchy::{CycleError, HierarchyError};
use fast_set::{IntSet, IntSetTrx, U32Hierarchy, U32HierarchyLog};
use std::{any::Any, marker::PhantomData};

pub struct HierarchyIndex<K, A> {
    _ka: PhantomData<(K, A)>,
    base: U32Hierarchy,
}

impl<K, A> HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
{
    pub fn new() -> Self {
        Self {
            base: Default::default(),
            _ka: PhantomData,
        }
    }

    pub fn ancestors(&self, child: K) -> AncestorIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        let mut iter = self.ancestors_with_self(child);
        iter.next();
        iter
    }

    pub fn ancestors_with_self(&self, child: K) -> AncestorIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        AncestorIter {
            iter: self.base.ancestors_with_self(usize::from(child) as u32),
            _k: PhantomData,
        }
    }

    #[inline]
    pub fn children(&self, parent: K) -> &IntSet<K>
    where
        usize: From<K>,
    {
        let b = self.base.children(usize::from(parent) as u32);
        unsafe { IntSet::from_bitmap_ref(b) }
    }

    #[inline]
    pub fn cycles(&self) -> impl Iterator<Item = K> + '_
    where
        K: TryFrom<usize>,
    {
        self.base.cycles().copied().map(k_try_from)
    }

    #[inline]
    pub fn depth(&self, node: K) -> Result<usize, CycleError>
    where
        usize: From<K>,
    {
        self.base.depth(usize::from(node) as u32)
    }

    #[inline]
    pub fn descendants(&self, parent: K) -> &IntSet<K>
    where
        usize: From<K>,
    {
        let b = self.base.descendants(usize::from(parent) as u32);
        unsafe { IntSet::from_bitmap_ref(b) }
    }

    #[inline]
    pub fn has_cycle(&self, node: K) -> bool
    where
        usize: From<K>,
    {
        self.base.has_cycle(usize::from(node) as u32)
    }

    #[inline]
    pub fn is_descendant_of(&self, child: K, parent: K) -> bool
    where
        usize: From<K>,
    {
        self.base
            .is_descendant_of(usize::from(child) as u32, usize::from(parent) as u32)
    }

    #[inline]
    pub fn parent(&self, child: K) -> Option<K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        self.base.parent(usize::from(child) as u32).map(k_try_from)
    }

    fn upsert_impl(
        &self,
        log: &mut dyn IndexLog,
        old: Option<(u32, Option<u32>)>,
        new: Option<(u32, Option<u32>)>,
    ) {
        if old != new {
            let log = log_mut(log);

            if let Some(old) = old {
                log.remove(&self.base, old.0);
            }

            if let Some((child, parent)) = new {
                log.insert(&self.base, parent, child);
            }
        }
    }

    fn remove_impl(&self, log: &mut dyn IndexLog, old: Option<(u32, Option<u32>)>) {
        if let Some(old) = old {
            log_mut(log).remove(&self.base, old.0);
        }
    }
}

impl<K, A> Default for HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, A> FromIterator<(Option<K>, K)> for HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
    usize: From<K>,
{
    fn from_iter<T: IntoIterator<Item = (Option<K>, K)>>(iter: T) -> Self {
        let mut base = U32Hierarchy::new();
        let mut log = U32HierarchyLog::default();

        for (parent, child) in iter {
            log.insert(
                &base,
                parent.map(|p| usize::from(p) as u32),
                usize::from(child) as u32,
            );
        }

        base.apply(log);

        Self {
            _ka: PhantomData,
            base,
        }
    }
}

impl<E, K, A> Index<E> for HierarchyIndex<K, A>
where
    E: Entity,
    K: Send + Sync + 'static,
    A: HierarchyAdapt<E, K> + Send + Sync + 'static,
    usize: From<K>,
{
    fn apply_log(&mut self, log: Box<dyn IndexLog>) {
        let log = *Box::<dyn Any>::downcast(log).expect("U32HierarchyLog");
        self.base.apply(log);
    }

    fn create_log(&self) -> Box<dyn IndexLog> {
        Box::new(U32HierarchyLog::default())
    }

    fn upsert(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity,
    {
        let old = old.and_then(|old| adapt_u32::<A, E, K>(k, old));
        let new = adapt_u32::<A, E, K>(k, entity);

        self.upsert_impl(log, old, new);
    }

    fn remove(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E)
    where
        E: Entity,
    {
        let old = adapt_u32::<A, E, K>(k, entity);

        self.remove_impl(log, old);
    }
}

fn log_mut(log: &mut dyn IndexLog) -> &mut U32HierarchyLog {
    <dyn Any>::downcast_mut(&mut *log).expect("U32HierarchyLog")
}

fn log_ref(log: &dyn IndexLog) -> &U32HierarchyLog {
    <dyn Any>::downcast_ref(log).expect("U32HierarchyLog")
}

impl IndexLog for U32HierarchyLog {}

pub struct HierarchyTrx<'a, K, A> {
    _kv: PhantomData<(K, A)>,
    base: &'a U32Hierarchy,
    log: &'a U32HierarchyLog,
}

impl<K, A> HierarchyTrx<'_, K, A>
where
    K: 'static,
    A: 'static,
{
    /// Returns an iterator over ancestors, stops at cycle nodes
    pub fn ancestors(&self, child: K) -> AncestorTrxIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        let mut iter = self.ancestors_with_self(child);
        iter.next();
        iter
    }

    pub fn ancestors_with_self(&self, child: K) -> AncestorTrxIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        AncestorTrxIter {
            _k: PhantomData,
            iter: self
                .log
                .ancestors_with_self(self.base, usize::from(child) as u32),
        }
    }

    #[inline]
    pub fn children(&self, node: K) -> IntSetTrx<K>
    where
        usize: From<K>,
    {
        let b = self.log.children(self.base, usize::from(node) as u32);
        unsafe { IntSetTrx::from_adaptive_bitmap_trx(b) }
    }

    /// Get a list of node having a cycle.
    #[inline]
    pub fn cycles(&self) -> impl Iterator<Item = K> + '_
    where
        K: TryFrom<usize>,
    {
        self.log.cycles(self.base).iter().copied().map(k_try_from)
    }

    #[inline]
    pub fn depth(&self, node: K) -> Result<usize, CycleError>
    where
        usize: From<K>,
    {
        self.log.depth(self.base, usize::from(node) as u32)
    }

    #[inline]
    pub fn descendants(&self, parent: K) -> IntSetTrx<K>
    where
        usize: From<K>,
    {
        let b = self.log.descendants(self.base, usize::from(parent) as u32);
        unsafe { IntSetTrx::from_adaptive_bitmap_trx(b) }
    }

    #[inline]
    pub fn has_cycle(&self, id: K) -> bool
    where
        usize: From<K>,
    {
        self.log.has_cycle(self.base, usize::from(id) as u32)
    }

    #[inline]
    pub fn is_descendant_of(&self, child: K, parent: K) -> bool
    where
        usize: From<K>,
    {
        self.log.is_descendant_of(
            self.base,
            usize::from(child) as u32,
            usize::from(parent) as u32,
        )
    }

    #[inline]
    pub fn parent(&self, child: K) -> Option<K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        self.log
            .parent(self.base, usize::from(child) as u32)
            .map(k_try_from)
    }
}

impl<K, A> IndexTrx for HierarchyIndex<K, A> {
    type Trx<'a>
        = HierarchyTrx<'a, K, A>
    where
        Self: 'a;

    fn trx<'a>(&'a self, log: &'a dyn IndexLog) -> Self::Trx<'a> {
        HierarchyTrx {
            base: &self.base,
            log: log_ref(log),
            _kv: PhantomData,
        }
    }
}

pub struct AncestorIter<'a, K> {
    _k: PhantomData<K>,
    iter: fast_set::u32_hierarchy::AncestorIter<'a>,
}

impl<K> Iterator for AncestorIter<'_, K>
where
    K: TryFrom<usize>,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(k_try_from)
    }
}

pub struct AncestorTrxIter<'a, K> {
    iter: fast_set::u32_hierarchy::AncestorLogIter<'a>,
    _k: PhantomData<K>,
}

impl<K> Iterator for AncestorTrxIter<'_, K>
where
    K: TryFrom<usize>,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(k_try_from)
    }
}

fn adapt_u32<A, E, K>(k: &E::Key, e: &E) -> Option<(u32, Option<u32>)>
where
    A: HierarchyAdapt<E, K>,
    E: Entity,
    usize: From<K>,
{
    A::adapt(k, e).map(|(child, parent)| {
        (
            usize::from(child) as u32,
            parent.map(|p| usize::from(p) as u32),
        )
    })
}

fn k_try_from<K>(v: u32) -> K
where
    K: TryFrom<usize>,
{
    match K::try_from(v as usize) {
        Ok(v) => v,
        Err(_) => unreachable!("Cannot convert to K"),
    }
}

pub trait HierarchyAdapt<E: Entity, K> {
    fn adapt(k: &E::Key, v: &E) -> Option<(K, Option<K>)>;
}

#[macro_export]
macro_rules! hierarchy_adapt {
    ($adapt:ident, $alias:ident, $init:ident, $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<($k:ty, Option<$k1:ty>)> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::HierarchyAdapt<$entity_ty, $k> for $adapt {
            #[allow(unused_variables)]
            fn adapt($id: &$entity_key, $entity: &$entity_ty) -> Option<($k, Option<$k>)> {
                $($t)*
            }
        }

        $vis type $alias = storm::indexing::HierarchyIndex<$k, $adapt>;

        #[$crate::linkme::distributed_slice($crate::STORM_INITS)]
        #[linkme(crate = $crate::linkme)]
        fn $init() {
            <$entity_ty as $crate::EntityAccessor>::entity_inits().register(|tbl| {
                tbl.register_index($alias::new());
            });
        }
    };
}
