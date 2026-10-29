use crate::{
    AppliedEvent, BoxFuture, ClearEvent, Ctx, CtxTransaction, Entity, EntityValidate, Error, Gc,
    Get, LogOf, OnceCell, ProviderContainer, RefIntoIterator, RemovedEvent, RemovingEvent, Result,
    TouchedEvent, UpsertedEvent, UpsertingEvent, ValidatingEvent, foreign_key,
    logs::TableLog,
    provider::{Delete, LoadAll, TransactionProvider, Upsert, UpsertMut},
    remove_rule,
};
use extobj::{ExtObj, Var, extobj};
use parking_lot::RwLock;
use std::{
    any::{TypeId, type_name},
    borrow::Cow,
    fmt::Debug,
    sync::OnceLock,
};
use tracing::error;

/// Error sites logged by the generic entity operations.
///
/// The logging is outlined in non-generic cold functions so it is compiled once instead of
/// once per entity.
#[derive(Clone, Copy)]
enum Site {
    Removing,
    Removed,
    Upsert,
    TrackInsert,
    Upserted,
}

#[cold]
#[inline(never)]
fn log_error(site: Site, e: &Error, id: &dyn Debug, ty: TypeId) {
    match site {
        Site::Removing => {
            error!({ error = %e, id = ?id, ty = ?ty }, "EntityAccessor::removing event error")
        }
        Site::Removed => {
            error!({ error = %e, id = ?id, ty = ?ty }, "EntityAccessor::removed event error")
        }
        Site::Upsert => error!({ error = %e, id = ?id, ty = ?ty }, "upsert error"),
        Site::TrackInsert => error!({ error = %e, id = ?id, ty = ?ty }, "track_insert error"),
        Site::Upserted => {
            error!({ error = %e, id = ?id, ty = ?ty }, "EntityAccessor::upserted event error")
        }
    }
}

#[cold]
#[inline(never)]
fn log_load_error(e: &Error, ty: TypeId) {
    error!({ error = %e, ty = ?ty }, "table load failed");
}

#[cold]
#[inline(never)]
fn log_validate_error(upserting: bool, id: &dyn Debug, ty: TypeId) {
    if upserting {
        error!({ id = ?id, ty = ?ty }, "EntityAccessor::upserting event error");
    } else {
        error!({ id = ?id, ty = ?ty }, "entity_validate error");
    }
}

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut CtxExtObj) + Send + Sync>>>;
pub type CtxVar<T> = Var<CtxExt, OnceCell<T>>;

extobj!(pub struct CtxExt);

pub type CtxExtObj = ExtObj<CtxExt>;

/// Private: used by macros.
///
/// `static` storage of a [`CtxVar`], allocated in the `Ctx` layout on first access.
///
/// Unlike `extobj!`, this needs no life-before-main constructor, which keeps the generated code
/// small. The macros force the allocation from their registration function, which runs before
/// any `Ctx` is created.
#[doc(hidden)]
pub struct LazyCtxVar<T>(OnceLock<CtxVar<T>>);

impl<T> LazyCtxVar<T> {
    #[allow(clippy::new_without_default)]
    #[inline]
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }
}

impl<T: Send + Sync + 'static> LazyCtxVar<T> {
    #[inline]
    pub fn get(&self) -> CtxVar<T> {
        *self.0.get_or_init(CtxVar::<T>::__new)
    }
}

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
    fn validating() -> &'static ValidatingEvent<Self>;

    /// Trims the `#[storm(trim)]` fields, before the `upserting` event.
    #[inline]
    fn trim_fields(&mut self) {}

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
            match log_entity(trx, key) {
                Some(v) => Ok(v),
                None => Ok(tbl_of::<Self>(trx.ctx).await?.get(key)),
            }
        })
    }

    fn manual_sync(trx: &mut CtxTransaction, key: Self::Key, new: Option<Self>) {
        trx.logs
            .get_mut_or_default(Self::tbl_var())
            .insert(key, new);
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
            let _gate = ctx.provider.gate(type_name::<Self>()).await;

            // if the table is already loaded when we gain access to the provider.
            if let Some(v) = slot.get() {
                return Ok(v);
            }

            // load the table
            let v = ctx
                .provider
                .load_all(&())
                .await
                .inspect_err(|e| log_load_error(e, TypeId::of::<Self>()))?;

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

/// Looks `key` up in the transaction log only: `None` when the log has no entry for it.
#[inline]
pub(crate) fn log_entity<'a, E: EntityAccessor>(
    trx: &'a CtxTransaction<'_>,
    key: &E::Key,
) -> Option<Option<&'a E>> {
    trx.logs
        .get(E::tbl_var())
        .and_then(|log| log.get(key))
        .map(Option::as_ref)
}

/// Same as [`EntityAccessor::entity_from`], without boxing a future when the entity is in the
/// transaction log or the table is already loaded.
#[inline]
pub(crate) async fn entity_of<'a, E>(
    trx: &'a CtxTransaction<'_>,
    key: &'a E::Key,
) -> Result<Option<&'a E>>
where
    E: EntityAccessor,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
{
    match log_entity(trx, key) {
        Some(v) => Ok(v),
        None => Ok(tbl_of::<E>(trx.ctx).await?.get(key)),
    }
}

/// Same as [`EntityAccessor::tbl_from`], without boxing a future when the table is already
/// loaded.
#[inline]
pub(crate) async fn tbl_of<E>(ctx: &Ctx) -> Result<&E::Tbl>
where
    E: EntityAccessor,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
{
    match ctx.tbl_of_opt::<E>() {
        Some(tbl) => Ok(tbl),
        None => E::tbl_from(ctx).await,
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

            // checked before any change so a blocked removal leaves the transaction usable.
            if !remove_rule::rules_of(TypeId::of::<Self>()).is_empty()
                && entity_of::<Self>(trx, &k).await?.is_some()
            {
                remove_rule::check(trx, TypeId::of::<Self>(), &k, &k).await?;
            }

            let gate = trx.err_gate.open()?;

            let old_opt: Option<Option<Self>> =
                trx.logs.get_mut_or_default(var).insert(k.clone(), None);

            let old = match old_opt.as_ref() {
                Some(None) => None,
                Some(Some(old)) => Some(old),
                None => tbl_of::<Self>(ctx).await?.get(&k),
            };

            let Some(old) = old else {
                // nothing to remove.
                gate.close();
                return Ok(false);
            };

            let _event_depth = trx.track_depth();

            Self::removing()
                .call(trx, &k)
                .await
                .inspect_err(|e| log_error(Site::Removing, e, &k, TypeId::of::<Self>()))?;

            if trx
                .logs
                .get(var)
                .is_some_and(|v| v.get(&k).is_some_and(Option::is_none))
            {
                // after `removing` so its handlers can detach the restricted children.
                foreign_key::check_restricts(trx, TypeId::of::<Self>(), &k).await?;

                trx.provider().delete(&k).await?;
                old.track_remove(&k, trx).await?;

                Self::removed()
                    .call(trx, &k, old)
                    .await
                    .inspect_err(|e| log_error(Site::Removed, e, &k, TypeId::of::<Self>()))?;
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

            // free for the owned keys, saving a clone of each of them.
            for key in keys.into_owned() {
                if Self::remove(trx, key).await? {
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
            let has_changed = entity_of::<Self>(trx, &k)
                .await?
                .is_none_or(|e| *e != entity);

            if !has_changed {
                return Ok(false);
            }

            let gate = trx.err_gate.open()?;
            let _event_depth = trx.track_depth();

            validate_on_change(trx, &k, &mut entity).await?;

            trx.provider()
                .upsert(&k, &entity)
                .await
                .inspect_err(|e| log_error(Site::Upsert, e, &k, TypeId::of::<Self>()))?;

            entity
                .track_insert(&k, trx)
                .await
                .inspect_err(|e| log_error(Site::TrackInsert, e, &k, TypeId::of::<Self>()))?;

            let old = trx
                .logs
                .get_mut_or_default(var)
                .insert(k.clone(), Some(entity));

            let old = match old.as_ref() {
                None => tbl_of::<Self>(ctx).await?.get(&k),
                Some(None) => None,
                Some(Some(old)) => Some(old),
            };

            Self::upserted()
                .call(trx, &k, old)
                .await
                .inspect_err(|e| log_error(Site::Upserted, e, &k, TypeId::of::<Self>()))?;
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
        mut k: Self::Key,
        mut entity: Self,
    ) -> BoxFuture<'a, Result<(Self::Key, bool)>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: UpsertMut<Self>,
    {
        let var = Self::tbl_var();

        Box::pin(async move {
            let ctx = trx.ctx;
            let has_changed = entity_of::<Self>(trx, &k)
                .await?
                .is_none_or(|e| *e != entity);

            if !has_changed {
                return Ok((k, false));
            }

            let gate = trx.err_gate.open()?;
            let _event_depth = trx.track_depth();

            validate_on_change(trx, &k, &mut entity).await?;

            trx.provider()
                .upsert_mut(&mut k, &mut entity)
                .await
                .inspect_err(|e| log_error(Site::Upsert, e, &k, TypeId::of::<Self>()))?;

            entity
                .track_insert(&k, trx)
                .await
                .inspect_err(|e| log_error(Site::TrackInsert, e, &k, TypeId::of::<Self>()))?;

            let old = trx
                .logs
                .get_mut_or_default(var)
                .insert(k.clone(), Some(entity));

            let old = match old.as_ref() {
                None => tbl_of::<Self>(ctx).await?.get(&k),
                Some(None) => None,
                Some(Some(old)) => Some(old),
            };

            Self::upserted()
                .call(trx, &k, old)
                .await
                .inspect_err(|e| log_error(Site::Upserted, e, &k, TypeId::of::<Self>()))?;
            gate.close();
            Ok((k, true))
        })
    }

    fn upsert_mut_all<'a>(
        trx: &'a mut CtxTransaction<'_>,
        entities: Vec<(Self::Key, Self)>,
    ) -> BoxFuture<'a, Result<usize>>
    where
        ProviderContainer: LoadAll<Self, (), Self::Tbl>,
        for<'b> TransactionProvider<'b>: UpsertMut<Self>,
    {
        Box::pin(async move {
            let mut count = 0;

            for (key, entity) in entities {
                if Self::upsert_mut(trx, key, entity).await?.1 {
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

    entity.trim_fields();

    if let Err(e) = E::upserting().call(trx, key, entity).await {
        log_validate_error(true, key, TypeId::of::<E>());
        error = Some(e);
        has_error = true;
    }

    // also called on error, so all the errors are reported at once.
    if let Err(e) = E::validating().call(trx, key, entity).await {
        Error::extend_one_opt(&mut error, e);
    }

    EntityValidate::entity_validate(&*entity, &mut error);

    match error {
        Some(e) => {
            if !has_error {
                log_validate_error(false, key, TypeId::of::<E>());
            }
            Err(e)
        }
        None => Ok(()),
    }
}
