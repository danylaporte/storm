use crate::{Asset, AssetProxy, Ctx, CtxVars, LogVars, Result, Tag, Trx};
use attached::Var;
use fxhash::FxHashMap;
use std::{borrow::Borrow, collections::hash_map::Entry, hash::Hash, marker::PhantomData};
use version_tag::VersionTag;

pub struct HashOneMany<K, V, A> {
    _a: PhantomData<A>,
    map: FxHashMap<K, Box<[V]>>,
    tag: VersionTag,
}

impl<K, V, A> HashOneMany<K, V, A>
where
    K: Eq + Hash,
{
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + Eq + Hash,
        Q: Eq + Hash,
    {
        !self.get(key).is_empty()
    }

    pub fn contains_key_value<Q>(&self, key: &Q, value: &V) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
        V: Ord,
    {
        self.get(key).binary_search(value).is_ok()
    }

    pub fn get<'b, Q>(&'b self, key: &Q) -> &'b [V]
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.map.get(key) {
            Some(set) => set,
            None => &[],
        }
    }
}

impl<K, V, A> Tag for HashOneMany<K, V, A> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

impl<K, V, A> FromIterator<(K, V)> for HashOneMany<K, V, A>
where
    K: Eq + Hash,
    V: Ord,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            _a: PhantomData,
            map: create_hash_map(iter),
            tag: VersionTag::new(),
        }
    }
}

pub struct HashOneManyTrx<'a, 'b, K, V, A> {
    map: &'b HashOneMany<K, V, A>,
    trx: &'b mut Trx<'a>,
}

impl<'a, 'b, K, V, A> HashOneManyTrx<'a, 'b, K, V, A>
where
    A: Asset<Log = FxHashMap<K, Vec<V>>>,
    K: Eq + Hash + 'static,
{
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        !self.get(key).is_empty()
    }

    pub fn contains_key_value<Q>(&self, key: &Q, value: &V) -> bool
    where
        K: Borrow<Q>,
        V: Ord,
        Q: Eq + Hash,
    {
        self.get(key).binary_search(value).is_ok()
    }

    fn entry(&mut self, key: K) -> Entry<K, Vec<V>> {
        self.log_mut().entry(key)
    }

    pub fn get<'c, Q>(&'c self, key: &Q) -> &'c [V]
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.trx
            .log
            .get::<A>()
            .and_then(|map| map.get(key))
            .map_or_else(|| self.map.get(key), |vec| &vec[..])
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        V: Clone + Ord,
    {
        let map = self.map;

        match self.entry(key) {
            Entry::Occupied(mut o) => {
                let vec = o.get_mut();

                if let Err(index) = vec.binary_search(&value) {
                    vec.insert(index, value);
                }
            }
            Entry::Vacant(v) => {
                let vec = map.get(v.key());

                if let Err(index) = vec.binary_search(&value) {
                    let mut vec = vec.iter().cloned().collect::<Vec<_>>();
                    vec.insert(index, value);
                    v.insert(vec);
                }
            }
        }
    }

    pub fn insert_key(&mut self, key: K, mut vec: Vec<V>)
    where
        V: Ord,
    {
        vec.sort();
        vec.dedup();

        self.log_mut().insert(key, vec);
    }

    fn log_mut(&mut self) -> &mut FxHashMap<K, Vec<V>> {
        self.trx.log.get_or_init_mut::<A>()
    }

    pub fn remove_key(&mut self, key: K) {
        self.log_mut().insert(key, Vec::new());
    }

    pub fn remove_key_value(&mut self, key: K, value: &V)
    where
        V: Clone + Ord,
    {
        let map = self.map;

        match self.entry(key) {
            Entry::Occupied(mut o) => {
                let vec = o.get_mut();

                if let Ok(index) = vec.binary_search(value) {
                    vec.remove(index);
                }
            }
            Entry::Vacant(v) => {
                let slice = map.get(v.key());

                if let Ok(index) = slice.binary_search(value) {
                    let mut vec = slice.iter().cloned().collect::<Vec<_>>();
                    vec.remove(index);
                    v.insert(vec);
                }
            }
        }
    }
}

impl<K, V, A> Asset for HashOneMany<K, V, A>
where
    A: AssetProxy<Asset = HashOneMany<K, V, A>> + Send + 'static,
    K: Eq + Hash + Send + 'static,
    V: PartialEq + Send + 'static,
{
    const SUPPORT_GC: bool = false;

    type Log = FxHashMap<K, Vec<V>>;
    type Trx<'a: 'b, 'b> = HashOneManyTrx<'a, 'b, K, V, A>;

    fn apply_log(&mut self, log: Self::Log) -> bool {
        let mut changed = false;

        for (key, vec) in log {
            match self.map.entry(key) {
                Entry::Occupied(mut o) => {
                    if vec.is_empty() {
                        o.remove();
                        changed = true;
                    } else if &**o.get() != &vec[..] {
                        o.insert(vec.into_boxed_slice());
                        changed = true;
                    }
                }
                Entry::Vacant(v) => {
                    if !vec.is_empty() {
                        v.insert(vec.into_boxed_slice());
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.tag.notify()
        }

        changed
    }

    #[inline]
    fn ctx_var() -> Var<Self, CtxVars> {
        A::ctx_var()
    }

    fn gc(&mut self) {}

    async fn init(ctx: &Ctx) -> Result<Self> {
        A::init(ctx).await
    }

    #[inline]
    fn log_var() -> Var<Self::Log, LogVars> {
        A::log_var()
    }

    async fn trx<'a: 'b, 'b>(trx: &'b mut Trx<'a>) -> Result<Self::Trx<'a, 'b>> {
        Ok(HashOneManyTrx {
            map: trx.ctx.asset::<Self>().await?,
            trx,
        })
    }

    fn trx_opt<'a: 'b, 'b>(trx: &'b mut Trx<'a>) -> Option<Self::Trx<'a, 'b>> {
        Some(HashOneManyTrx {
            map: trx.ctx.asset_opt::<Self>()?,
            trx,
        })
    }
}

impl<K, V, A> AssetProxy for HashOneMany<K, V, A>
where
    A: AssetProxy<Asset = Self>,
    K: Eq + Hash + Send + 'static,
    V: PartialEq + Send + 'static,
{
    type Asset = Self;

    #[inline]
    fn ctx_var() -> Var<Self::Asset, CtxVars> {
        A::ctx_var()
    }

    #[inline]
    fn log_var() -> Var<<Self::Asset as Asset>::Log, LogVars> {
        A::log_var()
    }

    #[inline]
    fn init(ctx: &Ctx) -> impl std::future::Future<Output = Result<Self::Asset>> + Send {
        A::init(ctx)
    }
}

fn create_hash_map<K, V, I>(iter: I) -> FxHashMap<K, Box<[V]>>
where
    K: Eq + Hash,
    I: IntoIterator<Item = (K, V)>,
    V: Ord,
{
    let mut map = FxHashMap::<K, Vec<V>>::default();

    for (k, v) in iter {
        let vec = map.entry(k).or_default();

        if let Err(index) = vec.binary_search(&v) {
            vec.insert(index, v);
        }
    }

    map.into_iter()
        .map(|(k, v)| (k, v.into_boxed_slice()))
        .collect()
}

// macro_rules! hash_one_many_index {
//     ($vis:vis $name:ident <$k:ty, $v:ty> {
//         async fn init($ctx_init:ident: &Ctx) ->  Result<HashOneMany<K, V, Self>> {
//             $expr_init:expr
//         }
//     }) => {
//         $vis struct $name;

//         impl HashOneManyIndex<$k, $v> for $name {
//             #[inline]
//             fn ctx_var() -> storm::attached::Var<storm::HashOneMany<$k, $v, Self>, storm::CtxVars> {
//                 storm::attached::::var!(VAR: storm::HashOneMany<$k, $v, Self>, storm::CtxVars);
//                 &VAR
//             }

//             #[inline]
//             fn log_var() -> Var<FxHashMap<$k, Vec<$v>>, storm::LogVars> {
//                 attached::var!(VAR: FxHashMap<$k, Vec<$v>>, storm::LogVars);
//                 &VAR
//             }

//             async fn init($ctx_init: &storm::Ctx) -> storm::Result<storm::HashOneMany<$k, $v, Self>>> {
//                 $expr_init
//             }
//         }
//     };
// }

/*
struct InvoiceIdsByAccountId;

impl<K, V> HashOneManyIndex<AccountId, InvoiceId> for InvoiceIdsByAccountId {
    fn ctx_var() -> Var<HashOneMany<AccountId, InvoiceId, Self>, CtxVars> {
        todo!()
    }

    fn log_var() -> Var<FxHashMap<AccountId, Vec<InvoiceId>>, LogVars> {
        todo!()
    }

    fn init(
        ctx: &Ctx,
    ) -> impl std::future::Future<Output = Result<HashOneMany<AccountId, InvoiceId, Self>>> + Send
    {
        todo!()
    }
}

// #[indexing2]
// async fn invoice_id_by_account_id(ctx: &Ctx) -> Result<HashOneMany<AccountId, InvoiceId, Self>> {
//     Invoice::changed(&invoice_id_by_account_id_invoice_changed);
//     Invoice::register_clear_asset(Self);

//     Ok(ctx
//         .tbl_of::<Invoice>()
//         .await?
//         .iter()
//         .map(|invoice_id, invoice| (invoice.account_id, *invoice_id))
//         .collect())
// }

// async fn invoice_id_by_account_id_invoice_changed(
//     trx: &mut Trx<'_>,
//     invoice_id: &InvoiceId,
//     invoice: &Invoice,
//     _history_ctx: &HistoryCtx,
// ) -> Result<()> {
//     let changed =
//         return_ok_default_if_none!(ChangeState::from_trx(trx, invoice_id, new, |e| e.account_id));

//     let mut index = trx.asset::<InvoiceIdByAccountId>().await?;

//     match changed {
//         ChangeState::Changed { old, new } => {
//             index.remove_key_value(old, invoice_id);
//             index.insert(new, invoice_id);
//         }
//         ChangeState::New(new) => {
//             index.insert(new, invoice_id);
//         }
//     }

//     Ok(())
// }
 */
