use crate::{
    provider::LoadAll, ApplyOrder, AsRefAsync, BoxFuture, Ctx, CtxLocks, CtxTransaction,
    CtxTypeInfo, CtxVar, EntityAccessor, LogOf, Logs, NotifyTag, ProviderContainer, Result, Tag,
    Touchable, TouchedEvent, VecTable, __register_apply, indexing::AsyncAsIdxTrx, ClearEvent,
    Clearable,
};
use fast_set::one_index;
use std::{any::type_name, future::ready, hash::Hash, marker::PhantomData, mem::take, ops::Deref};
use version_tag::VersionTag;

impl<A: OneAdapt> AsRefAsync<OneIndex<A>> for Ctx
where
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ OneIndex<A>>> {
        A::get_or_init(self)
    }
}

impl<A: OneAdapt, L> AsRef<OneIndex<A>> for CtxLocks<'_, L>
where
    L: AsRef<<A::Entity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref(&self) -> &OneIndex<A> {
        A::get_or_init_sync(self.ctx, self.locks.as_ref())
    }
}

pub struct OneIndex<A: OneAdapt> {
    base: one_index::OneIndex<A::K, A::V>,
    tag: VersionTag,
    _a: PhantomData<A>,
}

impl<A: OneAdapt> Default for OneIndex<A> {
    #[inline]
    fn default() -> Self {
        Self {
            base: Default::default(),
            tag: VersionTag::new(),
            _a: PhantomData,
        }
    }
}

impl<A: OneAdapt> Deref for OneIndex<A> {
    type Target = one_index::OneIndex<A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<A: OneAdapt + Touchable> Touchable for OneIndex<A> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        A::touched()
    }
}

impl<A: OneAdapt> AsyncAsIdxTrx for OneIndex<A>
where
    ProviderContainer: LoadAll<A::Entity, (), VecTable<A::Entity>>,
{
    type Trx<'a> = OneIndexTrx<'a, A>;

    fn async_as_idx_trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            A::get_or_init(trx.ctx).await?;

            let (base, log) =
                A::base_and_log(trx.ctx, &mut trx.logs).expect("extract base and log");

            Ok(OneIndexTrx(one_index::OneIndexTrx::new(&base.base, log)))
        })
    }
}

pub type BaseAndLog<'a, 'b, A> = Option<(
    &'a OneIndex<A>,
    &'b mut one_index::OneIndexLog<<A as OneAdapt>::K, <A as OneAdapt>::V>,
)>;

pub trait OneAdapt: Clearable + Send + Sized + Sync + Touchable + 'static {
    type Entity: EntityAccessor<Key = Self::K, Tbl = VecTable<Self::Entity>> + CtxTypeInfo;
    type K: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;
    type V: PartialEq + Send + Sync;

    fn adapt(id: &Self::K, entity: &Self::Entity) -> Option<Self::V>;
    fn index_var() -> CtxVar<OneIndex<Self>>;

    fn apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool {
        let Some((_, log)) = Self::base_and_log(ctx, logs) else {
            return false;
        };

        let changed = ctx
            .ctx_ext_obj
            .get_mut(Self::index_var())
            .get_mut()
            .is_some_and(|idx| {
                let changed = idx.base.apply(take(log));

                if changed {
                    idx.tag.notify();
                }

                changed
            });

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

            let mut log = one_index::OneIndexLog::<Self::K, Self::V>::default();

            for (k, new) in tbl_log {
                let old = tbl.get(k).and_then(|old| Self::adapt(k, old));
                let new = new.as_ref().and_then(|new| Self::adapt(k, new));

                if old != new {
                    if let Some(new) = new {
                        log.insert(base, *k, new);
                    } else {
                        log.remove(base, *k);
                    }
                }
            }

            logs.insert(index_var, log);
        }

        logs.get_mut(index_var).map(|log| (base, log))
    }

    fn get_or_init(ctx: &Ctx) -> BoxFuture<'_, Result<&OneIndex<Self>>>
    where
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
    ) -> &'a OneIndex<Self> {
        let slot = ctx.ctx_ext_obj.get(Self::index_var());

        slot.get_or_init(|| {
            let mut base = one_index::OneIndex::<Self::K, Self::V>::default();
            let mut log = one_index::OneIndexLog::<Self::K, Self::V>::default();

            for (k, entity) in tbl.iter() {
                if let Some(v) = Self::adapt(k, entity) {
                    log.insert(&base, *k, v);
                }
            }

            base.apply(log);

            OneIndex {
                base,
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

    fn handle_entity_remove<'a>(
        trx: &'a mut CtxTransaction<'_>,
        id: &'a Self::K,
        entity: &'a Self::Entity,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let Some(base) = trx.ctx.ctx_ext_obj.get(Self::index_var()).get() else {
                return Ok(());
            };

            if Self::adapt(id, entity).is_some() {
                trx.logs
                    .get_mut_or_default(Self::index_var())
                    .remove(base, *id);
            }

            Ok(())
        })
    }

    fn handle_entity_upsert<'a>(
        trx: &'a mut CtxTransaction<'_>,
        id: &'a Self::K,
        old: Option<&'a Self::Entity>,
    ) -> BoxFuture<'a, Result<()>> {
        let tbl_var = Self::Entity::tbl_var();

        // Because we cannot use 2 mut references of the log at the same time, we remove the new entity from the log
        // before updating the index.
        // We then reinsert it back to the log at the end.
        if let Some(new) = trx.logs.get_mut(tbl_var).and_then(|o| o.remove(id)) {
            if let Some(new) = new.as_ref() {
                if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs) {
                    let new = Self::adapt(id, new);
                    let old = old.as_ref().and_then(|old| Self::adapt(id, old));

                    if new != old {
                        if let Some(new) = new {
                            log.insert(base, *id, new);
                        } else {
                            log.remove(base, *id);
                        }
                    }
                }
            }

            trx.logs.get_mut_or_default(tbl_var).insert(*id, new);
        }

        Box::pin(ready(Ok(())))
    }

    fn register() {
        __register_apply(Self::apply_log, ApplyOrder::NodeSet);
        Self::Entity::cleared().on(Self::handle_clear);
        Self::Entity::removed().on(Self::handle_entity_remove);
        Self::Entity::upserted().on(Self::handle_entity_upsert);
    }
}

impl<A: OneAdapt> Clearable for OneIndex<A> {
    #[inline]
    fn cleared() -> &'static ClearEvent {
        A::cleared()
    }
}

impl<A: OneAdapt> LogOf for OneIndex<A> {
    type Log = one_index::OneIndexLog<A::K, A::V>;
}

impl<A: OneAdapt> NotifyTag for OneIndex<A> {
    #[inline]
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<A: OneAdapt> Tag for OneIndex<A> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

pub struct OneIndexTrx<'a, A: OneAdapt>(one_index::OneIndexTrx<'a, A::K, A::V>);

impl<'a, A: OneAdapt> Deref for OneIndexTrx<'a, A> {
    type Target = one_index::OneIndexTrx<'a, A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[macro_export]
macro_rules! one_adapt {
    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident($id:ident: &$k:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<$v:ty> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::OneAdapt for $adapt {
            type Entity = $entity_ty;
            type K = $k;
            type V = $v;

            #[allow(unused_variables)]
            fn adapt($id: &Self::K, $entity: &Self::Entity) -> Option<Self::V> {
                $($t)*
            }

            fn index_var() -> storm::CtxVar<storm::indexing::OneIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<storm::indexing::OneIndex<$adapt>>,
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
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }

        $vis type $alias = storm::indexing::OneIndex<$adapt>;

        #[storm::register]
        fn $init() {
            <$adapt as storm::indexing::OneAdapt>::register();
        }
    };
}
