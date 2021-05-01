use super::{Provider, ProviderFactory, TransactionProvider};
use crate::{BoxFuture, Error, Result};
use async_cell_lock::AsyncOnceCell;
use once_cell::sync::OnceCell;
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
};
use tokio::sync::{Mutex, MutexGuard};

/// Last recent use counter
type LRU = AtomicU64;

/// A trait that wrap the ProviderFactory to be able to use it in a Box<Any> trait object context.
trait AnyFactory: Send + Sync + 'static {
    fn create<'a>(&'a self) -> BoxFuture<'a, Result<Box<dyn Any + Send + Sync>>>;
}

/// Wrap a ProviderFactory trait to be able to use it in a Box<Any> trait object context.
/// This struct implement AnyFactory which can be use in a trait object.
struct Factory<F, P> {
    _provider: PhantomData<P>,
    factory: F,
}

impl<F, P> Factory<F, P> {
    fn new(factory: F) -> Self {
        Self {
            _provider: PhantomData,
            factory,
        }
    }
}

impl<F, P> AnyFactory for Factory<F, P>
where
    F: ProviderFactory<Provider = P>,
    P: Provider,
{
    fn create<'a>(&'a self) -> BoxFuture<'a, Result<Box<dyn Any + Send + Sync + 'static>>> {
        Box::pin(async move { Ok(Box::new(self.factory.create_provider().await?) as _) })
    }
}

/// A dependency container to be able to instantiate and provide connection to databases.
///
/// A database provider can be named and have a type.
pub struct ProviderContainer {
    last_gc: u64,
    lock: Mutex<()>,
    lru: LRU,
    records: Vec<Rec>,
}

impl ProviderContainer {
    pub fn new() -> Self {
        Self::default()
    }

    fn find_index(&self, type_id: TypeId, name: &str) -> std::result::Result<usize, usize> {
        self.records.binary_search_by_key(&(type_id, name), rec_key)
    }

    fn find_record<'a>(&'a self, type_id: TypeId, name: &str) -> Result<&'a Rec> {
        match self.find_index(type_id, name) {
            Ok(index) => Ok(&self.records[index]),
            Err(_) => return Err(Error::ProviderNotFound),
        }
    }

    pub(crate) async fn gate(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().await
    }

    /// A method to garbage collect all unused provider. This is intended to close database
    /// connections and release resources.
    pub fn gc(&mut self) {
        let last_gc = self.last_gc;
        let new_gc = *self.lru.get_mut();

        if last_gc != new_gc {
            for r in &mut self.records {
                if r.provider
                    .get_mut()
                    .map_or(false, |r| *r.lru.get_mut() <= last_gc)
                {
                    r.provider.take();
                }
            }

            self.last_gc = new_gc;
        }
    }

    pub(super) fn iter_transaction(&self) -> impl Iterator<Item = &'_ dyn Provider> {
        self.records.iter().filter_map(|r| r.for_commit())
    }

    /// Gets or creates a database provider that have been previously registered with
    /// the `register` method.
    pub fn provide<'a, P: Provider>(&'a self, name: &'a str) -> BoxFuture<'a, Result<&'a P>> {
        Box::pin(async move {
            let type_id = TypeId::of::<P>();
            let rec = self.find_record(type_id, name)?;
            let provider = rec.get_or_init(&self.lru).await?;

            Ok(provider.downcast_ref().expect("provider"))
        })
    }

    /// Register a provider factory that creates provider on demand. A provider can be named.
    pub fn register<'a, F: ProviderFactory>(&mut self, name: impl Into<Box<str>>, factory: F) {
        let rec = Rec {
            factory: Box::new(Factory::new(factory)),
            name: name.into(),
            provider: AsyncOnceCell::new(),
            type_id: TypeId::of::<F::Provider>(),
        };

        match self.find_index(rec.type_id, &rec.name) {
            Ok(index) => self.records[index] = rec,
            Err(index) => self.records.insert(index, rec),
        }
    }

    pub fn transaction(&self) -> TransactionProvider<'_> {
        TransactionProvider(self)
    }
}

impl Default for ProviderContainer {
    fn default() -> Self {
        Self {
            last_gc: 0,
            lock: Mutex::new(()),
            lru: AtomicU64::new(1), // starting at 1 because the garbage collector start at 0.
            records: Vec::new(),
        }
    }
}

struct ProviderRec {
    in_transaction: AtomicBool,
    lru: LRU,
    provider: Box<dyn Any + Send + Sync>,
}

impl ProviderRec {
    fn new(provider: Box<dyn Any + Send + Sync>) -> Self {
        Self {
            in_transaction: Default::default(),
            lru: Default::default(),
            provider,
        }
    }

    fn for_commit(&self) -> Option<&dyn Provider> {
        if self.in_transaction.swap(false, Relaxed) {
            Some(
                *self
                    .provider
                    .downcast_ref::<&dyn Provider>()
                    .expect("commit"),
            )
        } else {
            None
        }
    }
}

struct Rec {
    factory: Box<dyn AnyFactory>,
    name: Box<str>,
    provider: AsyncOnceCell<ProviderRec>,
    type_id: TypeId,
}

impl Rec {
    fn for_commit(&self) -> Option<&dyn Provider> {
        self.provider.get()?.for_commit()
    }

    async fn get_or_init(&self, lru: &LRU) -> Result<&Box<dyn Any + Send + Sync>> {
        let provider_rec = self
            .provider
            .get_or_try_init::<_, Error>(async {
                Ok(ProviderRec::new(self.factory.create().await?))
            })
            .await?;

        provider_rec.lru.store(lru.fetch_add(1, Relaxed), Relaxed);

        Ok(&provider_rec.provider)
    }
}

fn rec_key(rec: &Rec) -> (TypeId, &str) {
    (rec.type_id, &rec.name)
}

#[track_caller]
pub fn global_provider() -> &'static ProviderContainer {
    GLOBAL_PROVIDER.get().expect("global_provider")
}

/// Set the global provider.
pub fn set_global_provider(
    provider: ProviderContainer,
) -> std::result::Result<(), ProviderContainer> {
    GLOBAL_PROVIDER.set(provider)
}

static GLOBAL_PROVIDER: OnceCell<ProviderContainer> = OnceCell::new();
