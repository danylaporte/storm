//! Unique indexes declared on the entity model and enforced inside transactions.
//!
//! Declared with `#[storm(unique)]` on a field, or with
//! `#[storm(unique(fields = "a, b"))]` on the entity for multiple columns. The derive generates
//! a hash index (unique key -> entity keys) used to detect duplicates when a row is inserted
//! or updated.
//!
//! - String values are compared using a [Collation] (case insensitive by default), and trimmed
//!   when one of the indexed fields is `#[storm(trim)]`.
//! - `None` is a value: only one row can have `None` for the same key.
//! - A `filter` excludes rows from the index (like a SQL filtered index).
//! - Use `lookup` on the generated index (or [find]) to search with a field value: the
//!   collation and the trimming are applied for you.
//! - `no_index` generates no index: the duplicates are found with a table scan. Required for
//!   an entity key not based on `u32` (ex: composite key).

use crate::{
    AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, Entity, EntityAccessor, Error, Fields,
    ProviderContainer, RefIntoIterator, Result,
    accessor::{entity_of, tbl_of},
    error::debug_string,
    indexing::{HashFlatSetAdapt, HashFlatSetIndex, hash_flat_set::HashFlatSetIndexTrx},
    provider::LoadAll,
};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};
#[cfg(feature = "str_utils")]
use str_utils::cmp::EqExt;

/// How strings are compared by a unique index.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Collation {
    /// Case and accent sensitive (binary comparison).
    #[cfg_attr(not(feature = "str_utils"), default)]
    Exact,

    /// Case insensitive, accent sensitive.
    #[cfg(feature = "str_utils")]
    #[cfg_attr(feature = "str_utils", default)]
    Ci,

    /// Case sensitive, accent insensitive.
    #[cfg(feature = "str_utils")]
    Ai,

    /// Case and accent insensitive.
    #[cfg(feature = "str_utils")]
    AiCi,
}

/// A string compared and hashed according to a [Collation].
///
/// Only built by [UniqueValue], so the collation and trimming always match the index.
/// Values with different collations are never equal.
#[derive(Clone)]
pub struct UniqueStr {
    value: Box<str>,
    collation: Collation,
}

impl UniqueStr {
    pub(crate) fn new(value: &str, collation: Collation, trim: bool) -> Self {
        Self {
            value: if trim { value.trim() } else { value }.into(),
            collation,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[inline]
    pub fn collation(&self) -> Collation {
        self.collation
    }
}

impl Debug for UniqueStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.value, f)
    }
}

impl Eq for UniqueStr {}

impl Hash for UniqueStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s: &str = &self.value;

        match self.collation {
            Collation::Exact => s.hash(state),
            #[cfg(feature = "str_utils")]
            Collation::Ci => s.hash_ci(state),
            #[cfg(feature = "str_utils")]
            Collation::Ai => s.hash_ai(state),
            #[cfg(feature = "str_utils")]
            Collation::AiCi => s.hash_ai_ci(state),
        }

        // terminator, so ("ab", "c") and ("a", "bc") hash differently.
        state.write_u8(0xff);
    }
}

impl PartialEq for UniqueStr {
    fn eq(&self, other: &Self) -> bool {
        let (l, r): (&str, &str) = (&self.value, &other.value);

        self.collation == other.collation
            && match self.collation {
                Collation::Exact => l == r,
                #[cfg(feature = "str_utils")]
                Collation::Ci => l.eq_ci(r),
                #[cfg(feature = "str_utils")]
                Collation::Ai => l.eq_ai(r),
                #[cfg(feature = "str_utils")]
                Collation::AiCi => l.eq_ai_ci(r),
            }
    }
}

/// Converts a field value into the key stored in a unique index.
///
/// Implemented for the common scalar and string types. For a custom type, implement it
/// manually or use the [unique_value!](crate::unique_value) macro.
pub trait UniqueValue {
    type Key: Clone + Debug + Eq + Hash + Send + Sync + 'static;

    /// `trim` removes the leading and trailing whitespace of the strings.
    fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key;
}

/// Implements [UniqueValue] for types used as is in the index (`Clone + Debug + Eq + Hash`).
///
/// ```ignore
/// storm::unique_value!(UserId, CountryCode);
/// ```
#[macro_export]
macro_rules! unique_value {
    ($($t:ty),* $(,)?) => {
        $(
            impl $crate::unique::UniqueValue for $t {
                type Key = Self;

                #[inline]
                fn unique_key(&self, _: $crate::unique::Collation, _: bool) -> Self {
                    ::std::clone::Clone::clone(self)
                }
            }
        )*
    };
}

unique_value!(
    bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

#[cfg(feature = "chrono")]
unique_value!(
    chrono::NaiveDate,
    chrono::NaiveDateTime,
    chrono::NaiveTime,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::FixedOffset>,
);

#[cfg(feature = "dec19x5")]
unique_value!(dec19x5::Decimal);

#[cfg(feature = "uuid")]
unique_value!(uuid::Uuid);

macro_rules! unique_str {
    ($($t:ty),* $(,)?) => {
        $(
            impl UniqueValue for $t {
                type Key = UniqueStr;

                #[inline]
                fn unique_key(&self, collation: Collation, trim: bool) -> UniqueStr {
                    UniqueStr::new(self.as_ref(), collation, trim)
                }
            }
        )*
    };
}

unique_str!(str, String, Cow<'_, str>);

#[cfg(feature = "str_utils")]
unique_str!(str_utils::str_ci::StringCi);

#[cfg(feature = "str_utils")]
impl<F> UniqueValue for str_utils::form_str::FormStr<F> {
    type Key = UniqueStr;

    #[inline]
    fn unique_key(&self, collation: Collation, trim: bool) -> UniqueStr {
        UniqueStr::new(self, collation, trim)
    }
}

impl<T: UniqueValue + ?Sized> UniqueValue for &T {
    type Key = T::Key;

    #[inline]
    fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key {
        (**self).unique_key(collation, trim)
    }
}

impl<T: UniqueValue + ?Sized> UniqueValue for Box<T> {
    type Key = T::Key;

    #[inline]
    fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key {
        (**self).unique_key(collation, trim)
    }
}

impl<T: UniqueValue + ?Sized> UniqueValue for Arc<T> {
    type Key = T::Key;

    #[inline]
    fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key {
        (**self).unique_key(collation, trim)
    }
}

impl<T: UniqueValue> UniqueValue for Option<T> {
    type Key = Option<T::Key>;

    #[inline]
    fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key {
        self.as_ref().map(|v| v.unique_key(collation, trim))
    }
}

// Values of a unique index on multiple fields.
macro_rules! unique_tuple {
    ($($n:tt $t:ident),+) => {
        impl<$($t: UniqueValue),+> UniqueValue for ($($t,)+) {
            type Key = ($(<$t as UniqueValue>::Key,)+);

            #[inline]
            fn unique_key(&self, collation: Collation, trim: bool) -> Self::Key {
                ($(self.$n.unique_key(collation, trim),)+)
            }
        }
    };
}

unique_tuple!(0 A, 1 B);
unique_tuple!(0 A, 1 B, 2 C);
unique_tuple!(0 A, 1 B, 2 C, 3 D);

pub type EntityKey<U> = <<U as Unique>::Entity as Entity>::Key;

pub trait Unique: Send + Sync + Sized + 'static {
    type Entity: EntityAccessor + CtxTypeInfo;

    /// Value of the indexed field, or tuple of values for multiple fields.
    type Key: Clone + Debug + Eq + Hash + Send + Sync + 'static;

    /// Finds the entities having a unique key: the generated `HashFlatSetIndex`, or a table scan
    /// with `no_index`.
    type Index: UniqueIndex<Self::Key, EntityKey<Self>>;

    const NAME: &'static str;
    const FIELDS: &'static [&'static str];
    const COLLATION: Collation;

    /// Strings are trimmed: one of the indexed fields is `#[storm(trim)]`.
    const TRIM: bool;

    /// Returns the key of the entity in the index, `None` when excluded by the filter.
    fn unique_key(entity: &Self::Entity) -> Option<Self::Key>;

    /// Returns the indexed fields in the entity fields enum, empty when the entity has none.
    #[inline]
    fn entity_fields() -> Vec<Box<dyn Fields>> {
        Vec::new()
    }
}

/// Links the index generated by `#[storm(unique)]` to its [Unique] definition.
pub trait UniqueAdapt: HashFlatSetAdapt {
    type Unique: Unique<Key = Self::K>;
}

/// Search of the entities by unique key (unique key `K` -> entity keys `V`), implemented by
/// the derive.
pub trait UniqueIndex<K, V>: 'static {
    /// Returns the keys of the entities having `value` in the transaction.
    fn keys<'a>(trx: &'a mut CtxTransaction<'_>, value: &'a K) -> BoxFuture<'a, Result<Vec<V>>>;

    /// Returns the key of an entity having `value`, other than `key`.
    fn first_other<'a>(
        trx: &'a mut CtxTransaction<'_>,
        value: &'a K,
        key: &'a V,
    ) -> BoxFuture<'a, Result<Option<V>>>;
}

/// [UniqueIndex::keys] of a [HashFlatSetIndex].
pub fn hash_index_keys<'a, A>(
    trx: &'a mut CtxTransaction<'_>,
    value: &'a A::K,
) -> BoxFuture<'a, Result<Vec<A::V>>>
where
    A: HashFlatSetAdapt,
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let idx = trx.index::<HashFlatSetIndex<A>>().await?;
        Ok(collect_keys::<A>(idx.get(value)))
    })
}

/// [UniqueIndex::first_other] of a [HashFlatSetIndex].
pub fn hash_index_first_other<'a, A>(
    trx: &'a mut CtxTransaction<'_>,
    value: &'a A::K,
    key: &'a A::V,
) -> BoxFuture<'a, Result<Option<A::V>>>
where
    A: HashFlatSetAdapt,
    A::V: PartialEq,
    Ctx: AsRefAsync<<A::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<A::Entity, (), <A::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let idx = trx.index::<HashFlatSetIndex<A>>().await?;
        Ok(first_other_key::<A>(idx.get(value), key))
    })
}

/// [UniqueIndex::keys] without index: scans the table.
pub fn scan_keys<'a, U>(
    trx: &'a mut CtxTransaction<'_>,
    value: &'a U::Key,
) -> BoxFuture<'a, Result<Vec<EntityKey<U>>>>
where
    U: Unique,
    ProviderContainer: LoadAll<U::Entity, (), <U::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move { scan::<U>(trx, value, None).await })
}

/// [UniqueIndex::first_other] without index: scans the table.
pub fn scan_first_other<'a, U>(
    trx: &'a mut CtxTransaction<'_>,
    value: &'a U::Key,
    key: &'a EntityKey<U>,
) -> BoxFuture<'a, Result<Option<EntityKey<U>>>>
where
    U: Unique,
    ProviderContainer: LoadAll<U::Entity, (), <U::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move { Ok(scan::<U>(trx, value, Some(key)).await?.into_iter().next()) })
}

/// Scans the table overlaid with the transaction log, excluding `exclude`. Stops at the first
/// match when `exclude` is set.
async fn scan<U>(
    trx: &CtxTransaction<'_>,
    value: &U::Key,
    exclude: Option<&EntityKey<U>>,
) -> Result<Vec<EntityKey<U>>>
where
    U: Unique,
    ProviderContainer: LoadAll<U::Entity, (), <U::Entity as EntityAccessor>::Tbl>,
{
    let tbl = tbl_of::<U::Entity>(trx.ctx).await?;
    let log = trx.logs.get(U::Entity::tbl_var());
    let first = exclude.is_some();

    let matches = |k: &EntityKey<U>, e: &U::Entity| {
        exclude != Some(k) && U::unique_key(e).as_ref() == Some(value)
    };

    let committed = tbl
        .ref_iter()
        .filter(|(k, _)| !log.is_some_and(|l| l.contains_key(*k)))
        .filter(|(k, e)| matches(k, e))
        .map(|(k, _)| k.clone());

    let logged = log
        .into_iter()
        .flat_map(|l| l.iter())
        .filter_map(|(k, e)| e.as_ref().filter(|e| matches(k, e)).map(|_| k.clone()));

    let mut keys = committed.chain(logged);

    Ok(match first {
        true => keys.next().into_iter().collect(),
        false => keys.collect(),
    })
}

#[inline]
fn lookup_key<U: Unique>(value: &(impl UniqueValue<Key = U::Key> + ?Sized)) -> U::Key {
    value.unique_key(U::COLLATION, U::TRIM)
}

impl<A: UniqueAdapt> HashFlatSetIndex<A> {
    /// Returns the keys of the entities having `value` (a tuple for multiple fields).
    #[inline]
    pub fn lookup<V>(&self, value: &V) -> &crate::IntSet<A::V>
    where
        V: UniqueValue<Key = A::K> + ?Sized,
    {
        self.get(&lookup_key::<A::Unique>(value))
    }
}

impl<A: UniqueAdapt> HashFlatSetIndexTrx<'_, A> {
    /// Returns the keys of the entities having `value` (a tuple for multiple fields).
    #[inline]
    pub fn lookup<V>(&self, value: &V) -> &crate::IntSet<A::V>
    where
        V: UniqueValue<Key = A::K> + ?Sized,
    {
        self.get(&lookup_key::<A::Unique>(value))
    }
}

/// Returns the keys of the entities having `value` in the transaction, using the index.
///
/// There is normally at most one, unless duplicates were loaded from the provider.
pub async fn find<U>(
    trx: &mut CtxTransaction<'_>,
    value: &(impl UniqueValue<Key = U::Key> + ?Sized),
) -> Result<Vec<EntityKey<U>>>
where
    U: Unique,
    Ctx: AsRefAsync<<U::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<U::Entity, (), <U::Entity as EntityAccessor>::Tbl>,
{
    let key = lookup_key::<U>(value);
    U::Index::keys(trx, &key).await
}

#[inline]
pub(crate) fn collect_keys<A: HashFlatSetAdapt>(set: &crate::IntSet<A::V>) -> Vec<A::V> {
    collect_values(set)
}

#[inline]
fn first_other_key<A: HashFlatSetAdapt>(set: &crate::IntSet<A::V>, key: &A::V) -> Option<A::V> {
    first_other(set, key)
}

// The following helpers are generic over the key only, so they are shared by all the
// constraints of an entity.

pub(crate) fn collect_values<V: TryFrom<u32>>(set: &crate::IntSet<V>) -> Vec<V> {
    set.iter().collect()
}

fn first_other<V: PartialEq + TryFrom<u32>>(set: &crate::IntSet<V>, key: &V) -> Option<V> {
    set.iter().find(|k| k != key)
}

/// `validating` handler: no other entity may have the same unique key.
pub fn check_validating<'a, U>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a EntityKey<U>,
    entity: &'a U::Entity,
) -> BoxFuture<'a, Result<()>>
where
    U: Unique,
    Ctx: AsRefAsync<<U::Entity as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<U::Entity, (), <U::Entity as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let Some(new) = U::unique_key(entity) else {
            return Ok(());
        };

        // unchanged values are not validated again.
        let unchanged = entity_of::<U::Entity>(trx, key)
            .await?
            .and_then(U::unique_key)
            .is_some_and(|old| old == new);

        if unchanged {
            return Ok(());
        }

        if let Some(other) = U::Index::first_other(trx, &new, key).await? {
            return Err(Error::UniqueViolation {
                entity: U::Entity::NAME,
                index: U::NAME,
                fields: U::FIELDS,
                value: debug_string(&new),
                key: debug_string(&other),
                entity_fields: U::entity_fields(),
            });
        }

        Ok(())
    })
}
