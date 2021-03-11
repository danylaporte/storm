use crate::{Error, Result};
use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

/// Last recent use counter
type LRU = AtomicU64;

/// A trait that wrap the ProviderFactory to be able to use it in a Box<Any> trait object context.
#[async_trait]
trait AnyFactory {
    async fn create(&self) -> Result<Box<dyn Any>>;
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

#[async_trait]
impl<F, P> AnyFactory for Factory<F, P>
where
    F: ProviderFactory<Provider = P>,
    P: Send + Sync + 'static,
{
    async fn create(&self) -> Result<Box<dyn Any>> {
        Ok(Box::new(self.factory.create_provider().await?))
    }
}

/// A dependency container to be able to instantiate and provide connection to databases.
///
/// A database provider can be named and have a type.
pub struct ProviderContainer {
    last_gc: u64,
    lru: LRU,
    records: Vec<Rec>,
}

impl ProviderContainer {
    fn find_index(&self, type_id: TypeId, name: &str) -> std::result::Result<usize, usize> {
        self.records.binary_search_by(|probe| {
            probe
                .type_id
                .cmp(&type_id)
                .then_with(|| (*probe.name).cmp(name))
        })
    }

    fn find_record<'a>(&'a self, type_id: TypeId, name: &str) -> Result<&'a Rec> {
        match self.find_index(type_id, name) {
            Ok(index) => Ok(&self.records[index]),
            Err(_) => return Err(Error::ProviderNotFound),
        }
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
                    .map_or(false, |(_, lru)| *lru.get_mut() <= last_gc)
                {
                    r.provider.take();
                }
            }

            self.last_gc = new_gc;
        }
    }

    /// Gets or creates a database provider that have been previously registered with
    /// the `register` method.
    pub async fn provide<'a, T: Any>(&'a self, name: &str) -> Result<&'a T> {
        let type_id = TypeId::of::<T>();
        let rec = self.find_record(type_id, name)?;
        let provider = rec.get_or_init(&self.lru).await?;

        Ok(provider.downcast_ref().expect("provider"))
    }

    /// Register a provider factory that creates provider on demand. A provider can be named.
    pub fn register<'a, F>(&mut self, name: impl Into<Box<str>>, factory: F)
    where
        F: ProviderFactory + 'static,
    {
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
}

impl Default for ProviderContainer {
    fn default() -> Self {
        Self {
            last_gc: 0,
            lru: AtomicU64::new(1), // starting at 1 because the garbage collector start at 0.
            records: Vec::new(),
        }
    }
}

#[async_trait]
pub trait ProviderFactory: Send + Sync {
    type Provider: Send + Sync + 'static;

    async fn create_provider(&self) -> Result<Self::Provider>;
}

struct Rec {
    factory: Box<dyn AnyFactory>,
    name: Box<str>,
    provider: AsyncOnceCell<(Box<dyn Any>, LRU)>,
    type_id: TypeId,
}

impl Rec {
    async fn get_or_init(&self, lru: &LRU) -> Result<&Box<dyn Any>> {
        let (provider, rec_lru) = self
            .provider
            .get_or_try_init::<_, Error>(async {
                let provider = self.factory.create().await?;
                Ok((provider, AtomicU64::new(0)))
            })
            .await?;

        rec_lru.store(lru.fetch_add(1, Relaxed), Relaxed);

        Ok(provider)
    }
}
