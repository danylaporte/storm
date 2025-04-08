use super::{CastProvider, Provider, ProviderFactory, TransactionProvider};
use crate::{BoxFuture, Error, Result};
use async_cell_lock::AsyncOnceCell;
use std::{
    any::TypeId,
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};
use tokio::sync::{Mutex, MutexGuard};
use tracing::error;

/// Last recent use counter
type Lru = AtomicU64;

/// A trait that wrap the ProviderFactory to be able to use it in a Box<Any> trait object context.
trait AnyFactory: Send + Sync + 'static {
    fn create(&self) -> BoxFuture<'_, Result<CastProvider>>;
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
    fn create(&self) -> BoxFuture<'_, Result<CastProvider>> {
        Box::pin(async move { Ok(CastProvider::new(self.factory.create_provider().await?)) })
    }
}

/// A dependency container to be able to instantiate and provide connection to databases.
///
/// A database provider can be named and have a type.
pub struct ProviderContainer {
    last_gc: u64,
    lock: Mutex<()>,
    lru: Lru,
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
            Ok(index) => self.records.get(index).ok_or(Error::ProviderNotFound),
            Err(_) => Err(Error::ProviderNotFound),
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
                    .is_some_and(|r| *r.lru.get_mut() <= last_gc)
                {
                    r.provider.take();
                }
            }

            self.last_gc = new_gc;
        }
    }

    /// Gets or creates a database provider that have been previously registered with
    /// the `register` method.
    pub fn provide<'a, P: Provider>(&'a self, name: &'a str) -> BoxFuture<'a, Result<&'a P>> {
        Box::pin(async move {
            let type_id = TypeId::of::<P>();
            let rec = self.find_record(type_id, name)?;
            let castable = rec.get_or_init(&self.lru).await?;

            castable.downcast().ok_or_else(|| {
                error!("invalid cast for provider {name}");
                Error::Internal
            })
        })
    }

    pub(super) fn providers(&self) -> impl Iterator<Item = &'_ dyn Provider> {
        self.records
            .iter()
            .filter_map(|r| Some(r.get()?.provider()))
    }

    /// Register a provider factory that creates provider on demand. A provider can be named.
    pub fn register<F: ProviderFactory>(&mut self, name: impl Into<Box<str>>, factory: F) {
        let rec = Rec {
            factory: Box::new(Factory::new(factory)),
            name: name.into(),
            provider: AsyncOnceCell::new(),
            type_id: TypeId::of::<F::Provider>(),
        };

        #[allow(clippy::indexing_slicing)]
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
    lru: Lru,
    cast_provider: CastProvider,
}

impl ProviderRec {
    fn new(cast_provider: CastProvider) -> Self {
        Self {
            lru: Default::default(),
            cast_provider,
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
    fn get(&self) -> Option<&CastProvider> {
        Some(&self.provider.get()?.cast_provider)
    }

    async fn get_or_init<'a>(&'a self, lru: &'a Lru) -> Result<&'a CastProvider> {
        let provider_rec = self
            .provider
            .get_or_try_init::<_, Error>(async {
                Ok(ProviderRec::new(self.factory.create().await?))
            })
            .await?;

        provider_rec.lru.store(lru.fetch_add(1, Relaxed), Relaxed);

        Ok(&provider_rec.cast_provider)
    }
}

fn rec_key(rec: &Rec) -> (TypeId, &str) {
    (rec.type_id, &rec.name)
}
