use crate::{
    indexing::{TreeEntity, TreeIndex},
    provider::LoadAll,
    ApplyOrder, AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, CtxVar, EntityAccessor,
    LogOf, Logs, NotifyTag, ProviderContainer, Result, Tag, Touchable, TouchedEvent, TrxOf,
    VecTable, __register_apply,
};
use fast_set::{node_set_index, NodeSetIndexLog, Tree, TreeIndexLog};
use std::{any::type_name, future::ready, hash::Hash, marker::PhantomData, mem::take, ops::Deref};
use version_tag::VersionTag;

impl<A: NodeSetAdapt> AsRefAsync<NodeSetIndex<A>> for Ctx
where
    ProviderContainer: LoadAll<A::FlatEntity, (), <A::FlatEntity as EntityAccessor>::Tbl>
        + LoadAll<A::TreeEntity, (), <A::TreeEntity as EntityAccessor>::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ NodeSetIndex<A>>> {
        A::get_or_init(self)
    }
}

pub struct NodeSetIndex<A: NodeSetAdapt> {
    kv: node_set_index::NodeSetIndex<A::K, A::V>,
    tag: VersionTag,
    _a: PhantomData<A>,
}

impl<A: NodeSetAdapt> Default for NodeSetIndex<A> {
    #[inline]
    fn default() -> Self {
        Self {
            kv: Default::default(),
            tag: VersionTag::new(),
            _a: PhantomData,
        }
    }
}

impl<A: NodeSetAdapt> Deref for NodeSetIndex<A> {
    type Target = node_set_index::NodeSetIndex<A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.kv
    }
}

impl<A: NodeSetAdapt + Touchable> Touchable for NodeSetIndex<A> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        A::touched()
    }
}

pub type BaseAndLog<'a, 'b, A> = Option<(
    &'a NodeSetIndex<A>,
    &'b mut NodeSetIndexLog<<A as NodeSetAdapt>::K, <A as NodeSetAdapt>::V>,
    &'a TreeIndex<<A as NodeSetAdapt>::TreeEntity>,
    &'b TreeIndexLog<<A as NodeSetAdapt>::K>,
)>;

pub trait NodeSetAdapt: Touchable + Send + Sized + Sync + 'static {
    type FlatEntity: EntityAccessor<Key = Self::V, Tbl = VecTable<Self::FlatEntity>> + CtxTypeInfo;
    type TreeEntity: TreeEntity<Key = Self::K, Tbl = VecTable<Self::TreeEntity>> + CtxTypeInfo;
    type K: Copy + From<u32> + Into<u32> + PartialEq + Send + Sync;
    type V: Copy + Eq + From<u32> + Hash + Into<u32> + Send + Sync;

    fn adapt(id: &Self::V, entity: &Self::FlatEntity) -> Option<Self::K>;
    fn index_var() -> CtxVar<NodeSetIndex<Self>>;

    fn apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool {
        let Some((_, log, _, _)) = Self::base_and_log(ctx, logs) else {
            return false;
        };

        let changed = ctx
            .ctx_ext_obj
            .get_mut(Self::index_var())
            .get_mut()
            .is_some_and(|idx| {
                let changed = idx.kv.apply(take(log));

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

        // If the index is not loaded in ctx, no need to create anything, just return None.
        let base = ctx.ctx_ext_obj.get(index_var).get()?;

        // If the logs is not there for the index, we will create it.
        if !logs.contains(index_var) {
            let tbl_var = Self::FlatEntity::tbl_var();
            let tree_var = Self::TreeEntity::tree_var();

            let mut log = NodeSetIndexLog::<Self::K, Self::V>::default();

            // If there is data already in the change log for the related table / tree
            // create the log accordingly.
            if let Some(tbl_log) = logs.get(tbl_var) {
                if let Some(tbl) = ctx.ctx_ext_obj.get(tbl_var).get() {
                    if let Some(tree) = ctx.ctx_ext_obj.get(tree_var).get() {
                        let tree_log = TreeIndexLog::default();
                        let tree_log = logs.get(tree_var).unwrap_or(&tree_log);

                        for (v, new) in tbl_log {
                            let old = tbl.get(v);
                            Self::upsert_or_remove(
                                base,
                                &mut log,
                                tree,
                                tree_log,
                                v,
                                new.as_ref(),
                                old,
                            );
                        }
                    }
                }
            }

            logs.insert(index_var, log);
        }

        let map = logs as *mut Logs;

        // TreeEntity is not the same slot as the NodeSetIndexLog. this is considered safe.
        // The hashmap must be keep readonly, only the content of the slot is shared.
        let (tree_base, tree_log) = Self::TreeEntity::base_and_log(ctx, unsafe { &mut *map })?;

        let log = logs.get_mut(Self::index_var())?;

        Some((base, log, tree_base, tree_log))
    }

    fn get_or_init(ctx: &Ctx) -> BoxFuture<'_, Result<&NodeSetIndex<Self>>>
    where
        ProviderContainer: LoadAll<Self::FlatEntity, (), <Self::FlatEntity as EntityAccessor>::Tbl>,
        ProviderContainer: LoadAll<Self::TreeEntity, (), <Self::TreeEntity as EntityAccessor>::Tbl>,
    {
        Box::pin(async move {
            let slot = ctx.ctx_ext_obj.get(Self::index_var());

            if let Some(idx) = slot.get() {
                return Ok(idx);
            }

            let tbl = ctx.tbl_of::<Self::FlatEntity>().await?;
            let tree = Self::TreeEntity::tree_get_or_init(ctx).await?;
            let tree_log = TreeIndexLog::default();

            let _gate = ctx.provider.gate(type_name::<Self>()).await;

            Ok(slot.get_or_init(|| {
                let mut kv = fast_set::NodeSetIndex::<Self::K, Self::V>::default();
                let mut log_kv = NodeSetIndexLog::<Self::K, Self::V>::default();

                for (v, entity) in tbl.iter() {
                    if let Some(k) = Self::adapt(v, entity) {
                        log_kv.insert(&kv, tree, &tree_log, k, *v);
                    }
                }

                kv.apply(log_kv);

                NodeSetIndex {
                    kv,
                    tag: VersionTag::new(),
                    _a: PhantomData,
                }
            }))
        })
    }

    fn handle_clear(ctx: &mut Ctx) {
        ctx.ctx_ext_obj.get_mut(Self::index_var()).take();
    }

    fn handle_flat_entity_remove<'a>(
        trx: &'a mut CtxTransaction<'_>,
        v: &'a Self::V,
        entity: &'a Self::FlatEntity,
    ) -> BoxFuture<'a, Result<()>>
    where
        ProviderContainer: LoadAll<Self::TreeEntity, (), <Self::TreeEntity as EntityAccessor>::Tbl>,
    {
        Box::pin(async move {
            let Some(base) = trx.ctx.ctx_ext_obj.get(Self::index_var()).get() else {
                return Ok(());
            };

            if let Some(k) = Self::adapt(v, entity) {
                let tree = Self::TreeEntity::tree_get_or_init(trx.ctx).await?;
                let tree_log = trx.logs.remove(Self::TreeEntity::tree_var());
                let temp_tree_log = TreeIndexLog::default();
                let tree_log_ref = tree_log.as_ref().unwrap_or(&temp_tree_log);

                let log = trx.logs.get_mut_or_default(Self::index_var());

                log.remove(&base.kv, tree, tree_log_ref, k, *v);

                if let Some(tree_log) = tree_log {
                    // put back the log.
                    trx.logs.insert(Self::TreeEntity::tree_var(), tree_log);
                }
            }

            Ok(())
        })
    }

    fn handle_flat_entity_upsert<'a>(
        trx: &'a mut CtxTransaction<'_>,
        v: &'a Self::V,
        old: Option<&'a Self::FlatEntity>,
    ) -> BoxFuture<'a, Result<()>>
    where
        ProviderContainer: LoadAll<Self::TreeEntity, (), <Self::TreeEntity as EntityAccessor>::Tbl>,
    {
        let tbl_var = Self::FlatEntity::tbl_var();

        // Because we cannot use 2 mut references of the log at the same time, we remove the new entity from the log
        // before updating the index.
        // We then reinsert it back to the log at the end.
        if let Some(new) = trx.logs.get_mut(tbl_var).and_then(|map| map.remove(v)) {
            if let Some(new) = new.as_ref() {
                if let Some((base, log, tree, tree_log)) =
                    Self::base_and_log(trx.ctx, &mut trx.logs)
                {
                    Self::upsert_or_remove(base, log, tree, tree_log, v, Some(new), old);
                }
            }

            trx.logs.get_mut_or_default(tbl_var).insert(*v, new);
        }

        Box::pin(ready(Ok(())))
    }

    fn register()
    where
        ProviderContainer: LoadAll<Self::TreeEntity, (), <Self::TreeEntity as EntityAccessor>::Tbl>,
    {
        __register_apply(Self::apply_log, ApplyOrder::NodeSet);
        Self::FlatEntity::cleared().on(Self::handle_clear);
        Self::FlatEntity::removed().on(Self::handle_flat_entity_remove);
        Self::FlatEntity::upserted().on(Self::handle_flat_entity_upsert);
    }

    fn upsert_or_remove(
        base: &NodeSetIndex<Self>,
        log: &mut NodeSetIndexLog<Self::K, Self::V>,
        tree: &Tree<Self::K>,
        tree_log: &TreeIndexLog<Self::K>,
        v: &Self::V,
        new: Option<&Self::FlatEntity>,
        old: Option<&Self::FlatEntity>,
    ) {
        let new_k = new.and_then(|new| Self::adapt(v, new));
        let old_k = old.and_then(|old| Self::adapt(v, old));

        if new_k != old_k {
            if let Some(old_k) = old_k {
                log.remove(&base.kv, tree, tree_log, old_k, *v);
            }

            if let Some(new_k) = new_k {
                log.insert(&base.kv, tree, tree_log, new_k, *v);
            }
        }
    }
}

impl<A: NodeSetAdapt> LogOf for NodeSetIndex<A> {
    type Log = NodeSetIndexLog<A::K, A::V>;
}

impl<A: NodeSetAdapt> NotifyTag for NodeSetIndex<A> {
    #[inline]
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<A: NodeSetAdapt> Tag for NodeSetIndex<A> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<A: NodeSetAdapt> TrxOf for NodeSetIndex<A>
where
    ProviderContainer: LoadAll<A::FlatEntity, (), VecTable<A::FlatEntity>>,
    ProviderContainer: LoadAll<A::TreeEntity, (), VecTable<A::TreeEntity>>,
{
    type Trx<'a>
        = NodeSetIndexTrx<'a, A>
    where
        Self: 'a;

    fn trx<'a>(trx: &'a mut CtxTransaction) -> BoxFuture<'a, Result<Self::Trx<'a>>> {
        Box::pin(async move {
            // force loading the index.
            A::get_or_init(trx.ctx).await?;

            let (base, log, _, _) =
                A::base_and_log(trx.ctx, &mut trx.logs).expect("extract base and log");

            Ok(NodeSetIndexTrx(node_set_index::NodeSetIndexTrx::new(
                &base.kv, log,
            )))
        })
    }
}

pub struct NodeSetIndexTrx<'a, A: NodeSetAdapt>(node_set_index::NodeSetIndexTrx<'a, A::K, A::V>);

impl<'a, A: NodeSetAdapt> Deref for NodeSetIndexTrx<'a, A> {
    type Target = node_set_index::NodeSetIndexTrx<'a, A::K, A::V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[macro_export]
macro_rules! node_set_index_adapt {
    ($adapt:ident, $alias:ident, $init:ident,
        $vis:vis fn $n:ident(_tree: &$tree_ty:ty, $id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<$k:ty> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::NodeSetAdapt for $adapt {
            type FlatEntity = $entity_ty;
            type TreeEntity = $tree_ty;
            type K = $k;
            type V = $entity_key;

            #[allow(unused_variables)]
            fn adapt($id: &Self::V, $entity: &Self::FlatEntity) -> Option<Self::K> {
                $($t)*
            }

            fn index_var() -> storm::CtxVar<storm::indexing::NodeSetIndex<Self>> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<storm::indexing::NodeSetIndex<$adapt>>,
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

        $vis type $alias = storm::indexing::NodeSetIndex<$adapt>;

        #[storm::register]
        fn $init() {
            <$adapt as storm::indexing::NodeSetAdapt>::register();
        }
    };
}
