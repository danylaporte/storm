//! Business rules restricting the removal of entities.
//!
//! A rule returns `Ok(Some(reason))` to block the removal. Rules are checked by
//! [EntityRemove::remove](crate::EntityRemove::remove) (including cascaded removals) before
//! anything is modified, and reported by [DeletePlan](crate::DeletePlan) as restrictions.
//!
//! ```ignore
//! #[storm::register]
//! fn rules() {
//!     storm::on_remove_rule::<Invoice>(posted_invoice);
//! }
//!
//! fn posted_invoice<'a>(
//!     trx: &'a mut CtxTransaction<'_>,
//!     key: &'a InvoiceId,
//! ) -> BoxFuture<'a, Result<Option<String>>> {
//!     Box::pin(async move {
//!         let posted = trx.get_entity::<Invoice>(key).await?.is_some_and(|i| i.posted);
//!         Ok(posted.then(|| "a posted invoice cannot be removed".into()))
//!     })
//! }
//! ```

use crate::{
    BoxFuture, CtxTransaction, CtxTypeInfo, Entity, Error, Result, error::debug_string,
    registry::InitCell,
};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

pub type RemoveRuleFn<E> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a <E as Entity>::Key,
) -> BoxFuture<'a, Result<Option<String>>>;

type ErasedFn = for<'a> fn(
    rule: &'a (dyn Any + Send + Sync),
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
) -> BoxFuture<'a, Result<Option<String>>>;

pub(crate) struct RemoveRule {
    pub(crate) entity: &'static str,
    rule: Box<dyn Any + Send + Sync>,
    call: ErasedFn,
}

impl RemoveRule {
    /// Returns the reason when the removal of `key` is blocked.
    pub(crate) fn check<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        key: &'a (dyn Any + Send + Sync),
    ) -> BoxFuture<'a, Result<Option<String>>> {
        (self.call)(&*self.rule, trx, key)
    }
}

/// Rules by entity type, in registration order.
static RULES: InitCell<FxHashMap<TypeId, Vec<RemoveRule>>> =
    InitCell::new(FxHashMap::with_hasher(FxBuildHasher));

/// Registers a business rule restricting the removal of `E`. Call in a `#[storm::register]` fn.
pub fn on_remove_rule<E: Entity + CtxTypeInfo>(f: RemoveRuleFn<E>) {
    let rules = RULES.get_mut().entry(TypeId::of::<E>()).or_default();

    rules.push(RemoveRule {
        entity: E::NAME,
        rule: Box::new(f),
        call: call_erased::<E>,
    });
}

fn call_erased<'a, E: Entity>(
    rule: &'a (dyn Any + Send + Sync),
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
) -> BoxFuture<'a, Result<Option<String>>> {
    match (rule.downcast_ref::<RemoveRuleFn<E>>(), key.downcast_ref()) {
        (Some(f), Some(key)) => f(trx, key),
        _ => Box::pin(async { Ok(None) }),
    }
}

#[inline]
pub(crate) fn rules_of(ty: TypeId) -> &'static [RemoveRule] {
    RULES.get().get(&ty).map_or(&[], Vec::as_slice)
}

/// Fails with [Error::RemoveRestricted] on the first rule blocking the removal of `key`.
///
/// Not generic so its state machine is compiled once instead of once per entity.
pub(crate) async fn check(
    trx: &mut CtxTransaction<'_>,
    ty: TypeId,
    key: &(dyn Any + Send + Sync),
    key_dbg: &(dyn Debug + Sync),
) -> Result<()> {
    for rule in rules_of(ty) {
        if let Some(reason) = rule.check(trx, key).await? {
            return Err(Error::RemoveRestricted {
                entity: rule.entity,
                key: debug_string(key_dbg),
                reason,
            });
        }
    }

    Ok(())
}
