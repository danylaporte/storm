use crate::{AsyncOnceCell, Entity, Log, State};
use std::hash::{BuildHasher, Hash};

pub trait ApplyLog {
    type Log: Sized;

    fn apply_log(&mut self, log: Self::Log);

    fn apply_log_opt(&mut self, log: Option<Self::Log>) {
        if let Some(log) = log {
            self.apply_log(log);
        }
    }
}

#[cfg(feature = "cache")]
impl<E: Entity, S> ApplyLog for cache::Cache<E::Key, E, S>
where
    E::Key: Clone + Eq + Hash,
    S: BuildHasher,
{
    type Log = Log<E>;

    fn apply_log(&mut self, log: Self::Log) {
        for (k, state) in log {
            match state {
                State::Inserted(v) => {
                    self.insert(k, v);
                }
                State::Removed => {
                    self.remove(&k);
                }
            }
        }
    }
}

impl<T> ApplyLog for AsyncOnceCell<T>
where
    T: ApplyLog,
{
    type Log = T::Log;

    fn apply_log(&mut self, log: Self::Log) {
        if let Some(t) = self.get_mut() {
            t.apply_log(log);
        }
    }
}
