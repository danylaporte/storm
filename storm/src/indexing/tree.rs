use crate::{
    provider::LoadAll, ApplyOrder, AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, CtxVar,
    EntityAccessor, LogOf, Logs, ProviderContainer, Result, Touchable, TouchedEvent, TrxOf,
    VecTable, __register_apply,
};
use fast_set::tree::{TreeIndexLog, TreeTrx};
use std::{future::ready, marker::PhantomData, mem::take, ops::Deref};

impl<E: TreeEntity> AsRefAsync<TreeIndex<E>> for Ctx
where
    E: TreeEntity,
    E::Key: Copy + Into<u32> + Into<usize>,
    ProviderContainer: LoadAll<E, (), VecTable<E>>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ TreeIndex<E>>> {
        E::tree_get_or_init(self)
    }
}

pub struct TreeIndex<E: TreeEntity>(fast_set::Tree<E::Key>, PhantomData<E>);

impl<E: TreeEntity> Default for TreeIndex<E> {
    #[inline]
    fn default() -> Self {
        Self(Default::default(), PhantomData)
    }
}

impl<E: TreeEntity> Deref for TreeIndex<E> {
    type Target = fast_set::Tree<E::Key>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<E: TreeEntity> FromIterator<(E::Key, Option<E::Key>)> for TreeIndex<E>
where
    E::Key: Into<u32>,
{
    #[inline]
    fn from_iter<T: IntoIterator<Item = (E::Key, Option<E::Key>)>>(iter: T) -> Self {
        Self(fast_set::Tree::from_iter(iter), PhantomData)
    }
}

impl<E: TreeEntity> LogOf for TreeIndex<E> {
    type Log = TreeIndexLog<E::Key>;
}

impl<E: TreeEntity> Touchable for TreeIndex<E> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        E::tree_touched()
    }
}

impl<E: TreeEntity> TrxOf for TreeIndex<E>
where
    E::Key: Into<u32>,
    ProviderContainer: LoadAll<E, (), VecTable<E>>,
{
    type Trx<'a>
        = TreeTrx<'a, E::Key>
    where
        Self: 'a;

    fn trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            E::tree_get_or_init(trx.ctx).await?;

            let (base, log) =
                E::base_and_log(trx.ctx, &mut trx.logs).expect("extract base and log");

            let trx = TreeTrx::new(base, log);

            Ok(trx)
        })
    }
}

pub trait TreeEntity: EntityAccessor<Tbl = VecTable<Self>> + CtxTypeInfo + Send {
    fn parent(&self) -> Option<Self::Key>;
    fn tree_touched() -> &'static TouchedEvent;
    fn tree_var() -> CtxVar<TreeIndex<Self>>;

    fn apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool
    where
        Self::Key: Into<u32>,
    {
        let Some((_, log)) = Self::base_and_log(ctx, logs) else {
            return false;
        };

        let changed = ctx
            .ctx_ext_obj
            .get_mut(Self::tree_var())
            .get_mut()
            .is_some_and(|idx| idx.0.apply(take(log)));

        if changed {
            Self::tree_touched().call(ctx);
        }

        changed
    }

    fn base_and_log<'a, 'b>(
        ctx: &'a Ctx,
        logs: &'b mut Logs,
    ) -> Option<(&'a TreeIndex<Self>, &'b mut TreeIndexLog<Self::Key>)>
    where
        Self::Key: Into<u32>,
    {
        let index_var = Self::tree_var();
        let base = ctx.ctx_ext_obj.get(index_var).get()?;

        if !logs.contains(index_var) {
            let tbl_var = Self::tbl_var();
            let tbl_log = logs.get(tbl_var)?;
            let tbl = ctx.ctx_ext_obj.get(tbl_var).get()?;

            let mut log = TreeIndexLog::default();

            for (k, new) in tbl_log {
                let old = tbl.get(k);

                Self::upsert_or_remove(base, &mut log, k, new.as_ref(), old);
            }
        }

        logs.get_mut(index_var).map(|log| (base, log))
    }

    fn handle_clear(ctx: &mut Ctx) {
        ctx.ctx_ext_obj.get_mut(Self::tree_var()).take();
    }

    fn handle_removed<'a>(
        trx: &'a mut CtxTransaction,
        id: &'a Self::Key,
        entity: &'a Self,
    ) -> BoxFuture<'a, Result<()>>
    where
        Self::Key: Into<u32>,
    {
        Box::pin(async move {
            if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs) {
                Self::upsert_or_remove(base, log, id, None, Some(entity));
            }

            Ok(())
        })
    }

    fn handle_upserted<'a>(
        trx: &'a mut CtxTransaction,
        id: &'a Self::Key,
        old: Option<&'a Self>,
    ) -> BoxFuture<'a, Result<()>>
    where
        Self::Key: Into<u32>,
    {
        let tbl_var = Self::tbl_var();

        // Because we cannot use 2 mut references of the log at the same time, we remove the new entity from the log
        // before updating the index.
        // We then reinsert it back to the log at the end.
        if let Some(new) = trx.logs.get_mut(tbl_var).and_then(|map| map.remove(id)) {
            if let Some(new) = new.as_ref() {
                if let Some((base, log)) = Self::base_and_log(trx.ctx, &mut trx.logs) {
                    Self::upsert_or_remove(base, log, id, Some(new), old);
                }
            }

            trx.logs.get_mut_or_default(tbl_var).insert(id.clone(), new);
        }

        Box::pin(ready(Ok(())))
    }

    fn tree_get_or_init(ctx: &Ctx) -> BoxFuture<'_, Result<&TreeIndex<Self>>>
    where
        Self::Key: Into<u32>,
        ProviderContainer: LoadAll<Self, (), VecTable<Self>>,
    {
        Box::pin(async move {
            let slot = ctx.ctx_ext_obj.get(Self::tree_var());

            if let Some(idx) = slot.get() {
                return Ok(idx);
            }

            let tbl = Self::tbl_from(ctx).await?;
            let _gate = ctx.provider.gate().await;

            Ok(slot.get_or_init(|| {
                TreeIndex::from_iter(tbl.iter().map(|(k, e)| (k.clone(), e.parent())))
            }))
        })
    }

    fn tree_register()
    where
        Self::Key: Into<u32>,
    {
        __register_apply(Self::apply_log, ApplyOrder::Tree);
        Self::cleared().on(Self::handle_clear);
        Self::removed().on(Self::handle_removed);
        Self::upserted().on(Self::handle_upserted);
    }

    fn upsert_or_remove(
        base: &TreeIndex<Self>,
        log: &mut TreeIndexLog<Self::Key>,
        key: &Self::Key,
        new: Option<&Self>,
        old: Option<&Self>,
    ) where
        Self::Key: Into<u32>,
    {
        let old_parent = old.and_then(|old| old.parent());
        let new_parent = new.and_then(|new| new.parent());

        if old_parent != new_parent {
            let key = key.clone();

            if let Some(new_parent) = new_parent {
                log.insert(base, Some(new_parent), key);
            } else {
                log.remove(base, key);
            }
        }
    }
}

#[macro_export]
macro_rules! tree_index_adapt {
    ($alias:ident, $init:ident, $vis:vis fn $n:ident($entity:ident: &$entity_ty:ty $(,)?) -> Option<$k:ty> {
        $($t:tt)*
    }) => {
        impl storm::indexing::TreeEntity for $entity_ty {
            #[inline]
            fn parent(&self) -> Option<Self::Key> {
                let $entity = self;
                $($t)*
            }

            fn tree_touched() -> &'static storm::TouchedEvent {
                static EVENT: storm::TouchedEvent = storm::TouchedEvent::new();
                &EVENT
            }

            fn tree_var() -> storm::CtxVar<storm::indexing::TreeIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<storm::indexing::TreeIndex<$entity_ty>>,
                    },
                    crate_path = storm::extobj
                );

                *V
            }
        }

        $vis type $alias = storm::indexing::TreeIndex<$entity_ty>;

        #[storm::register]
        fn $init() {
            <$entity_ty as storm::indexing::TreeEntity>::tree_register();
        }
    };
}
