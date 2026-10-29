//! Foreign keys declared on the entity model and enforced inside transactions.
//!
//! Declared with `#[storm(foreign_key(entity = Parent, on_delete = "..."))]` on a field of
//! an entity deriving `Ctx`. The derive generates a reverse index (parent key -> child keys)
//! which is used to find the referencing rows when a parent is removed.
//!
//! - Inserting / updating a child fails when the referenced parent does not exist.
//! - Removing a parent applies the [OnDelete] action to the referencing children.
//! - [OnDelete::Restrict] is checked after the `removing` handlers, so they can detach the
//!   children first.
//! - The children are found with a table scan, unless the reverse index is declared with
//!   `index = "..."` or already built.
//! - The reverse index is a vec-based [FlatSetIndex] (requires `u32` based keys), or a
//!   [HashFlatSetIndex] with `hash`, for parent keys not based on `u32`.
//! - A key column can reference a parent with
//!   `#[storm(foreign_key(column = "...", entity = Parent, key = 0))]` on the type (`key` is
//!   the position in a composite key). The child is found by key for a single key, with a
//!   table scan for a composite key.
//! - A parent defined in a crate depending on the child crate is declared with
//!   `table = "..."` instead of `entity`, and registered on the parent with
//!   `#[storm(referenced_by = ...)]` (see [ExternalForeignKey]).

use crate::{
    AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, Entity, EntityAccessor, EntityRemove,
    EntityUpsert, EntityUpsertMut, Error, Fields, ProviderContainer, RefIntoIterator, Result,
    accessor::{entity_of, tbl_of},
    error::debug_string,
    indexing::{FlatSetAdapt, FlatSetIndex, HashFlatSetAdapt, HashFlatSetIndex},
    provider::{Delete, LoadAll, TransactionProvider, Upsert, UpsertMut},
    registry::InitCell,
    unique::{collect_keys, collect_values},
};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
};

/// Action performed on the referencing children when the referenced parent is removed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OnDelete {
    /// The children are removed (recursively applying their own foreign keys).
    Cascade,

    /// The removal of the parent is blocked with an [Error::ForeignKeyRestricted].
    #[default]
    Restrict,

    /// The foreign key field of the children is set to `None`.
    SetNull,
}

pub type ChildKey<F> = <<F as ForeignKey>::Child as Entity>::Key;
pub type ParentKey<F> = <<F as ForeignKey>::Parent as Entity>::Key;

pub trait ForeignKey: Send + Sync + Sized + 'static {
    /// The entity holding the foreign key field.
    type Child: EntityAccessor + CtxTypeInfo;

    /// The referenced entity.
    type Parent: EntityAccessor + CtxTypeInfo;

    /// Reverse index mapping a parent key to the keys of the children referencing it.
    type Index: ForeignKeyIndex<ParentKey<Self>, ChildKey<Self>>;

    const FIELD: &'static str;
    const ON_DELETE: OnDelete;

    /// Finds the children with the reverse index (built on first use) instead of a table scan.
    const INDEXED: bool = false;

    /// Returns the parent key referenced by the child (from a field or a column of its key), if any.
    fn foreign_key<'a>(key: &'a ChildKey<Self>, child: &'a Self::Child)
    -> Option<&'a ParentKey<Self>>;

    /// Returns the field in the child fields enum, `None` when the child has none.
    #[inline]
    fn entity_field() -> Option<Box<dyn Fields>> {
        None
    }
}

/// Reverse index of a foreign key (parent key `K` -> child keys `V`), implemented by the derive.
pub trait ForeignKeyIndex<K, V>: 'static {
    /// Returns `true` when the index is already built in the context.
    fn is_built(ctx: &Ctx) -> bool;

    /// Returns the keys of the children referencing `parent` in the transaction.
    fn children<'a>(
        trx: &'a mut CtxTransaction<'_>,
        parent: &'a K,
    ) -> BoxFuture<'a, Result<Vec<V>>>;
}

/// [ForeignKeyIndex::is_built] of a [HashFlatSetIndex].
#[inline]
pub fn hash_index_built<A: HashFlatSetAdapt>(ctx: &Ctx) -> bool {
    ctx.ctx_ext_obj.get(A::index_var()).get().is_some()
}

/// [ForeignKeyIndex::children] of a [HashFlatSetIndex].
pub fn hash_index_children<'a, A>(
    trx: &'a mut CtxTransaction<'_>,
    parent: &'a A::K,
) -> BoxFuture<'a, Result<Vec<A::V>>>
where
    A: HashFlatSetAdapt,
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let idx = trx.index::<HashFlatSetIndex<A>>().await?;
        Ok(collect_keys::<A>(idx.get(parent)))
    })
}

/// [ForeignKeyIndex::is_built] of a [FlatSetIndex].
#[inline]
pub fn flat_index_built<A: FlatSetAdapt>(ctx: &Ctx) -> bool {
    ctx.ctx_ext_obj.get(A::index_var()).get().is_some()
}

/// [ForeignKeyIndex::children] of a [FlatSetIndex].
pub fn flat_index_children<'a, A>(
    trx: &'a mut CtxTransaction<'_>,
    parent: &'a A::K,
) -> BoxFuture<'a, Result<Vec<A::V>>>
where
    A: FlatSetAdapt,
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let idx = trx.index::<FlatSetIndex<A>>().await?;
        Ok(collect_values(idx.get(*parent)))
    })
}

/// [ForeignKeyIndex::children] of a foreign key on the whole key of the child: the child having
/// the parent key as key, if any.
pub fn key_children<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    parent: &'a ParentKey<F>,
) -> BoxFuture<'a, Result<Vec<ChildKey<F>>>>
where
    F: ForeignKey,
    F::Child: Entity<Key = ParentKey<F>>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        Ok(entity_of::<F::Child>(trx, parent)
            .await?
            .map(|_| parent.clone())
            .into_iter()
            .collect())
    })
}

/// [ForeignKeyIndex::children] without index: scans the child table.
pub fn scan_index_children<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    parent: &'a ParentKey<F>,
) -> BoxFuture<'a, Result<Vec<ChildKey<F>>>>
where
    F: ForeignKey,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(scan_children::<F>(trx, parent))
}

/// [ForeignKeyIndex::children] of an [ExternalForeignKey] without index: scans the child table.
pub fn scan_external_children<'a, D>(
    trx: &'a mut CtxTransaction<'_>,
    parent: &'a D::ParentKey,
) -> BoxFuture<'a, Result<Vec<<D::Child as Entity>::Key>>>
where
    D: ExternalForeignKey,
    ProviderContainer: LoadAll<D::Child, (), <D::Child as EntityAccessor>::Tbl>,
{
    Box::pin(scan_by::<D::Child>(trx, move |k, c| {
        D::foreign_key(k, c) == Some(parent)
    }))
}

pub trait ForeignKeySetNull: ForeignKey {
    fn set_null(child: &mut Self::Child);

    fn save<'a>(
        trx: &'a mut CtxTransaction<'_>,
        key: ChildKey<Self>,
        child: Self::Child,
    ) -> BoxFuture<'a, Result<()>>;
}

/// Foreign key declared on the child with `table = "..."` instead of `entity`, for a parent
/// defined in a crate depending on the child crate.
///
/// The child derive generates the marker (implementing this trait) and its reverse index. The
/// parent registers it with `#[storm(referenced_by = child_crate::{Child}{Field}Fk)]`, which
/// enforces it as the [ForeignKey] [External]`<Marker, Parent>`.
pub trait ExternalForeignKey: Send + Sync + Sized + 'static {
    type Child: EntityAccessor + CtxTypeInfo;

    /// Key of the referenced (parent) entity.
    type ParentKey: Clone + Debug + Eq + Hash + Send + Sync + 'static;

    type Index: ForeignKeyIndex<Self::ParentKey, <Self::Child as Entity>::Key>;

    const FIELD: &'static str;
    const ON_DELETE: OnDelete;
    const INDEXED: bool = false;

    fn foreign_key<'a>(
        key: &'a <Self::Child as Entity>::Key,
        child: &'a Self::Child,
    ) -> Option<&'a Self::ParentKey>;

    #[inline]
    fn entity_field() -> Option<Box<dyn Fields>> {
        None
    }
}

/// [ForeignKeySetNull] of an [ExternalForeignKey].
pub trait ExternalForeignKeySetNull: ExternalForeignKey {
    fn set_null(child: &mut Self::Child);

    fn save<'a>(
        trx: &'a mut CtxTransaction<'_>,
        key: <Self::Child as Entity>::Key,
        child: Self::Child,
    ) -> BoxFuture<'a, Result<()>>;
}

/// The [ForeignKey] of the [ExternalForeignKey] `D` referencing the parent `P`.
pub struct External<D, P>(PhantomData<fn() -> (D, P)>);

impl<D, P> ForeignKey for External<D, P>
where
    D: ExternalForeignKey,
    P: EntityAccessor + CtxTypeInfo + Entity<Key = D::ParentKey>,
{
    type Child = D::Child;
    type Parent = P;
    type Index = D::Index;

    const FIELD: &'static str = D::FIELD;
    const ON_DELETE: OnDelete = D::ON_DELETE;
    const INDEXED: bool = D::INDEXED;

    #[inline]
    fn foreign_key<'a>(
        key: &'a ChildKey<Self>,
        child: &'a Self::Child,
    ) -> Option<&'a ParentKey<Self>> {
        D::foreign_key(key, child)
    }

    #[inline]
    fn entity_field() -> Option<Box<dyn Fields>> {
        D::entity_field()
    }
}

impl<D, P> ForeignKeySetNull for External<D, P>
where
    D: ExternalForeignKeySetNull,
    P: EntityAccessor + CtxTypeInfo + Entity<Key = D::ParentKey>,
{
    #[inline]
    fn set_null(child: &mut Self::Child) {
        D::set_null(child);
    }

    #[inline]
    fn save<'a>(
        trx: &'a mut CtxTransaction<'_>,
        key: ChildKey<Self>,
        child: Self::Child,
    ) -> BoxFuture<'a, Result<()>> {
        D::save(trx, key, child)
    }
}

/// Returns the keys of the children referencing `parent` in the transaction.
pub async fn children_of<F>(
    trx: &mut CtxTransaction<'_>,
    parent: &ParentKey<F>,
) -> Result<Vec<ChildKey<F>>>
where
    F: ForeignKey,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    if F::INDEXED || F::Index::is_built(trx.ctx) {
        return F::Index::children(trx, parent).await;
    }

    scan_children::<F>(trx, parent).await
}

/// Scans the child table, overlaid with the transaction log.
async fn scan_children<F>(
    trx: &CtxTransaction<'_>,
    parent: &ParentKey<F>,
) -> Result<Vec<ChildKey<F>>>
where
    F: ForeignKey,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    scan_by::<F::Child>(trx, |k, c| F::foreign_key(k, c) == Some(parent)).await
}

/// Returns the keys of the `C` linked to the parent, scanning the table overlaid with the
/// transaction log.
async fn scan_by<C>(
    trx: &CtxTransaction<'_>,
    linked: impl Fn(&C::Key, &C) -> bool,
) -> Result<Vec<C::Key>>
where
    C: EntityAccessor,
    ProviderContainer: LoadAll<C, (), C::Tbl>,
{
    let tbl = tbl_of::<C>(trx.ctx).await?;
    let log = trx.logs.get(C::tbl_var());

    let mut keys = tbl
        .ref_iter()
        .filter(|(k, c)| linked(k, c) && !log.is_some_and(|l| l.contains_key(*k)))
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>();

    if let Some(log) = log {
        keys.extend(
            log.iter()
                .filter(|(k, c)| c.as_ref().is_some_and(|c| linked(k, c)))
                .map(|(k, _)| k.clone()),
        );
    }

    Ok(keys)
}

/// Returns the number of children referencing `parent` in the transaction, ignoring a self
/// reference.
pub(crate) async fn count_children<F>(
    trx: &mut CtxTransaction<'_>,
    parent: &ParentKey<F>,
) -> Result<usize>
where
    F: ForeignKey,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Ok(children_of::<F>(trx, parent)
        .await?
        .iter()
        .filter(|c| !is_self_reference::<F>(c, parent))
        .count())
}

type RestrictFn = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
) -> BoxFuture<'a, Result<()>>;

/// [OnDelete::Restrict] checks by parent type, in registration order.
static RESTRICTS: InitCell<FxHashMap<TypeId, Vec<RestrictFn>>> =
    InitCell::new(FxHashMap::with_hasher(FxBuildHasher));

/// Registers a restriction checked when a `P` is removed, after its `removing` handlers.
pub fn on_restrict<P: 'static>(f: RestrictFn) {
    RESTRICTS
        .get_mut()
        .entry(TypeId::of::<P>())
        .or_default()
        .push(f);
}

/// Runs the restrictions of the parent type `ty` for `key`.
pub(crate) async fn check_restricts(
    trx: &mut CtxTransaction<'_>,
    ty: TypeId,
    key: &(dyn Any + Send + Sync),
) -> Result<()> {
    if let Some(restricts) = RESTRICTS.get().get(&ty) {
        for f in restricts {
            f(trx, key).await?;
        }
    }

    Ok(())
}

/// [on_restrict] handler of the parent for [OnDelete::Restrict].
pub fn check_restrict<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    match key.downcast_ref::<ParentKey<F>>() {
        Some(key) => on_delete_restrict::<F>(trx, key),
        None => Box::pin(async { Ok(()) }),
    }
}

/// `validating` handler of the child: the referenced parent must exist.
pub fn check_validating<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a ChildKey<F>,
    child: &'a F::Child,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>
        + LoadAll<F::Parent, (), <F::Parent as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let Some(parent_key) = F::foreign_key(key, child) else {
            return Ok(());
        };

        // unchanged references are not validated again.
        let old = entity_of::<F::Child>(trx, key).await?;

        if old.and_then(|old| F::foreign_key(key, old)) == Some(parent_key)
            || is_self_reference::<F>(key, parent_key)
        {
            return Ok(());
        }

        if entity_of::<F::Parent>(trx, parent_key).await?.is_none() {
            return Err(Error::ForeignKeyNotFound {
                entity: F::Child::NAME,
                field: F::FIELD,
                parent: F::Parent::NAME,
                key: debug_string(parent_key),
                entity_field: F::entity_field(),
            });
        }

        Ok(())
    })
}

/// Check of the parent for [OnDelete::Restrict], run after its `removing` handlers.
pub fn on_delete_restrict<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a ParentKey<F>,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let count = count_children::<F>(trx, key).await?;

        if count == 0 {
            return Ok(());
        }

        Err(Error::ForeignKeyRestricted {
            entity: F::Child::NAME,
            field: F::FIELD,
            parent: F::Parent::NAME,
            key: debug_string(key),
            count,
        })
    })
}

/// `removing` handler of the parent for [OnDelete::Cascade].
pub fn on_delete_cascade<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a ParentKey<F>,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    F::Child: EntityRemove,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
    for<'b> TransactionProvider<'b>: Delete<F::Child>,
{
    Box::pin(async move {
        for child in children_of::<F>(trx, key).await? {
            F::Child::remove(trx, child).await?;
        }

        Ok(())
    })
}

/// `removing` handler of the parent for [OnDelete::SetNull].
pub fn on_delete_set_null<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a ParentKey<F>,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKeySetNull,
    F::Child: Clone,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        for child_key in children_of::<F>(trx, key).await? {
            let Some(child) = entity_of::<F::Child>(trx, &child_key).await? else {
                continue;
            };

            if F::foreign_key(&child_key, child) != Some(key) {
                continue;
            }

            let mut child = child.clone();
            F::set_null(&mut child);
            F::save(trx, child_key, child).await?;
        }

        Ok(())
    })
}

pub(crate) fn is_self_reference<F: ForeignKey>(child: &ChildKey<F>, parent: &ParentKey<F>) -> bool {
    TypeId::of::<F::Child>() == TypeId::of::<F::Parent>()
        && (child as &dyn Any).downcast_ref::<ParentKey<F>>() == Some(parent)
}

/// Private. Used by the macro to save a child using either
/// [EntityUpsert] or [EntityUpsertMut] (autoref specialization).
#[doc(hidden)]
pub struct __FkSaver<E>(PhantomData<E>);

impl<E> __FkSaver<E> {
    #[doc(hidden)]
    #[inline]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[doc(hidden)]
pub trait __FkSaveUpsert<E: Entity> {
    fn __fk_save<'a>(
        &self,
        trx: &'a mut CtxTransaction<'_>,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'a, Result<()>>;
}

impl<E> __FkSaveUpsert<E> for __FkSaver<E>
where
    E: EntityUpsert,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
    for<'b> TransactionProvider<'b>: Upsert<E>,
{
    fn __fk_save<'a>(
        &self,
        trx: &'a mut CtxTransaction<'_>,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move { E::upsert(trx, key, entity).await.map(|_| ()) })
    }
}

#[doc(hidden)]
pub trait __FkSaveUpsertMut<E: Entity> {
    fn __fk_save<'a>(
        &self,
        trx: &'a mut CtxTransaction<'_>,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'a, Result<()>>;
}

impl<E> __FkSaveUpsertMut<E> for &__FkSaver<E>
where
    E: EntityUpsertMut,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
    for<'b> TransactionProvider<'b>: UpsertMut<E>,
{
    fn __fk_save<'a>(
        &self,
        trx: &'a mut CtxTransaction<'_>,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move { E::upsert_mut(trx, key, entity).await.map(|_| ()) })
    }
}
