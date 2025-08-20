use crate::{
    logs::TableLog,
    provider::{Delete, LoadAll, TransactionProvider, Upsert, UpsertMut},
    AppliedEvent, BoxFuture, ClearEvent, Ctx, CtxTransaction, Entity, EntityValidate, Gc, Get,
    LogOf, ProviderContainer, RefIntoIterator, RemovedEvent, RemovingEvent, Result, TouchedEvent,
    UpsertedEvent, UpsertingEvent,
};
use extobj::{extobj, ExtObj, Var};
use parking_lot::RwLock;
use std::{any::TypeId, borrow::Cow, sync::OnceLock};
use tracing::error;

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut CtxExtObj) + Send + Sync>>>;
pub type CtxVar<T> = Var<CtxExt, OnceLock<T>>;

extobj!(pub struct CtxExt);

pub type CtxExtObj = ExtObj<CtxExt>;

pub trait EntityAccessor: Entity + Sized + 'static {
    type Tbl: Default
        + Extend<(Self::Key, Self)>
        + Get<Self>
        + LogOf<Log = TableLog<Self>>
        + for<'a> RefIntoIterator<Item<'a> = (&'a Self::Key, &'a Self)>
        + Send
        + Sync;

    fn applied() -> &'static AppliedEvent<Self>;
    fn cleared() -> &'static ClearEvent;
    fn removed() -> &'static RemovedEvent<Self>;
    fn removing() -> &'static RemovingEvent<Self>;
    fn tbl_var() -> CtxVar<Self::Tbl>;
    fn touched() -> &'static TouchedEvent;
    fn upserted() -> &'static UpsertedEvent<Self>;
    fn upserting() -> &'static UpsertingEvent<Self>;

    fn clear(ctx: &mut Ctx) {
        if ctx.ctx_ext_obj.get_mut(Self::tbl_var()).take().is_some() {
            Self::cleared().call(ctx);
        }
    }

    fn entity_from<'a>(
        trx: &'a CtxTransaction<'_>,
        key: &'a Self::Key,
    ) -> BoxFuture<'a, Result<Option<&'a Self>>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
    {
        Box::pin(async move {
            let var = Self::tbl_var();
            let ctx = trx.ctx;

            // check if the table is already loaded in the transaction logs.
            Ok(match trx.logs.get(var).and_then(|map| map.get(key)) {
                Some(v) => v.as_ref(),
                None => Self::tbl_from(ctx).await?.get(key),
            })
        })
    }

    fn tbl_from(ctx: &Ctx) -> BoxFuture<'_, Result<&Self::Tbl>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
    {
        Box::pin(async move {
            let slot = ctx.ctx_ext_obj.get(Self::tbl_var());

            // get the table if already initialized.
            if let Some(v) = slot.get() {
                return Ok(v);
            }

            // lock the provider to load the table.
            let _gate = ctx.provider.gate().await;

            // if the table is already loaded when we gain access to the provider.
            if let Some(v) = slot.get() {
                return Ok(v);
            }

            // load the table
            let v = ctx.provider.load_all(&()).await.inspect_err(|e| {
                error!({ error = %e, ty = ?TypeId::of::<Self>() }, "table load failed");
            })?;

            Ok(slot.get_or_init(|| v))
        })
    }

    fn tbl_gc(ctx: &mut Ctx)
    where
        Self::Tbl: Gc,
    {
        if let Some(tbl) = ctx.ctx_ext_obj.get_mut(Self::tbl_var()).get_mut() {
            tbl.gc();
        }
    }
}

pub trait EntityRemove: EntityAccessor {
    fn remove<'a>(trx: &'a mut CtxTransaction<'_>, k: Self::Key) -> BoxFuture<'a, Result<bool>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: Delete<Self>,
    {
        let var = Self::tbl_var();

        Box::pin(async move {
            let ctx = trx.ctx;
            let gate = trx.err_gate.open()?;

            let old_opt: Option<Option<Self>> =
                trx.logs.get_mut_or_default(var).insert(k.clone(), None);

            let old = match old_opt.as_ref() {
                Some(None) => None,
                Some(Some(old)) => Some(old),
                None => Self::tbl_from(ctx).await?.get(&k),
            };

            let Some(old) = old else {
                // nothing to remove.
                gate.close();
                return Ok(false);
            };

            Self::removing().call(trx, &k).await.inspect_err(|e| {
                error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "EntityAccessor::removing event error")
            })?;

            if trx
                .logs
                .get(var)
                .is_some_and(|v| v.get(&k).is_some_and(Option::is_none))
            {
                trx.provider().delete(&k).await?;
                old.track_remove(&k, trx).await?;

                Self::removed().call(trx, &k, old).await.inspect_err(|e| {
                    error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "EntityAccessor::removed event error")
                })?;
            }

            gate.close();

            Ok(true)
        })
    }

    fn remove_all<'a>(
        trx: &'a mut CtxTransaction<'_>,
        keys: Cow<'a, [Self::Key]>,
    ) -> BoxFuture<'a, Result<usize>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: Delete<Self>,
    {
        Box::pin(async move {
            let mut count = 0;

            for key in &*keys {
                if Self::remove(trx, key.clone()).await? {
                    count += 1;
                }
            }

            Ok(count)
        })
    }
}

pub trait EntityUpsert: EntityAccessor + EntityValidate + PartialEq {
    fn upsert<'a>(
        trx: &'a mut CtxTransaction<'_>,
        k: Self::Key,
        mut entity: Self,
    ) -> BoxFuture<'a, Result<bool>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: Upsert<Self>,
    {
        let var = Self::tbl_var();

        Box::pin(async move {
            let ctx = trx.ctx;
            let has_changed = Self::entity_from(trx, &k)
                .await?
                .is_none_or(|e| *e != entity);

            if !has_changed {
                return Ok(false);
            }

            let gate = trx.err_gate.open()?;

            validate_on_change(trx, &k, &mut entity).await?;

            trx.provider().upsert(&k, &entity).await.inspect_err(
                |e| error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "upsert error"),
            )?;

            entity.track_insert(&k, trx).await.inspect_err(|e| {
                error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "track_insert error")
            })?;

            let old = trx
                .logs
                .get_mut_or_default(var)
                .insert(k.clone(), Some(entity));

            let old = match old.as_ref() {
                None => Self::tbl_from(ctx).await?.get(&k),
                Some(None) => None,
                Some(Some(old)) => Some(old),
            };

            Self::upserted().call(trx, &k, old).await.inspect_err(|e| error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "EntityAccessor::upserted event error"))?;
            gate.close();
            Ok(true)
        })
    }

    fn upsert_all<'a>(
        trx: &'a mut CtxTransaction<'_>,
        entities: Vec<(Self::Key, Self)>,
    ) -> BoxFuture<'a, Result<usize>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: Upsert<Self>,
    {
        Box::pin(async move {
            let mut count = 0;

            for (key, entity) in entities {
                if Self::upsert(trx, key, entity).await? {
                    count += 1;
                }
            }

            Ok(count)
        })
    }
}

pub trait EntityUpsertMut: EntityAccessor + EntityValidate + PartialEq {
    fn upsert_mut<'a>(
        trx: &'a mut CtxTransaction<'_>,
        k: &'a mut Self::Key,
        mut entity: Self,
    ) -> BoxFuture<'a, Result<bool>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: UpsertMut<Self>,
    {
        let var = Self::tbl_var();

        Box::pin(async move {
            let ctx = trx.ctx;
            let has_changed = Self::entity_from(trx, k)
                .await?
                .is_none_or(|e| *e != entity);

            if !has_changed {
                return Ok(false);
            }

            let gate = trx.err_gate.open()?;

            validate_on_change(trx, &*k, &mut entity).await?;

            trx.provider()
                .upsert_mut(k, &mut entity)
                .await
                .inspect_err(
                    |e| error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "upsert error"),
                )?;

            entity.track_insert(k, trx).await.inspect_err(|e| {
                error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "track_insert error")
            })?;

            let old = trx
                .logs
                .get_mut_or_default(var)
                .insert(k.clone(), Some(entity));

            let old = match old.as_ref() {
                None => Self::tbl_from(ctx).await?.get(k),
                Some(None) => None,
                Some(Some(old)) => Some(old),
            };

            Self::upserted().call(trx, k, old).await.inspect_err(|e| error!({ error = %e, id = ?k, ty = ?TypeId::of::<Self>() }, "EntityAccessor::upserted event error"))?;
            gate.close();
            Ok(true)
        })
    }

    fn upsert_all_mut<'a>(
        trx: &'a mut CtxTransaction<'_>,
        entities: Vec<(Self::Key, Self)>,
    ) -> BoxFuture<'a, Result<usize>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: UpsertMut<Self>,
    {
        Box::pin(async move {
            let mut count = 0;

            for (mut key, entity) in entities {
                if Self::upsert_mut(trx, &mut key, entity).await? {
                    count += 1;
                }
            }

            Ok(count)
        })
    }
}

async fn validate_on_change<E>(
    trx: &mut CtxTransaction<'_>,
    key: &E::Key,
    entity: &mut E,
) -> Result<()>
where
    E: EntityAccessor + EntityValidate,
{
    let mut error = None;
    let mut has_error = false;

    if let Err(e) = E::upserting().call(trx, key, entity).await {
        error!({ id = ?key, ty = ?TypeId::of::<E>() }, "EntityAccessor::upserting event error");
        error = Some(e);
        has_error = true;
    }

    EntityValidate::entity_validate(&*entity, &mut error);

    match error {
        Some(e) => {
            if !has_error {
                error!({ id = ?key, ty = ?TypeId::of::<E>() }, "entity_validate error");
            }
            Err(e)
        }
        None => Ok(()),
    }
}
