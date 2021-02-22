use crate::{ApplyLog, Get, Init, Result};
use async_trait::async_trait;
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

#[derive(Clone)]
pub struct Version<T> {
    tag: u64,
    val: T,
}

impl<T> Version<T> {
    pub fn new(val: T) -> Self {
        Self {
            tag: COUNTER.fetch_add(1, Relaxed),
            val,
        }
    }
}

impl<T> ApplyLog for Version<T>
where
    T: ApplyLog,
{
    type Log = T::Log;

    fn apply_log(&mut self, log: Self::Log) {
        self.tag = COUNTER.fetch_add(1, Relaxed);
        self.val.apply_log(log);
    }
}

impl<T: Debug> Debug for Version<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.val.fmt(f)
    }
}

impl<T: Default> Default for Version<T> {
    fn default() -> Self {
        Self {
            tag: COUNTER.fetch_add(1, Relaxed),
            val: Default::default(),
        }
    }
}

impl<T> Deref for Version<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> DerefMut for Version<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.tag = COUNTER.fetch_add(1, Relaxed);
        &mut self.val
    }
}

impl<T: Display> Display for Version<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.val.fmt(f)
    }
}

impl<T: Eq> Eq for Version<T> {}

impl<K, V, T> Get<K, V> for Version<T>
where
    T: Get<K, V>,
{
    fn get(&self, k: &K) -> Option<&V> {
        self.val.get(k)
    }
}

impl<T: Hash> Hash for Version<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state)
    }
}

#[async_trait]
impl<P, T> Init<P> for Version<T>
where
    P: Send + Sync,
    T: Init<P>,
{
    async fn init(provider: &P) -> Result<Self> {
        Ok(Self {
            tag: COUNTER.fetch_add(1, Relaxed),
            val: T::init(provider).await?,
        })
    }
}

impl<T: IntoIterator> IntoIterator for Version<T> {
    type IntoIter = T::IntoIter;
    type Item = T::Item;

    fn into_iter(self) -> Self::IntoIter {
        self.val.into_iter()
    }
}

impl<T: Ord> Ord for Version<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.val.cmp(&other.val)
    }
}

impl<T: PartialEq> PartialEq for Version<T> {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl<T: PartialOrd> PartialOrd for Version<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);
