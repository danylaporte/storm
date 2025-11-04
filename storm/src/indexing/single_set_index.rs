use crate::{
    provider::LoadAll, AsRefAsync, BoxFuture, Ctx, CtxLocks, CtxTransaction, CtxTypeInfo, CtxVar,
    Entity, EntityAccessor, Get, LogOf, Logs, NotifyTag, ProviderContainer, RefIntoIterator,
    Result, Tag, Touchable, TouchedEvent, __register_apply, indexing::AsyncAsIdxTrx, ClearEvent,
    Clearable,
};
use fast_set::IntSet;
use std::{any::type_name, future::ready, hash::Hash, marker::PhantomData, mem::take, ops::Deref};
use version_tag::VersionTag;

pub struct SingleSetLog<A: SingleSetAdapt> {
    set: Option<IntSet<A::K>>,
    _a: PhantomData<A>,
}

impl<A: SingleSetAdapt> SingleSetLog<A> {
    pub fn insert(&mut self, base: &SingleSetIndex<A>, key: A::K) {
        match self.set.as_mut() {
            Some(v) => {
                v.insert(key);
            }
            None => {
                if !base.index.contains(key) {
                    let mut set = base.index.clone();
                    set.insert(key);
                    self.set = Some(set);
                }
            }
        }
    }

    pub fn remove(&mut self, base: &SingleSetIndex<A>, key: A::K) {
        match self.set.as_mut() {
            Some(v) => {
                v.remove(key);
            }
            None => {
                if base.index.contains(key) {
                    let mut set = base.index.clone();
                    set.remove(key);
                    self.set = Some(set);
                }
            }
        }
    }
}

impl<A: SingleSetAdapt> Default for SingleSetLog<A> {
    fn default() -> Self {
        Self {
            set: Default::default(),
            _a: Default::default(),
        }
    }
}

impl<A: SingleSetAdapt> AsRefAsync<SingleSetIndex<A>> for Ctx
where
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ SingleSetIndex<A>>> {
        A::get_or_init(self)
    }
}

impl<A: SingleSetAdapt, L> AsRef<SingleSetIndex<A>> for CtxLocks<'_, L>
where
    L: AsRef<<A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref(&self) -> &SingleSetIndex<A> {
        A::get_or_init_sync(self.ctx, self.locks.as_ref())
    }
}

pub struct SingleSetIndex<A: SingleSetAdapt> {
    index: IntSet<A::K>,
    tag: VersionTag,
    _a: PhantomData<A>,
}

impl<A: SingleSetAdapt> SingleSetIndex<A> {
    #[inline]
    fn apply(&mut self, log: IntSet<A::K>) -> bool {
        let changed = self.index != log;

        if changed {
            self.index = log;
            self.tag.notify();
        }

        changed
    }
}

impl<A: SingleSetAdapt> Default for SingleSetIndex<A> {
    #[inline]
    fn default() -> Self {
        Self {
            index: Default::default(),
            tag: VersionTag::new(),
            _a: PhantomData,
        }
    }
}

impl<A: SingleSetAdapt> Deref for SingleSetIndex<A> {
    type Target = fast_set::IntSet<A::K>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<A: SingleSetAdapt> AsyncAsIdxTrx for SingleSetIndex<A>
where
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    type Trx<'a> = SingleSetIndexTrx<'a, A>;

    fn async_as_idx_trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            A::get_or_init(trx.ctx).await?;

            // extract the index log and init if required.
            let (_, log) =
                A::base_and_log(trx.ctx, &mut trx.logs, true).expect("extract base and log");

            Ok(SingleSetIndexTrx(log))
        })
    }
}

pub type BaseAndLog<'a, 'b, A> = Option<(&'a SingleSetIndex<A>, &'b mut SingleSetLog<A>)>;

pub trait SingleSetAdapt: Clearable + Send + Sized + Sync + Touchable + 'static {
    type Entity: EntityAccessor<Key = Self::K> + CtxTypeInfo + Send;
    type K: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;

    fn adapt(id: &<Self::Entity as Entity>::Key, entity: &Self::Entity) -> bool;

    fn index_var() -> CtxVar<SingleSetIndex<Self>>;

    fn apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool {
        let Some(log) = Self::base_and_log(ctx, logs, false).and_then(|l| l.1.set.as_mut()) else {
            return false;
        };

        let changed = ctx
            .ctx_ext_obj
            .get_mut(Self::index_var())
            .get_mut()
            .is_some_and(|idx| idx.apply(take(log)));

        if changed {
            Self::touched().call(ctx);
        }

        changed
    }

    fn base_and_log<'a, 'b>(
        ctx: &'a Ctx,
        logs: &'b mut Logs,
        force_log: bool,
    ) -> BaseAndLog<'a, 'b, Self> {
        let index_var = Self::index_var();
        let base = ctx.ctx_ext_obj.get(index_var).get()?;

        if !logs.contains(index_var) {
            let tbl_var = Self::Entity::tbl_var();

            if let Some(tbl_log) = logs.get(tbl_var) {
                let tbl = ctx.ctx_ext_obj.get(tbl_var).get().expect("tbl");
                let mut log = SingleSetLog::default();

                for (k, new) in tbl_log {
                    let new = new.as_ref().is_some_and(|new| Self::adapt(k, new));
                    let old = tbl.get(k).is_some_and(|old| Self::adapt(k, old));

                    if old != new {
                        if old {
                            log.remove(base, *k);
                        } else {
                            log.insert(base, *k);
                        }
                    }
                }

                logs.insert(index_var, log);
            } else if force_log {
                logs.insert(index_var, Default::default());
            }
        }

        logs.get_mut(index_var).map(|log| (base, log))
    }

    fn get_or_init(ctx: &Ctx) -> BoxFuture<'_, Result<&SingleSetIndex<Self>>>
    where
        Ctx: AsRefAsync<<Self::Entity as EntityAccessor>::Tbl>,
        ProviderContainer: LoadAll<Self::Entity, (), <Self::Entity as EntityAccessor>::Tbl>,
    {
        Box::pin(async move {
            let slot = ctx.ctx_ext_obj.get(Self::index_var());

            if let Some(idx) = slot.get() {
                return Ok(idx);
            }

            let tbl = ctx.tbl_of::<Self::Entity>().await?;

            if let Some(idx) = slot.get() {
                return Ok(idx);
            }

            let _gate = ctx.provider.gate(type_name::<Self>()).await;

            Ok(Self::get_or_init_sync(ctx, tbl))
        })
    }

    fn get_or_init_sync<'a>(
        ctx: &'a Ctx,
        tbl: &'a <Self::Entity as EntityAccessor>::Tbl,
    ) -> &'a SingleSetIndex<Self> {
        let slot = ctx.ctx_ext_obj.get(Self::index_var());

        slot.get_or_init(|| {
            #[cfg(feature = "telemetry")]
            let instant = std::time::Instant::now();

            let mut index = fast_set::IntSet::<Self::K>::default();

            for (id, entity) in tbl.ref_iter() {
                if Self::adapt(id, entity) {
                    index.insert(*id);
                }
            }

            #[cfg(feature = "telemetry")]
            {
                let dur = instant.elapsed().as_secs_f64();
                metrics::histogram!("index_build_dur_sec", "name" => type_name::<Self>())
                    .record(dur);
            }

            SingleSetIndex {
                index,
                tag: VersionTag::new(),
                _a: PhantomData,
            }
        })
    }

    fn handle_clear(ctx: &mut Ctx) {
        if ctx.ctx_ext_obj.get_mut(Self::index_var()).take().is_some() {
            Self::cleared().call(ctx);
        }
    }

    fn handle_removed<'a>(
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <Self::Entity as Entity>::Key,
        entity: &'a Self::Entity,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs, true) {
                if Self::adapt(id, entity) {
                    log.remove(base, *id);
                }
            }

            Ok(())
        })
    }

    fn handle_upserted<'a>(
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <Self::Entity as Entity>::Key,
        old: Option<&'a Self::Entity>,
    ) -> BoxFuture<'a, Result<()>> {
        let tbl_var = Self::Entity::tbl_var();

        // Because we cannot use 2 mut references of the log at the same time, we remove the new entity from the log
        // before updating the index.
        // We then reinsert it back to the log at the end.
        if let Some(new) = trx.logs.get_mut(tbl_var).and_then(|map| map.remove(id)) {
            if let Some(new) = new.as_ref() {
                if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs, true) {
                    let old = old.is_some_and(|old| Self::adapt(id, old));
                    let new = Self::adapt(id, new);

                    if old != new {
                        if old {
                            log.remove(base, *id);
                        } else {
                            log.insert(base, *id);
                        }
                    }
                }
            }

            trx.logs.get_mut_or_default(tbl_var).insert(*id, new);
        }

        Box::pin(ready(Ok(())))
    }

    fn register() {
        __register_apply(Self::apply_log, crate::ApplyOrder::FlatSet);
        <Self::Entity as EntityAccessor>::cleared().on(Self::handle_clear);
        <Self::Entity as EntityAccessor>::removed().on(Self::handle_removed);
        <Self::Entity as EntityAccessor>::upserted().on(Self::handle_upserted);
    }
}

impl<A: SingleSetAdapt> Clearable for SingleSetIndex<A> {
    #[inline]
    fn cleared() -> &'static ClearEvent {
        A::cleared()
    }
}

impl<A: SingleSetAdapt> LogOf for SingleSetIndex<A> {
    type Log = SingleSetLog<A>;
}

impl<A: SingleSetAdapt> NotifyTag for SingleSetIndex<A> {
    #[inline]
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<A: SingleSetAdapt> Tag for SingleSetIndex<A> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<A: SingleSetAdapt> Touchable for SingleSetIndex<A> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        A::touched()
    }
}

#[allow(dead_code)]
pub struct SingleSetIndexTrx<'a, A: SingleSetAdapt>(&'a SingleSetLog<A>);

// impl<'a, A: SingleSetAdapt> Deref for SingleSetIndexTrx<'a, A> {
//     type Target = IntSet<A::K>;

//     #[inline]
//     fn deref(&self) -> &Self::Target {
//         self.0
//     }
// }

#[macro_export]
macro_rules! single_set_adapt {
    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> bool {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::SingleSetAdapt for $adapt {
            type Entity = $entity_ty;
            type K = $entity_key;

            #[allow(unused_variables)]
            fn adapt($id: &<Self::Entity as storm::Entity>::Key, $entity: &Self::Entity) -> bool {
                $($t)*
            }

            fn index_var() -> storm::CtxVar<storm::indexing::SingleSetIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: storm::OnceCell<storm::indexing::SingleSetIndex<$adapt>>,
                    },
                    crate_path = storm::extobj
                );

                *V
            }
        }

        impl storm::Clearable for $adapt {
            #[inline]
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }
        }

        impl storm::Touchable for $adapt {
            #[inline]
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }

        $vis type $alias = storm::indexing::SingleSetIndex<$adapt>;

        #[storm::register]
        fn $init() {
            <$adapt as storm::indexing::SingleSetAdapt>::register();
        }
    };
}
