use crate::{
    provider::LoadAll, AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, CtxVar, Entity,
    EntityAccessor, LogOf, Logs, ProviderContainer, RefIntoIterator, Result, Table, Touchable,
    TouchedEvent, TrxOf, __register_apply,
};
use fast_set::flat_set_index;
use fxhash::FxHashSet;
use std::{future::ready, hash::Hash, marker::PhantomData, mem::take};

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

pub struct FlatSetIndex<A: FlatSetAdapt> {
    pub kv: flat_set_index::FlatSetIndex<A::K, A::V>,
    pub vk: flat_set_index::FlatSetIndex<A::V, A::K>,
    _a: PhantomData<A>,
}

impl<A: FlatSetAdapt> FlatSetIndex<A> {
    fn apply(&mut self, log: FlatSetIndexLog<A::K, A::V>) -> bool {
        let mut changed = false;

        changed |= self.kv.apply(log.0);
        changed |= self.vk.apply(log.1);

        changed
    }
}

impl<A: FlatSetAdapt> Default for FlatSetIndex<A> {
    #[inline]
    fn default() -> Self {
        Self {
            kv: Default::default(),
            vk: Default::default(),
            _a: PhantomData,
        }
    }
}

pub type BaseAndLog<'a, 'b, A> = Option<(
    &'a FlatSetIndex<A>,
    &'b mut FlatSetIndexLog<<A as FlatSetAdapt>::K, <A as FlatSetAdapt>::V>,
)>;

pub trait FlatSetAdapt: Send + Sized + Sync + Touchable + 'static {
    type Entity: EntityAccessor + CtxTypeInfo + Send;
    type K: Copy + Eq + From<u32> + Hash + Into<u32> + Into<usize> + Send + Sync;
    type V: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;

    fn adapt(
        id: &<Self::Entity as Entity>::Key,
        entity: &Self::Entity,
        out: &mut FxHashSet<(Option<Self::K>, Option<Self::V>)>,
    );

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

            let mut log = FlatSetIndexLog::default();

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
            let _gate = ctx.provider.gate().await;

            Ok(slot.get_or_init(|| {
                let mut base_kv = fast_set::FlatSetIndex::<Self::K, Self::V>::default();
                let mut base_vk = fast_set::FlatSetIndex::<Self::V, Self::K>::default();
                let mut log_kv = fast_set::FlatSetIndexLog::<Self::K, Self::V>::default();
                let mut log_vk = fast_set::FlatSetIndexLog::<Self::V, Self::K>::default();
                let mut set = FxHashSet::default();

                for (id, entity) in tbl.ref_iter() {
                    set.clear();

                    Self::adapt(&id, entity, &mut set);

                    for (k, v) in set.drain() {
                        match (k, v) {
                            (Some(k), Some(v)) => {
                                log_kv.insert(&base_kv, k.clone(), v.clone());
                                log_vk.insert(&base_vk, v, k);
                            }
                            (None, Some(v)) => {
                                log_kv.insert_none(&base_kv, v);
                            }
                            (Some(k), None) => {
                                log_vk.insert_none(&base_vk, k);
                            }
                            (None, None) => {}
                        }
                    }
                }

                base_kv.apply(log_kv);
                base_vk.apply(log_vk);

                FlatSetIndex {
                    _a: PhantomData,
                    kv: base_kv,
                    vk: base_vk,
                }
            }))
        })
    }

    fn handle_clear(ctx: &mut Ctx) {
        ctx.ctx_ext_obj.get_mut(Self::index_var()).take();
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
        log: &mut FlatSetIndexLog<Self::K, Self::V>,
        key: &<Self::Entity as Entity>::Key,
        new: Option<&Self::Entity>,
        old: Option<&Self::Entity>,
        old_set: &mut FxHashSet<(Option<Self::K>, Option<Self::V>)>,
        new_set: &mut FxHashSet<(Option<Self::K>, Option<Self::V>)>,
    ) {
        if let Some(new) = new {
            Self::adapt(key, new, new_set);
        }

        if let Some(old) = old {
            Self::adapt(key, old, old_set);
        }

        if old_set != new_set {
            for (k, v) in &*old_set - &*new_set {
                match (k, v) {
                    (Some(k), Some(v)) => {
                        log.0.remove(&base.kv, k.clone(), v.clone());
                        log.1.remove(&base.vk, v, k);
                    }
                    (None, Some(v)) => {
                        log.0.remove_none(&base.kv, v);
                    }
                    (Some(k), None) => {
                        log.1.remove_none(&base.vk, k);
                    }
                    (None, None) => {}
                }
            }

            for (k, v) in &*new_set - &*old_set {
                match (k, v) {
                    (Some(k), Some(v)) => {
                        log.0.insert(&base.kv, k.clone(), v.clone());
                        log.1.insert(&base.vk, v, k);
                    }
                    (None, Some(v)) => {
                        log.0.insert_none(&base.kv, v);
                    }
                    (Some(k), None) => {
                        log.1.insert_none(&base.vk, k);
                    }
                    (None, None) => {}
                }
            }
        }
    }
}

type FlatSetIndexLog<K, V> = (
    flat_set_index::FlatSetIndexLog<K, V>,
    flat_set_index::FlatSetIndexLog<V, K>,
);

impl<A: FlatSetAdapt> LogOf for FlatSetIndex<A> {
    type Log = FlatSetIndexLog<A::K, A::V>;
}

impl<A: FlatSetAdapt> Touchable for FlatSetIndex<A> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        A::touched()
    }
}

impl<A: FlatSetAdapt> TrxOf for FlatSetIndex<A>
where
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    type Trx<'a>
        = FlatSetIndexTrx<'a, A>
    where
        Self: 'a;

    fn trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            A::get_or_init(trx.ctx).await?;

            // extract the index log and init if required.
            let (base, log) =
                A::base_and_log(trx.ctx, &mut trx.logs).expect("extract base and log");

            let trx = FlatSetIndexTrx {
                base,
                kv: &log.0,
                vk: &log.1,
            };

            Ok(trx)
        })
    }
}

pub struct FlatSetIndexTrx<'a, A: FlatSetAdapt> {
    base: &'a FlatSetIndex<A>,
    kv: &'a flat_set_index::FlatSetIndexLog<A::K, A::V>,
    vk: &'a flat_set_index::FlatSetIndexLog<A::V, A::K>,
}

impl<A: FlatSetAdapt> FlatSetIndexTrx<'_, A> {
    #[inline]
    pub fn kv(&self) -> flat_set_index::FlatSetIndexTrx<'_, A::K, A::V> {
        flat_set_index::FlatSetIndexTrx::new(&self.base.kv, self.kv)
    }

    #[inline]
    pub fn vk(&self) -> flat_set_index::FlatSetIndexTrx<'_, A::V, A::K> {
        flat_set_index::FlatSetIndexTrx::new(&self.base.vk, self.vk)
    }
}

#[macro_export]
macro_rules! flat_set_adapt {
    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty, $out:ident: &mut FxHashSet<(Option<$k:ty>, Option<$v:ty> $(,)?)> $(,)?) {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::FlatSetAdapt for $adapt {
            type Entity = $entity_ty;
            type K = $k;
            type V = $v;

            fn adapt($id: &<Self::Entity as storm::Entity>::Key, $entity: &Self::Entity, $out: &mut storm::fxhash::FxHashSet<(Option<Self::K>, Option<Self::V>)>) {
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

        impl storm::Touchable for $adapt {
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }

        $vis type $alias = storm::indexing::FlatSetIndex<$adapt>;

        #[storm::register]
        fn $init() {
            $adapt::register();
        }
    };
}
