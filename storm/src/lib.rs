#[macro_use]
mod version_deps;

mod accessor;
mod apply_log;
mod as_ref_async;
mod as_ref_opt;
mod async_try_from;
mod ctx;
mod entity;
mod entity_of;
mod error;
mod get;
mod get_mut;
mod hash_table;
mod init;
mod insert;
pub mod mem;
mod one_to_many;
pub mod prelude;
pub mod provider;
mod remove;
mod state;
mod tag;
mod transaction;
mod vec_table;

pub use accessor::*;
pub use apply_log::ApplyLog;
pub use as_ref_async::AsRefAsync;
pub use as_ref_opt::AsRefOpt;
pub use async_cell_lock::{self, AsyncOnceCell, QueueRwLock};
pub use async_try_from::AsyncTryFrom;
pub use attached;
#[cfg(feature = "cache")]
pub use cache;
pub use ctx::*;
pub use entity::Entity;
pub use entity_of::EntityOf;
pub use error::Error;
pub use get::Get;
pub use get_mut::GetMut;
pub use hash_table::HashTable;
pub use init::Init;
pub use insert::Insert;
#[cfg(feature = "metrics")]
pub use metrics;
pub use once_cell::sync::OnceCell;
pub use one_to_many::OneToMany;
pub use parking_lot;
pub use provider::ProviderContainer;
pub use remove::Remove;
pub use state::LogState;
pub use tag::{NotifyTag, Tag};
pub use tokio;
pub use transaction::Transaction;
pub use vec_map::{self, VecMap};
pub use vec_table::VecTable;
pub use version_tag::{self, VersionTag};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a + Send>>;
pub type Log<E> = fxhash::FxHashMap<<E as Entity>::Key, LogState<E>>;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "derive")]
pub use storm_derive::{indexing, Ctx, LocksAwait, NoopDelete, NoopLoad, NoopSave};
#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlDelete, MssqlLoad, MssqlSave};

#[cfg(feature = "metrics")]
pub fn register_metrics() {
    use metrics::{register_histogram, Unit};

    register_histogram!(
        "storm.execution_time",
        Unit::Seconds,
        "execution time of a storm request."
    );
}
