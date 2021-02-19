use crate::Entity;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub struct CacheIsland<T>(Option<CacheIslandInternal<T>>);

struct CacheIslandInternal<T> {
    age: AtomicU64,
    value: T,
}

impl<T> CacheIsland<T> {
    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn clear_if_untouched_since(&mut self, age: u64) {
        if self.0.as_mut().map_or(false, |v| *v.age.get_mut() <= age) {
            self.0 = None;
        }
    }

    pub fn get(&self) -> Option<&T> {
        match self.0.as_ref() {
            Some(v) => {
                v.age.store(CACHE_ISLAND_AGE.fetch_add(1, Relaxed), Relaxed);
                Some(&v.value)
            }
            None => None,
        }
    }

    pub fn set(&mut self, value: T) {
        self.0 = Some(CacheIslandInternal {
            age: AtomicU64::new(CACHE_ISLAND_AGE.fetch_add(1, Relaxed)),
            value,
        })
    }
}

impl<T> Clone for CacheIsland<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(match self.0.as_ref() {
            Some(v) => Some(CacheIslandInternal {
                age: AtomicU64::new(CACHE_ISLAND_AGE.fetch_add(1, Relaxed)),
                value: v.value.clone(),
            }),
            None => None,
        })
    }
}

impl<T> Default for CacheIsland<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> Entity for CacheIsland<T>
where
    T: Entity,
{
    type Key = T::Key;
}

static CACHE_ISLAND_AGE: AtomicU64 = AtomicU64::new(0);
