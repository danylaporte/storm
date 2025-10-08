use crate::{
    provider::LoadAll, AsRefAsync, BoxFuture, Ctx, CtxLocks, CtxTransaction, CtxTypeInfo, CtxVar,
    Entity, EntityAccessor, Get, LogOf, Logs, NotifyTag, ProviderContainer, RefIntoIterator,
    Result, Tag, Touchable, TouchedEvent, __register_apply, indexing::AsyncAsIdxTrx, ClearEvent,
    Clearable,
};
use fast_set::flat_set_index;
use fxhash::FxHashSet;
use std::{any::type_name, future::ready, hash::Hash, marker::PhantomData, mem::take, ops::Deref};
use version_tag::VersionTag;

impl<A: FlatSetAdapt> AsRefAsync<FlatSetIndex<A>> for Ctx
where
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ FlatSetIndex<A>>> {
        A::get_or_init(self)
    }
}

impl<A: FlatSetAdapt, L> AsRef<FlatSetIndex<A>> for CtxLocks<'_, L>
where
    L: AsRef<<A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref(&self) -> &FlatSetIndex<A> {
        A::get_or_init_sync(self.ctx, self.locks.as_ref())
    }
}

pub struct FlatSetIndex<A: FlatSetAdapt> {
    index: flat_set_index::FlatSetIndex<A::K, A::V>,
    tag: VersionTag,
    _a: PhantomData<A>,
}

impl<A: FlatSetAdapt> FlatSetIndex<A> {
    #[inline]
    fn apply(&mut self, log: flat_set_index::FlatSetIndexLog<A::K, A::V>) -> bool {
        self.index.apply(log)
    }
}

impl<A: FlatSetAdapt> Default for FlatSetIndex<A> {
    #[inline]
    fn default() -> Self {
        Self {
            index: Default::default(),
            tag: VersionTag::new(),
            _a: PhantomData,
        }
    }
}

impl<A: FlatSetAdapt> Deref for FlatSetIndex<A> {
    type Target = flat_set_index::FlatSetIndex<A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.index
    }
}

impl<A> AsyncAsIdxTrx for FlatSetIndex<A>
where
    A: FlatSetAdapt,
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    type Trx<'a> = FlatSetIndexTrx<'a, A>;

    fn async_as_idx_trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            A::get_or_init(trx.ctx).await?;

            // extract the index log and init if required.
            let (base, log) =
                A::base_and_log(trx.ctx, &mut trx.logs).expect("extract base and log");

            Ok(FlatSetIndexTrx(flat_set_index::FlatSetIndexTrx::new(
                base, log,
            )))
        })
    }
}

type HashSet<A> = FxHashSet<(Option<<A as FlatSetAdapt>::K>, <A as FlatSetAdapt>::V)>;

pub type BaseAndLog<'a, 'b, A> = Option<(
    &'a FlatSetIndex<A>,
    &'b mut flat_set_index::FlatSetIndexLog<<A as FlatSetAdapt>::K, <A as FlatSetAdapt>::V>,
)>;

pub trait FlatSetAdapt: Clearable + Send + Sized + Sync + Touchable + 'static {
    type Entity: EntityAccessor + CtxTypeInfo + Send;
    type K: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;
    type V: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;

    fn adapt(id: &<Self::Entity as Entity>::Key, entity: &Self::Entity, out: &mut HashSet<Self>);

    fn index_var() -> CtxVar<FlatSetIndex<Self>>;

    fn apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool {
        let Some((_, log)) = Self::base_and_log(ctx, logs) else {
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

    fn base_and_log<'a, 'b>(ctx: &'a Ctx, logs: &'b mut Logs) -> BaseAndLog<'a, 'b, Self> {
        let index_var = Self::index_var();
        let base = ctx.ctx_ext_obj.get(index_var).get()?;

        if !logs.contains(index_var) {
            let tbl_var = Self::Entity::tbl_var();
            let tbl_log = logs.get(tbl_var)?;
            let tbl = ctx.ctx_ext_obj.get(tbl_var).get()?;

            let mut log = flat_set_index::FlatSetIndexLog::default();

            let mut old_set = FxHashSet::default();
            let mut new_set = FxHashSet::default();

            for (k, new) in tbl_log {
                old_set.clear();
                new_set.clear();

                let old = tbl.get(k);

                Self::upsert_or_remove(
                    base,
                    &mut log,
                    k,
                    new.as_ref(),
                    old,
                    &mut old_set,
                    &mut new_set,
                );
            }

            logs.insert(index_var, log);
        }

        logs.get_mut(index_var).map(|log| (base, log))
    }

    fn get_or_init(ctx: &Ctx) -> BoxFuture<'_, Result<&FlatSetIndex<Self>>>
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
    ) -> &'a FlatSetIndex<Self> {
        let slot = ctx.ctx_ext_obj.get(Self::index_var());

        slot.get_or_init(|| {
            let mut base = fast_set::FlatSetIndex::<Self::K, Self::V>::default();
            let mut log = fast_set::FlatSetIndexLog::<Self::K, Self::V>::default();
            let mut set = FxHashSet::default();

            for (id, entity) in tbl.ref_iter() {
                set.clear();

                Self::adapt(id, entity, &mut set);

                for (k, v) in set.drain() {
                    match k {
                        Some(k) => {
                            log.insert(&base, k, v);
                        }
                        None => {
                            log.insert_none(&base, v);
                        }
                    }
                }
            }

            base.apply(log);

            FlatSetIndex {
                _a: PhantomData,
                index: base,
                tag: VersionTag::new(),
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
            if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs) {
                let mut old_set = FxHashSet::default();
                let mut new_set = FxHashSet::default();

                Self::upsert_or_remove(
                    base,
                    log,
                    id,
                    None,
                    Some(entity),
                    &mut old_set,
                    &mut new_set,
                );
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
                if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs) {
                    let mut old_set = FxHashSet::default();
                    let mut new_set = FxHashSet::default();

                    Self::upsert_or_remove(
                        base,
                        log,
                        id,
                        Some(new),
                        old,
                        &mut old_set,
                        &mut new_set,
                    );
                }
            }

            trx.logs.get_mut_or_default(tbl_var).insert(id.clone(), new);
        }

        Box::pin(ready(Ok(())))
    }

    fn register() {
        __register_apply(Self::apply_log, crate::ApplyOrder::FlatSet);
        <Self::Entity as EntityAccessor>::cleared().on(Self::handle_clear);
        <Self::Entity as EntityAccessor>::removed().on(Self::handle_removed);
        <Self::Entity as EntityAccessor>::upserted().on(Self::handle_upserted);
    }

    fn upsert_or_remove(
        base: &FlatSetIndex<Self>,
        log: &mut flat_set_index::FlatSetIndexLog<Self::K, Self::V>,
        key: &<Self::Entity as Entity>::Key,
        new: Option<&Self::Entity>,
        old: Option<&Self::Entity>,
        old_set: &mut HashSet<Self>,
        new_set: &mut HashSet<Self>,
    ) {
        if let Some(new) = new {
            Self::adapt(key, new, new_set);
        }

        if let Some(old) = old {
            Self::adapt(key, old, old_set);
        }

        if old_set != new_set {
            for (k, v) in &*old_set - &*new_set {
                match k {
                    Some(k) => {
                        log.remove(&base.index, k, v);
                    }
                    None => {
                        log.remove_none(&base.index, v);
                    }
                }
            }

            for (k, v) in &*new_set - &*old_set {
                match k {
                    Some(k) => {
                        log.insert(&base.index, k, v);
                    }
                    None => {
                        log.insert_none(&base.index, v);
                    }
                }
            }
        }
    }
}

impl<A: FlatSetAdapt> Clearable for FlatSetIndex<A> {
    #[inline]
    fn cleared() -> &'static ClearEvent {
        A::cleared()
    }
}

impl<A: FlatSetAdapt> LogOf for FlatSetIndex<A> {
    type Log = flat_set_index::FlatSetIndexLog<A::K, A::V>;
}

impl<A: FlatSetAdapt> NotifyTag for FlatSetIndex<A> {
    #[inline]
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<A: FlatSetAdapt> Tag for FlatSetIndex<A> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<A: FlatSetAdapt> Touchable for FlatSetIndex<A> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        A::touched()
    }
}

pub struct FlatSetIndexTrx<'a, A: FlatSetAdapt>(flat_set_index::FlatSetIndexTrx<'a, A::K, A::V>);

impl<'a, A: FlatSetAdapt> Deref for FlatSetIndexTrx<'a, A> {
    type Target = flat_set_index::FlatSetIndexTrx<'a, A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[macro_export]
macro_rules! flat_set_adapt {
    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<(Option<$k:ty>, $v:ty $(,)?)> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::FlatSetAdapt for $adapt {
            type Entity = $entity_ty;
            type K = $k;
            type V = $v;

            #[allow(unused_variables)]
            fn adapt(id: &<Self::Entity as storm::Entity>::Key, entity: &Self::Entity, set: &mut storm::fxhash::FxHashSet<(Option<Self::K>, Self::V)>) {
                fn f($id: &$entity_key, $entity: &$entity_ty) -> Option<(Option<$k>, $v)> {
                    $($t)*
                }

                if let Some((k, v)) = f(id, entity) {
                    set.insert((k, v));
                }
            }

            fn index_var() -> storm::CtxVar<storm::indexing::FlatSetIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<storm::indexing::FlatSetIndex<$adapt>>,
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

        $vis type $alias = storm::indexing::FlatSetIndex<$adapt>;

        #[storm::register]
        fn $init() {
            <$adapt as storm::indexing::FlatSetAdapt>::register();
        }
    };

    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty, $out:ident: &mut FxHashSet<(Option<$k:ty>, $v:ty $(,)?)> $(,)?) {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::FlatSetAdapt for $adapt {
            type Entity = $entity_ty;
            type K = $k;
            type V = $v;

            #[allow(unused_variables)]
            fn adapt($id: &<Self::Entity as storm::Entity>::Key, $entity: &Self::Entity, $out: &mut storm::fxhash::FxHashSet<(Option<Self::K>, Self::V)>) {
                $($t)*
            }

            fn index_var() -> storm::CtxVar<storm::indexing::FlatSetIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<storm::indexing::FlatSetIndex<$adapt>>,
                    },
                    crate_path = storm::extobj
                );

                *V
            }
        }

        impl storm::Clearable for $adapt {
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }
        }

        impl storm::Touchable for $adapt {
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }

        $vis type $alias = storm::indexing::FlatSetIndex<$adapt>;

        #[storm::register]
        fn $init() {
            <$adapt as storm::indexing::FlatSetAdapt>::register();
        }
    };
}
